#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* Validation ABI only: two owned sockets; never change or block traffic. */
struct datagram_event {
    __u64 sequence, monotonic_ns, cookie;
    __u32 cpu, ifindex, total_len;
    __u8 source[16], destination[16];
    __u16 sport, dport, payload_len, prefix_len;
    __u8 ip_header_len, ip_version;
    __u8 prefix[128];
};
struct capture_counts { __u64 total, dropped; };
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 128);
    __type(key, __u32);
    __type(value, struct datagram_event);
} events SEC(".maps");
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct capture_counts);
} counts SEC(".maps");
static __always_inline __u16 be16(const __u8 *p) {
    return ((__u16)p[0] << 8) | p[1];
}
static __always_inline __u32 be32(const __u8 *p) {
    return ((__u32)p[0] << 24) | ((__u32)p[1] << 16) | ((__u32)p[2] << 8) | p[3];
}
#ifdef CAPTURE_EGRESS
SEC("cgroup_skb/egress")
#else
SEC("cgroup_skb/ingress")
#endif
int capture_udp_dns(struct __sk_buff *ctx) {
    __u64 cookie = bpf_get_socket_cookie(ctx);
    if (cookie != COOKIE_A && cookie != COOKIE_B)
        return 1;
    __u32 zero = 0;
    struct capture_counts *stats = bpf_map_lookup_elem(&counts, &zero);
    if (!stats)
        return 1;
    __u8 ip[40], udp[8];
#if IP_VERSION == 6
    if (bpf_skb_load_bytes(ctx, 0, ip, 40))
        goto malformed;
    __u32 ihl = 40, total = 40 + be16(ip + 4);
    /* No extension headers, fragments or jumbograms in this bounded fixture. */
    if (ip[0] >> 4 != 6 || ip[6] != 17 || total < 48 || total > ctx->len)
        goto malformed;
#else
    if (bpf_skb_load_bytes(ctx, 0, ip, 20))
        goto malformed;
    __u32 ihl = (ip[0] & 15) * 4, total = be16(ip + 2);
    if (ip[0] >> 4 != 4 || ip[9] != 17 || ihl < 20 ||
        (be16(ip + 6) & 0x3fff) || total < ihl + 8 || total > ctx->len)
        goto malformed;
#endif
    if (bpf_skb_load_bytes(ctx, ihl, udp, sizeof(udp)))
        goto malformed;
    __u16 local = cookie == COOKIE_A ? PORT_A : PORT_B;
#ifdef CAPTURE_EGRESS
    if (be16(udp) != local || be16(udp + 2) != REMOTE_PORT)
        goto malformed;
    const __u32 source[4] = {SOURCE_0, SOURCE_1, SOURCE_2, SOURCE_3};
    const __u32 destination[4] = {DESTINATION_0, DESTINATION_1, DESTINATION_2, DESTINATION_3};
#else
    if (be16(udp) != REMOTE_PORT || be16(udp + 2) != local)
        goto malformed;
    const __u32 source[4] = {DESTINATION_0, DESTINATION_1, DESTINATION_2, DESTINATION_3};
    const __u32 destination[4] = {SOURCE_0, SOURCE_1, SOURCE_2, SOURCE_3};
#endif
#if IP_VERSION == 6
#pragma unroll
    for (int i = 0; i < 4; i++)
        if (be32(ip + 8 + 4*i) != source[i] || be32(ip + 24 + 4*i) != destination[i])
            goto malformed;
#else
    if (be32(ip + 12) != source[0] || be32(ip + 16) != destination[0])
        goto malformed;
#endif
    if (be16(udp + 4) != total - ihl)
        goto malformed;
    struct datagram_event value = {};
    value.monotonic_ns = bpf_ktime_get_ns();
    value.cookie = cookie;
    value.cpu = bpf_get_smp_processor_id();
    value.ifindex = ctx->ifindex;
#if IP_VERSION == 6
    __builtin_memcpy(value.source, ip + 8, 16);
    __builtin_memcpy(value.destination, ip + 24, 16);
#else
    __builtin_memcpy(value.source, ip + 12, 4);
    __builtin_memcpy(value.destination, ip + 16, 4);
#endif
    value.sport = be16(udp);
    value.dport = be16(udp + 2);
    value.total_len = total;
    value.ip_header_len = ihl;
    value.ip_version = IP_VERSION;
    value.payload_len = total - ihl - 8;
    __u32 length = value.payload_len > 128 ? 128 : value.payload_len;
    __asm__ volatile("" : "+r"(length));
    value.prefix_len = length;
    if (value.payload_len) {
        if (length == 0 || length > 128 || bpf_skb_load_bytes(ctx, ihl + 8, value.prefix, length))
            goto malformed;
    }
    __u64 slot = __sync_fetch_and_add(&stats->total, 1);
    value.sequence = slot + 1;
    if (slot >= 128)
        goto malformed;
    __u32 key = slot;
    if (bpf_map_update_elem(&events, &key, &value, BPF_NOEXIST))
        goto malformed;
    return 1;
malformed:
    __sync_fetch_and_add(&stats->dropped, 1);
    return 1;
}
char LICENSE[] SEC("license") = "GPL";
