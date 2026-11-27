/* The including protocol parser supplies step() and its flow layout. */
struct counts { __u64 no_origin, no_cookie, capacity, invalid; };
_Static_assert(sizeof(struct counts) == 32, "TCP counters ABI");
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 64);
    __type(key, __u64);
    __type(value, struct STREAM_ROW);
} flows SEC(".maps");
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct counts);
} counts SEC(".maps");
static __always_inline __u16 be16(const __u8 *p) { return ((__u16)p[0] << 8) | p[1]; }
static __always_inline __u32 be32(const __u8 *p) { return ((__u32)be16(p) << 16) | be16(p + 2); }

SEC("cgroup_skb/ingress")
int STREAM_INGRESS(struct __sk_buff *ctx) {
    __u8 ip[20], tcp[20];
    if (ctx->ifindex != 1 || bpf_skb_load_bytes(ctx, 0, ip, 20) || ip[0] >> 4 != 4 ||
        ip[9] != 6 || ip[12] != 127 || ip[16] != 127 || (be16(ip+6) & 0x3fff)) return 1;
    __u32 ihl = (ip[0] & 15) * 4;
    if (ihl < 20 || bpf_skb_load_bytes(ctx, ihl, tcp, 20) || be16(tcp) != STREAM_PORT) return 1;
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
    struct STREAM_ROW *r = bpf_map_lookup_elem(&flows, &cookie);
    if (synack) {
        if (len) goto invalid;
        if (!r) {
            struct STREAM_ROW start = {.origin = seq+1, .next = seq+1, .port = be16(tcp+2)};
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
