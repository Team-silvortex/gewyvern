#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

enum { SEQUENCE = 1, MALFORMED, UNSUPPORTED, LIMIT, BUDGET, READ_FAILURE };
struct item { __u32 op_status, value_bytes; };
struct mc_flow {
    __u64 busy, raced;
    __u32 origin, next, port, fault;
    __u32 pos, op, extras, status, length, left;
    __u32 count, closed, segments, max_batch;
    struct item items[32];
};
_Static_assert(sizeof(struct mc_flow) == 328, "Memcached map ABI");
struct cursor { struct __sk_buff *skb; struct mc_flow *r; __u32 at, end; };

static long step(__u32 index, void *data) {
    struct cursor *c = data;
    struct mc_flow *r = c->r;
    if (c->at >= c->end || r->fault) return 1;
    if (r->left) {
        /* Opaque, CAS and body are never inspected, even across an skb boundary. */
        __u32 n = c->end - c->at;
        if (n > r->left) n = r->left;
        c->at += n;
        r->left -= n;
        if (!r->left) {
            __u32 i = r->count;
            if (i >= 32) { r->fault = LIMIT; return 1; }
            r->items[i].op_status = r->op << 16 | r->status;
            r->items[i].value_bytes = (!r->status && (r->op == 0 || r->op == 9)) ? r->length - 4 : 0;
            r->count = i + 1;
            r->pos = 0;
        }
        return 0;
    }
    __u8 ch;
    if (bpf_skb_load_bytes(c->skb, c->at, &ch, 1)) { r->fault = READ_FAILURE; return 1; }
    c->at++;
    switch (r->pos++) {
    case 0:
        if (ch != 0x81) r->fault = UNSUPPORTED;
        break;
    case 1:
        switch (ch) {
        case 0: case 1: case 2: case 3: case 4: case 9: case 10: case 14: case 15:
            r->op = ch; break;
        default: r->fault = UNSUPPORTED;
        }
        break;
    case 2: case 3: /* None of the supported response types returns a key. */
    case 5:
        if (ch) r->fault = MALFORMED;
        break;
    case 4: r->extras = ch; break;
    case 6: r->status = (__u32)ch << 8; break;
    case 7: r->status |= ch; break;
    case 8: r->length = ch; break;
    case 9: case 10: r->length = (r->length << 8) | ch; break;
    case 11: {
        __u32 length = (r->length << 8) | ch;
        if (length > 16 * 1024 * 1024) { r->fault = LIMIT; break; }
        if (!r->status && (r->op == 0 || r->op == 9)) {
            if (r->extras != 4 || length < 4) { r->fault = MALFORMED; break; }
        } else if (r->extras || (!r->status && length)) { r->fault = MALFORMED; break; }
        r->length = length;
        r->left = 12 + length;
        break;
    }
    default: r->fault = MALFORMED;
    }
    return 0;
}
#define STREAM_ROW mc_flow
#define STREAM_INGRESS mc_binary_in
#define STREAM_PORT MC_PORT
#include "scoped_stream.inc.c"
