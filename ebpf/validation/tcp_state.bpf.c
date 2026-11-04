#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>

#ifndef COOKIE_A
#error COOKIE_A must identify an owned socket
#endif
#ifndef COOKIE_B
#error COOKIE_B must identify an owned socket
#endif
#ifndef SOURCE_IPV4
#error SOURCE_IPV4 must identify the owned source address
#endif
#ifndef DESTINATION_IPV4
#error DESTINATION_IPV4 must identify the owned destination address
#endif

/* Minimal CO-RE views, not copied kernel offsets. No kernel pointers leave BPF. */
struct ns_common { unsigned int inum; } __attribute__((preserve_access_index));
struct net { struct ns_common ns; } __attribute__((preserve_access_index));
struct possible_net { struct net *net; } __attribute__((preserve_access_index));
struct cookie_counter { long long counter; } __attribute__((preserve_access_index));
struct sock_common {
    struct possible_net skc_net;
    struct cookie_counter skc_cookie;
} __attribute__((preserve_access_index));
struct sock { struct sock_common __sk_common; } __attribute__((preserve_access_index));
struct trace_event_raw_inet_sock_set_state {
    const void *skaddr;
    int oldstate;
    int newstate;
    __u16 sport;
    __u16 dport;
    __u16 family;
    __u16 protocol;
    __u8 saddr[4];
    __u8 daddr[4];
} __attribute__((preserve_access_index));

struct capture_event {
    __u64 sequence;
    __u64 monotonic_ns;
    __u64 cookie;
    __u32 netns;
    __u32 cpu;
    __u16 sport;
    __u16 dport;
    __u16 family;
    __u8 old_state;
    __u8 new_state;
    __u8 saddr[4];
    __u8 daddr[4];
};

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 128);
    __type(key, __u32);
    __type(value, struct capture_event);
} events SEC(".maps");

struct capture_counts { __u64 total; __u64 dropped; };
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct capture_counts);
} counts SEC(".maps");

SEC("tracepoint/sock/inet_sock_set_state")
int capture_tcp_state(struct trace_event_raw_inet_sock_set_state *ctx) {
    if (ctx->family != 2 || ctx->protocol != 6)
        return 0;
    const struct sock *sk = (const struct sock *)ctx->skaddr;
    __u64 cookie = BPF_CORE_READ(sk, __sk_common.skc_cookie.counter);
    if (cookie != COOKIE_A && cookie != COOKIE_B)
        return 0;
    __u8 src[4], dst[4];
    BPF_CORE_READ_INTO(&src, ctx, saddr);
    BPF_CORE_READ_INTO(&dst, ctx, daddr);
    __u32 source = ((__u32)src[0] << 24) | ((__u32)src[1] << 16) |
                   ((__u32)src[2] << 8) | src[3];
    __u32 destination = ((__u32)dst[0] << 24) | ((__u32)dst[1] << 16) |
                        ((__u32)dst[2] << 8) | dst[3];
    if (source != SOURCE_IPV4 || destination != DESTINATION_IPV4)
        return 0;

    __u32 zero = 0;
    struct capture_counts *stats = bpf_map_lookup_elem(&counts, &zero);
    if (!stats)
        return 0;
    __u64 slot = __sync_fetch_and_add(&stats->total, 1);
    if (slot >= 128) {
        __sync_fetch_and_add(&stats->dropped, 1);
        return 0;
    }
    __u32 key = slot;
    struct capture_event value = {};
    struct capture_event *event = &value;
    event->monotonic_ns = bpf_ktime_get_ns();
    event->cookie = cookie;
    event->netns = BPF_CORE_READ(sk, __sk_common.skc_net.net, ns.inum);
    event->cpu = bpf_get_smp_processor_id();
    event->sport = ctx->sport;
    event->dport = ctx->dport;
    event->family = ctx->family;
    event->old_state = ctx->oldstate;
    event->new_state = ctx->newstate;
    __builtin_memcpy(event->saddr, src, sizeof(src));
    __builtin_memcpy(event->daddr, dst, sizeof(dst));
    event->sequence = slot + 1;
    if (bpf_map_update_elem(&events, &key, event, BPF_NOEXIST))
        __sync_fetch_and_add(&stats->dropped, 1);
    return 0;
}

char LICENSE[] SEC("license") = "GPL";
