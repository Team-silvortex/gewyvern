#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* Validation only: two owned sockets, ingress only, always allow traffic. */
struct packet_event {
    __u64 sequence, monotonic_ns, cookie;
    __u32 cpu, ifindex, source, destination, seq, ack;
    __u16 sport, dport, total_len, window, payload_len, prefix_len;
    __u8 flags, ip_header_len, tcp_header_len;
    __u8 prefix[64];
};
struct capture_counts { __u64 total, dropped; };
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 128);
    __type(key, __u32);
    __type(value, struct packet_event);
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
    return ((__u32)p[0] << 24) | ((__u32)p[1] << 16) |
           ((__u32)p[2] << 8) | p[3];
}

SEC("cgroup_skb/ingress")
int capture_http_prefix(struct __sk_buff *ctx) {
    __u64 cookie = bpf_get_socket_cookie(ctx);
    if (cookie != COOKIE_A && cookie != COOKIE_B)
        return 1;
    __u32 zero = 0;
    struct capture_counts *stats = bpf_map_lookup_elem(&counts, &zero);
    if (!stats)
        return 1;
    __u8 ip[20], tcp[20];
    if (bpf_skb_load_bytes(ctx, 0, ip, sizeof(ip)))
        goto malformed;
    __u32 ihl = (ip[0] & 15) * 4;
    if (ip[0] >> 4 != 4 || ip[9] != 6 || ihl < 20 || (be16(ip + 6) & 0x3fff))
        goto malformed;
    if (be32(ip + 12) != DESTINATION_IPV4 || be32(ip + 16) != SOURCE_IPV4)
        goto malformed;
    if (bpf_skb_load_bytes(ctx, ihl, tcp, sizeof(tcp)))
        goto malformed;
    __u32 thl = (tcp[12] >> 4) * 4;
    __u32 total = be16(ip + 2);
    if (thl < 20 || total < ihl + thl || total > ctx->len || be16(tcp) != 3000 ||
        (cookie == COOKIE_A && be16(tcp + 2) != PORT_A) ||
        (cookie == COOKIE_B && be16(tcp + 2) != PORT_B))
        goto malformed;
    __u32 payload = total - ihl - thl;
    if (!payload)
        return 1;
    struct packet_event value = {};
    value.monotonic_ns = bpf_ktime_get_ns();
    value.cookie = cookie;
    value.cpu = bpf_get_smp_processor_id();
    value.ifindex = ctx->ifindex;
    value.source = be32(ip + 12);
    value.destination = be32(ip + 16);
    value.sport = be16(tcp);
    value.dport = be16(tcp + 2);
    value.total_len = total;
    value.seq = be32(tcp + 4);
    value.ack = be32(tcp + 8);
    value.flags = tcp[13];
    value.window = be16(tcp + 14);
    value.ip_header_len = ihl;
    value.tcp_header_len = thl;
    value.payload_len = payload;
    __u32 length = payload > 64 ? 64 : payload;
    /* Keep a local nonzero bound visible to the verifier after LLVM folding. */
    __asm__ volatile("" : "+r"(length));
    if (length == 0 || length > 64)
        goto malformed;
    value.prefix_len = length;
    if (bpf_skb_load_bytes(ctx, ihl + thl, value.prefix, length))
        goto malformed;
    __u64 slot = __sync_fetch_and_add(&stats->total, 1);
    if (slot >= 128)
        goto malformed;
    __u32 key = slot;
    value.sequence = slot + 1;
    if (bpf_map_update_elem(&events, &key, &value, BPF_NOEXIST))
        goto malformed;
    return 1;
malformed:
    __sync_fetch_and_add(&stats->dropped, 1);
    return 1;
}

char LICENSE[] SEC("license") = "GPL";
