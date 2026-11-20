#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* Product ABI v1. Observe only; all paths allow the original packet. */
struct event {
    __u64 monotonic_ns, cookie, netns_cookie;
    __u32 cpu, ifindex, total_len;
    __u16 sport, dport, payload_len, prefix_len;
    __u8 source[16], destination[16];
    __u8 version, ip_version, ip_header_len, outbound;
    __u8 prefix[128];
};
struct counters { __u64 matched, ring_dropped, unsupported, malformed, no_cookie; };
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, RING_BYTES);
} events SEC(".maps");
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct counters);
} counts SEC(".maps");
static __always_inline __u16 be16(const __u8 *p) { return ((__u16)p[0] << 8) | p[1]; }
static __always_inline int observe(struct __sk_buff *ctx, __u8 outbound) {
    __u32 zero = 0;
    struct counters *stats = bpf_map_lookup_elem(&counts, &zero);
    if (!stats) return 1;
    __u8 ip[40], udp[8];
    if (bpf_skb_load_bytes(ctx, 0, ip, 1)) goto malformed;
    __u8 version = ip[0] >> 4;
    __u32 header, total;
    if (version == 4) {
        if (bpf_skb_load_bytes(ctx, 0, ip, 20)) goto malformed;
        if (ip[9] != 17) return 1;
        if (be16(ip + 6) & 0x3fff) goto unsupported;
        header = (ip[0] & 15) * 4;
        total = be16(ip + 2);
        if (header < 20) goto malformed;
    } else if (version == 6) {
        if (bpf_skb_load_bytes(ctx, 0, ip, 40)) goto malformed;
        if (ip[6] == 0 || ip[6] == 43 || ip[6] == 44 || ip[6] == 51 || ip[6] == 60)
            goto unsupported;
        if (ip[6] != 17) return 1;
        if (!be16(ip + 4)) goto unsupported;
        header = 40;
        total = 40 + be16(ip + 4);
    } else return 1;
    if (total < header + 8 || bpf_skb_load_bytes(ctx, header, udp, 8))
        goto malformed;
    if (be16(udp) != UDP_PORT && be16(udp + 2) != UDP_PORT) return 1;
    /* ABI v1 describes one datagram, not a GSO/GRO aggregate. Filter the
     * selected port first so unrelated offload does not poison this capture. */
    if (ctx->gso_size || ctx->gso_segs > 1) goto unsupported;
    if (total != ctx->len || be16(udp + 4) != total - header || !be16(udp) || !be16(udp + 2))
        goto malformed;
    __u64 cookie = bpf_get_socket_cookie(ctx), netns = bpf_get_netns_cookie(ctx);
    if (!cookie || !netns) { __sync_fetch_and_add(&stats->no_cookie, 1); return 1; }
    __sync_fetch_and_add(&stats->matched, 1);
    struct event *value = bpf_ringbuf_reserve(&events, sizeof(*value), 0);
    if (!value) { __sync_fetch_and_add(&stats->ring_dropped, 1); return 1; }
    __builtin_memset(value, 0, sizeof(*value));
    value->version = 1;
    value->monotonic_ns = bpf_ktime_get_ns();
    value->cookie = cookie;
    value->netns_cookie = netns;
    value->cpu = bpf_get_smp_processor_id();
    value->ifindex = ctx->ifindex;
    value->total_len = total;
    value->ip_version = version;
    value->ip_header_len = header;
    value->outbound = outbound;
    if (version == 6) {
        __builtin_memcpy(value->source, ip + 8, 16);
        __builtin_memcpy(value->destination, ip + 24, 16);
    } else {
        __builtin_memcpy(value->source, ip + 12, 4);
        __builtin_memcpy(value->destination, ip + 16, 4);
    }
    value->sport = be16(udp);
    value->dport = be16(udp + 2);
    value->payload_len = total - header - 8;
    __u32 length = value->payload_len > 128 ? 128 : value->payload_len;
    __asm__ volatile("" : "+r"(length));
    value->prefix_len = length;
    if (length && (length > 128 || bpf_skb_load_bytes(ctx, header + 8, value->prefix, length))) {
        bpf_ringbuf_discard(value, 0);
        __sync_fetch_and_add(&stats->ring_dropped, 1);
        __sync_fetch_and_add(&stats->malformed, 1);
        return 1;
    }
    bpf_ringbuf_submit(value, 0);
    return 1;
unsupported:
    __sync_fetch_and_add(&stats->unsupported, 1);
    return 1;
malformed:
    __sync_fetch_and_add(&stats->malformed, 1);
    return 1;
}
SEC("cgroup_skb/ingress") int udp_ingress(struct __sk_buff *ctx) { return observe(ctx, 0); }
SEC("cgroup_skb/egress") int udp_egress(struct __sk_buff *ctx) { return observe(ctx, 1); }
char LICENSE[] SEC("license") = "GPL";
