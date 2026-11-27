#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* RESP2 server-to-client framing. Only types, lengths and error counts leave the parser. */
enum { TYPE, LINE, LINE_LF, NUMBER, NUMBER_LF, BODY, BODY_CR, BODY_LF };
enum { SIMPLE = 1, ERROR, INTEGER, BULK, NULL_BULK, ARRAY, NULL_ARRAY };
enum { SEQUENCE = 1, MALFORMED, UNSUPPORTED, LIMIT, BUDGET, READ_FAILURE, DEPTH };
struct resp {
    __u64 busy, raced;
    __u32 origin, next, port, fault, state, tag, digits, negative, length, left;
    __u32 array_left, array_len, inner_left, array_errors, count, closed, segments, max_batch;
    __u32 items[32];
};
_Static_assert(sizeof(struct resp) == 216, "RESP map ABI");

static __always_inline void finish(struct resp *r, __u32 kind, __u32 length) {
    r->state = TYPE;
    if (r->array_left) {
        if (kind == ERROR) r->array_errors++;
        /* The entire inner array consumes exactly one outer slot. */
        if (r->inner_left && --r->inner_left) return;
        if (--r->array_left) return;
        kind = ARRAY;
        /* An array has <=128 children and <=128*128 error leaves. */
        length = (r->array_errors << 8) | r->array_len;
        r->array_errors = 0;
    }
    __u32 i = r->count;
    if (i >= 32) { r->fault = LIMIT; return; }
    r->items[i] = length << 4 | kind;
    r->count = i + 1;
}

struct cursor { struct __sk_buff *skb; struct resp *r; __u32 at, end; };
static long step(__u32 index, void *data) {
    struct cursor *c = data;
    struct resp *r = c->r;
    if (c->at >= c->end || r->fault) return 1;
    if (r->state == BODY) {
        __u32 n = c->end - c->at;
        if (n > r->left) n = r->left;
        c->at += n;
        r->left -= n;
        if (!r->left) r->state = BODY_CR;
        return 0;
    }
    __u8 ch;
    if (bpf_skb_load_bytes(c->skb, c->at, &ch, 1)) { r->fault = READ_FAILURE; return 1; }
    c->at++;
    switch (r->state) {
    case TYPE:
        r->digits = r->negative = r->length = 0;
        if (ch == '+' || ch == '-') {
            r->tag = ch == '+' ? SIMPLE : ERROR;
            r->state = LINE;
        } else if (ch == '*' && r->inner_left) r->fault = DEPTH;
        else if (ch == ':' || ch == '$' || ch == '*') {
            r->tag = ch == ':' ? INTEGER : ch == '$' ? BULK : ARRAY;
            r->state = NUMBER;
        } else r->fault = UNSUPPORTED;
        break;
    case LINE:
        if (ch == '\r') r->state = LINE_LF;
        else if (ch == '\n') r->fault = MALFORMED;
        break;
    case LINE_LF:
        if (ch != '\n') r->fault = MALFORMED;
        else finish(r, r->tag, 0);
        break;
    case NUMBER:
        if (ch == '-' && !r->digits && !r->negative) r->negative = 1;
        else if (ch == '\r' && r->digits) r->state = NUMBER_LF;
        else if (ch >= '0' && ch <= '9' && r->digits < 19) {
            r->digits++;
            if (r->tag != INTEGER) {
                if (r->length > (16777216U - (ch - '0')) / 10U) r->fault = LIMIT;
                else r->length = r->length * 10 + ch - '0';
            }
        } else r->fault = MALFORMED;
        break;
    case NUMBER_LF:
        if (ch != '\n') r->fault = MALFORMED;
        else if (r->tag == INTEGER) finish(r, INTEGER, 0);
        else if (r->negative) {
            if (r->digits != 1 || r->length != 1) r->fault = MALFORMED;
            else finish(r, r->tag == BULK ? NULL_BULK : NULL_ARRAY, 0);
        } else if (r->tag == BULK) {
            r->left = r->length;
            r->state = r->left ? BODY : BODY_CR;
        } else if (r->length > 128) r->fault = LIMIT;
        else if (!r->length) finish(r, ARRAY, 0);
        else {
            if (r->array_left) r->inner_left = r->length;
            else r->array_left = r->array_len = r->length;
            r->state = TYPE;
        }
        break;
    case BODY_CR:
        if (ch != '\r') r->fault = MALFORMED;
        else r->state = BODY_LF;
        break;
    case BODY_LF:
        if (ch != '\n') r->fault = MALFORMED;
        else finish(r, BULK, r->length);
        break;
    default: r->fault = MALFORMED;
    }
    return 0;
}

#define STREAM_ROW resp
#define STREAM_INGRESS redis_resp_in
#define STREAM_PORT REDIS_PORT
#include "scoped_stream.inc.c"
