#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* PostgreSQL v3 backend framing only. No request, row, message text or key bytes in maps. */
enum { TAG, LENGTH, SKIP, READY, ERROR_FIELDS };
enum { SEQUENCE = 1, MALFORMED, UNSUPPORTED, LIMIT, BUDGET, READ_FAILURE };
enum { FIELD, OTHER_VALUE, CODE_VALUE };
struct pg_flow {
    __u64 busy, raced;
    __u32 origin, next, port, fault;
    __u32 state, tag, nbytes, length, left, started, ready;
    __u32 field, seen, pos, mask, count, closed, segments, max_batch, reserved;
    __u32 items[32];
};
_Static_assert(sizeof(struct pg_flow) == 224, "PostgreSQL flow ABI");
struct cursor { struct __sk_buff *skb; struct pg_flow *r; __u32 at, end; };

static __always_inline void emit(struct pg_flow *r, __u32 event) {
    __u32 n = r->count;
    if (n >= 32) { r->fault = LIMIT; return; }
    r->items[n] = event;
    r->count = n + 1;
}

static __always_inline void pg_error_byte(struct pg_flow *r, __u8 ch) {
    if (r->field == FIELD) {
        if (!ch) {
            if (r->left || !r->seen) { r->fault = MALFORMED; return; }
            __u32 code = r->mask == 1 ? 1 : r->mask == 2 ? 2 : r->mask == 4 ? 3 : 0;
            emit(r, (r->ready ? 12 : 8) + code);
            r->state = TAG;
        } else if (ch == 'C') {
            if (r->seen) { r->fault = MALFORMED; return; }
            r->field = CODE_VALUE;
            r->pos = 0;
            r->mask = 7;
        } else r->field = OTHER_VALUE;
    } else if (r->field == OTHER_VALUE) {
        if (!ch) r->field = FIELD;
    } else {
        __u32 i = r->pos;
        if (i == 5 && !ch) {
            r->seen = 1;
            r->field = FIELD;
        } else if (i < 5 && ((ch >= '0' && ch <= '9') || (ch >= 'A' && ch <= 'Z'))) {
            if (ch != "28P01"[i]) r->mask &= ~1U;
            if (ch != "22012"[i]) r->mask &= ~2U;
            if (ch != "25P02"[i]) r->mask &= ~4U;
            r->pos = i + 1;
        } else r->fault = MALFORMED;
    }
    if (!r->left && r->state != TAG) r->fault = MALFORMED;
}

static long step(__u32 index, void *data) {
    struct cursor *c = data;
    struct pg_flow *r = c->r;
    if (c->at >= c->end || r->fault) return 1;
    if (r->state == SKIP) {
        __u32 n = c->end - c->at;
        if (n > r->left) n = r->left;
        c->at += n;
        r->left -= n;
        if (!r->left) {
            if (r->tag == 'D') emit(r, 1);
            r->state = TAG;
        }
        return 0;
    }
    __u8 ch;
    if (bpf_skb_load_bytes(c->skb, c->at, &ch, 1)) { r->fault = READ_FAILURE; return 1; }
    c->at++;
    if (r->state == TAG) {
        if (!r->started && ch != 'R' && ch != 'E') { r->fault = UNSUPPORTED; return 1; }
        switch (ch) {
        case 'R': case 'E': case 'Z': case 'S': case 'K': case 'T':
        case 'D': case 'C': case 'N': case 'I': break;
        default: r->fault = UNSUPPORTED; return 1;
        }
        r->started = 1;
        r->tag = ch;
        r->nbytes = r->length = 0;
        r->state = LENGTH;
    } else if (r->state == LENGTH) {
        r->length = (r->length << 8) | ch;
        if (++r->nbytes != 4) return 0;
        __u32 len = r->length, tag = r->tag;
        if (len > 16 * 1024 * 1024) { r->fault = LIMIT; return 1; }
        if (len < 4 || (tag == 'Z' && len != 5) || (tag == 'I' && len != 4) ||
            (tag == 'K' && len != 12) || (tag == 'R' && len < 8) ||
            ((tag == 'T' || tag == 'D' || tag == 'S') && len < 6) ||
            ((tag == 'C' || tag == 'E' || tag == 'N') && len < 5)) {
            r->fault = MALFORMED; return 1;
        }
        r->left = len - 4;
        if (tag == 'Z') r->state = READY;
        else if (tag == 'E') { r->field = FIELD; r->mask = r->seen = 0; r->state = ERROR_FIELDS; }
        else r->state = r->left ? SKIP : TAG;
    } else if (r->state == READY) {
        if (ch != 'I' && ch != 'T' && ch != 'E') { r->fault = MALFORMED; return 1; }
        emit(r, ch == 'I' ? 2 : ch == 'T' ? 3 : 4);
        r->ready = 1;
        r->state = TAG;
        r->left = 0;
    } else if (r->state == ERROR_FIELDS) {
        r->left--;
        pg_error_byte(r, ch);
    } else r->fault = MALFORMED;
    return 0;
}

#define STREAM_ROW pg_flow
#define STREAM_INGRESS pg_wire_in
#define STREAM_PORT PG_PORT
#include "scoped_stream.inc.c"
