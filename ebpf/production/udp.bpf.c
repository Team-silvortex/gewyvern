#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

#if OFFLOAD_CORE
#include <bpf/bpf_core_read.h>
struct sk_buff { unsigned char *head; unsigned int end; }
    __attribute__((preserve_access_index));
struct skb_shared_info { unsigned int gso_type; }
    __attribute__((preserve_access_index));
extern void *bpf_cast_to_kern_ctx(void *ctx) __ksym __weak;
extern void *bpf_rdonly_cast(const void *obj, __u32 btf_id) __ksym __weak;
#endif

#define MAX_SEGMENTS 16
/* Private product ABI v2. Observe only; all paths allow original traffic. */
struct event {
    __u64 monotonic_ns, cookie, netns_cookie;
    __u32 cpu, ifindex, total_len;
    __u16 sport, dport, payload_len, prefix_len;
    __u8 source[16], destination[16];
    __u8 version, ip_version, ip_header_len, outbound;
    __u8 prefix[128];
    __u64 aggregate_id;
    __u32 aggregate_payload_len, segment_size, segment_index, segment_count;
};
_Static_assert(sizeof(struct event) == 232, "private event ABI size");
_Static_assert(__builtin_offsetof(struct event, aggregate_id) == 208, "private event ABI provenance");
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
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, __u64);
} aggregate_ids SEC(".maps");
static __always_inline int udp_l4_offload(struct __sk_buff *ctx) {
#if OFFLOAD_CORE
    if (bpf_ksym_exists(bpf_cast_to_kern_ctx) && bpf_ksym_exists(bpf_rdonly_cast)) {
        struct sk_buff *skb = bpf_cast_to_kern_ctx(ctx);
        if (!bpf_core_field_exists(skb->head) || !bpf_core_field_exists(skb->end) ||
            bpf_core_field_size(skb->end) != sizeof(__u32)) return 0;
        struct skb_shared_info *info = bpf_rdonly_cast(skb->head + skb->end,
            bpf_core_type_id_kernel(struct skb_shared_info));
        /* SKB_GSO_UDP_L4 in linux/skbuff.h. Exact equality excludes UFO,
         * tunnels, DODGY, PARTIAL and FRAGLIST, not just non-UDP traffic. */
        return bpf_core_field_exists(info->gso_type) && info->gso_type == (1U << 17);
    }
#endif
    return 0;
}
static __always_inline __u64 aggregate_id(void) {
    __u32 zero = 0;
    __u64 *next = bpf_map_lookup_elem(&aggregate_ids, &zero);
    if (!next) return 0;
    /* Bounded CAS never wraps/reuses IDs, even after exhaustion. Contention
     * beyond this retry budget is an explicit unsupported observation. */
    for (int attempt = 0; attempt < 16; attempt++) {
        __u64 old = *next;
        if (old >= 0x7fffffffffffffffULL) return 0;
        if (__sync_val_compare_and_swap(next, old, old + 1) == old) return old + 1;
    }
    return 0;
}
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
    /* Port admission precedes offload admission: unrelated aggregates must
     * not degrade a selected-port capture. Geometry alone is not a type. */
    __u32 size = 0, segments = 1, payload = total - header - 8;
    __u64 group = 0;
    if (ctx->gso_size || ctx->gso_segs > 1) {
        if (!udp_l4_offload(ctx)) goto unsupported;
        size = ctx->gso_size;
        segments = ctx->gso_segs;
        if (!size || size > 65535 || payload <= size || segments < 2 ||
            segments > MAX_SEGMENTS || segments != (payload + size - 1) / size)
            goto unsupported;
        group = aggregate_id();
        if (!group) goto unsupported;
    }
    if (total != ctx->len || be16(udp + 4) != total - header || !be16(udp) || !be16(udp + 2))
        goto malformed;
    __u64 cookie = bpf_get_socket_cookie(ctx), netns = bpf_get_netns_cookie(ctx);
    if (!cookie || !netns) { __sync_fetch_and_add(&stats->no_cookie, 1); return 1; }
    __u64 observed = bpf_ktime_get_ns();
    __sync_fetch_and_add(&stats->matched, segments);
    for (__u32 index = 0; index < MAX_SEGMENTS; index++) {
        if (index >= segments) break;
        struct event *value = bpf_ringbuf_reserve(&events, sizeof(*value), 0);
        if (!value) {
            __sync_fetch_and_add(&stats->ring_dropped, segments - index);
            return 1;
        }
        __builtin_memset(value, 0, sizeof(*value));
        value->version = 2;
        value->monotonic_ns = observed;
        value->cookie = cookie;
        value->netns_cookie = netns;
        value->cpu = bpf_get_smp_processor_id();
        value->ifindex = ctx->ifindex;
        __u32 offset = index * size;
        __u32 part = payload - offset;
        if (group && part > size) part = size;
        value->total_len = header + 8 + part;
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
        value->payload_len = part;
        if (group) {
            value->aggregate_id = group;
            value->aggregate_payload_len = payload;
            value->segment_size = size;
            value->segment_index = index;
            value->segment_count = segments;
        }
        __u32 length = value->payload_len > 128 ? 128 : value->payload_len;
        __asm__ volatile("" : "+r"(length));
        value->prefix_len = length;
        if (length && (length > 128 || bpf_skb_load_bytes(ctx, header + 8 + offset, value->prefix, length))) {
            bpf_ringbuf_discard(value, 0);
            __sync_fetch_and_add(&stats->ring_dropped, segments - index);
            __sync_fetch_and_add(&stats->malformed, 1);
            return 1;
        }
        bpf_ringbuf_submit(value, 0);
    }
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
