#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* RESP2 server-to-client framing. Only types and declared lengths leave the parser. */
enum { TYPE, LINE, LINE_LF, NUMBER, NUMBER_LF, BODY, BODY_CR, BODY_LF };
enum { SIMPLE = 1, ERROR, INTEGER, BULK, NULL_BULK, ARRAY, NULL_ARRAY };
enum { SEQUENCE = 1, MALFORMED, UNSUPPORTED, LIMIT, BUDGET, READ_FAILURE };
struct resp {
    __u64 busy, raced;
    __u32 origin, next, port, fault, state, tag, digits, negative, length, left;
    __u32 array_left, array_len, count, closed, segments, max_batch;
    __u32 items[32];
};
struct counts { __u64 no_origin, no_cookie, capacity, invalid; };
_Static_assert(sizeof(struct resp) == 208, "RESP map ABI");
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 64);
    __type(key, __u64);
    __type(value, struct resp);
} flows SEC(".maps");
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct counts);
} counts SEC(".maps");
static __always_inline __u16 be16(const __u8 *p) { return ((__u16)p[0] << 8) | p[1]; }
static __always_inline __u32 be32(const __u8 *p) { return ((__u32)be16(p) << 16) | be16(p + 2); }

static __always_inline void finish(struct resp *r, __u32 kind, __u32 length) {
    r->state = TYPE;
    if (r->array_left) {
        if (--r->array_left) return;
        kind = ARRAY;
        length = r->array_len;
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
        } else if (ch == ':' || ch == '$' || (ch == '*' && !r->array_left)) {
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
        else { r->array_left = r->array_len = r->length; r->state = TYPE; }
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

SEC("cgroup_skb/ingress")
int redis_resp_in(struct __sk_buff *ctx) {
    __u8 ip[20], tcp[20];
    if (ctx->ifindex != 1 || bpf_skb_load_bytes(ctx, 0, ip, 20) || ip[0] >> 4 != 4 ||
        ip[9] != 6 || ip[12] != 127 || ip[16] != 127 || (be16(ip+6) & 0x3fff)) return 1;
    __u32 ihl = (ip[0] & 15) * 4;
    if (ihl < 20 || bpf_skb_load_bytes(ctx, ihl, tcp, 20) || be16(tcp) != REDIS_PORT) return 1;
    __u32 zero = 0;
    struct counts *stats = bpf_map_lookup_elem(&counts, &zero);
    if (!stats) return 1;
    __u32 header = ihl + (tcp[12] >> 4) * 4, total = be16(ip+2);
    if ((tcp[12] >> 4) < 5 || total < header || total > ctx->len) goto invalid;
    __u32 len = total - header, seq = be32(tcp+4), flags = tcp[13];
    int synack = (flags & 0x17) == 0x12;
    if (!len && !synack && !(flags & 5)) return 1;
    __u64 cookie = bpf_get_socket_cookie(ctx);
    if (!cookie) { __sync_fetch_and_add(&stats->no_cookie, 1); return 1; }
    struct resp *r = bpf_map_lookup_elem(&flows, &cookie);
    if (synack) {
        if (len) goto invalid;
        if (!r) {
            struct resp start = {.origin = seq+1, .next = seq+1, .port = be16(tcp+2)};
            if (!bpf_map_update_elem(&flows, &cookie, &start, BPF_NOEXIST)) return 1;
            r = bpf_map_lookup_elem(&flows, &cookie);
            if (!r) { __sync_fetch_and_add(&stats->capacity, 1); return 1; }
        }
        if (r->origin != seq+1) __sync_fetch_and_or(&r->raced, 1);
        return 1;
    }
    if (!r) { if (len) __sync_fetch_and_add(&stats->no_origin, 1); return 1; }
    if (__sync_val_compare_and_swap(&r->busy, 0, 1)) {
        __sync_fetch_and_or(&r->raced, 1);
        return 1;
    }
    if (!r->fault && (len || (flags & 1))) {
        if (r->closed || r->next != seq || (flags & 0x26) || !(flags & 0x10)) r->fault = SEQUENCE;
        else if (len) {
            struct cursor c = {.skb = ctx, .r = r, .at = header, .end = total};
            __u32 before = r->count;
            bpf_loop(512, step, &c, 0);
            if (!r->fault && c.at != total) r->fault = BUDGET;
            r->segments++;
            __u32 batch = r->count - before;
            if (batch > r->max_batch) r->max_batch = batch;
            r->next += len;
        }
    }
    if (flags & 5) r->closed = 1;
    __sync_fetch_and_and(&r->busy, 0);
    return 1;
invalid:
    __sync_fetch_and_add(&stats->invalid, 1);
    return 1;
}
char LICENSE[] SEC("license") = "GPL";
