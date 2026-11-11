#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* Validation only: two owned sockets, ingress only, always allow traffic. */
#ifndef CAPTURE_MAP_CAPACITY
#define CAPTURE_MAP_CAPACITY 128
#endif
#if CAPTURE_MAP_CAPACITY < 1 || CAPTURE_MAP_CAPACITY > 128
#error Invalid validation map capacity
#endif
struct packet_event {
    __u64 sequence, monotonic_ns, cookie;
    __u32 cpu, ifindex, source, destination, seq, ack;
    __u16 sport, dport, total_len, window, payload_len, prefix_len;
    __u8 flags, ip_header_len, tcp_header_len;
    __u8 prefix[64];
};
#ifdef CAPTURE_STREAMING
struct capture_counts { __u64 total, dropped, malformed; };
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 1048576);
} events SEC(".maps");
_Static_assert(sizeof(struct packet_event) == 128, "packet event ABI changed");
#else
struct capture_counts { __u64 total, dropped; };
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, CAPTURE_MAP_CAPACITY);
    __type(key, __u32);
    __type(value, struct packet_event);
} events SEC(".maps");
#endif
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
    if (thl < 20 || total < ihl + thl || total > ctx->len ||
        (cookie == COOKIE_A && (be16(tcp + 2) != PORT_A || be16(tcp) != REMOTE_PORT_A)) ||
        (cookie == COOKIE_B && (be16(tcp + 2) != PORT_B || be16(tcp) != REMOTE_PORT_B)))
        goto malformed;
    __u32 payload = total - ihl - thl;
    __u8 syn_ack = (tcp[13] & 0x17) == 0x12;
    /* Preserve the peer ISN as an observed stream origin. Fast Open is out of scope. */
    if (syn_ack && payload)
        goto malformed;
    if (!payload && !syn_ack)
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
    value.prefix_len = length;
    if (payload) {
        if (length == 0 || length > 64)
            goto malformed;
        if (bpf_skb_load_bytes(ctx, ihl + thl, value.prefix, length))
            goto malformed;
    }
    __u64 slot = __sync_fetch_and_add(&stats->total, 1);
    value.sequence = slot + 1;
#ifdef CAPTURE_STREAMING
    /* Never block network traffic or overwrite unread evidence. */
    if (bpf_ringbuf_output(&events, &value, sizeof(value), 0))
        __sync_fetch_and_add(&stats->dropped, 1);
#else
    if (slot >= 128)
        goto malformed;
    __u32 key = slot;
    if (bpf_map_update_elem(&events, &key, &value, BPF_NOEXIST))
        goto malformed;
#endif
    return 1;
malformed:
#ifdef CAPTURE_STREAMING
    __sync_fetch_and_add(&stats->malformed, 1);
#else
    __sync_fetch_and_add(&stats->dropped, 1);
#endif
    return 1;
}

char LICENSE[] SEC("license") = "GPL";
