#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

/* Client-side IPv4 loopback only. No raw payload is copied to a map. */
struct prefix {
    __u32 origin, local_port;
    __u64 conflicting_origin, sealed, pieces, slots[13];
};
struct counts { __u64 no_origin, no_cookie, capacity, invalid; };
_Static_assert(sizeof(struct prefix) == 136, "HTTP prefix ABI");
_Static_assert(sizeof(struct counts) == 32, "HTTP counter ABI");
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 64);
    __type(key, __u64);
    __type(value, struct prefix);
} prefixes SEC(".maps");
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct counts);
} counts SEC(".maps");

static __always_inline __u16 be16(const __u8 *p) { return ((__u16)p[0] << 8) | p[1]; }
static __always_inline __u32 be32(const __u8 *p) { return ((__u32)be16(p) << 16) | be16(p + 2); }

SEC("cgroup_skb/ingress")
int http_status_in(struct __sk_buff *ctx) {
    __u8 ip[20], tcp[20];
    if (ctx->ifindex != 1 || bpf_skb_load_bytes(ctx, 0, ip, sizeof(ip)) || ip[0] >> 4 != 4 ||
        ip[9] != 6 || ip[12] != 127 || ip[16] != 127 || (be16(ip+6) & 0x3fff)) return 1;
    __u32 ihl = (ip[0] & 15) * 4;
    if (ihl < 20 || bpf_skb_load_bytes(ctx, ihl, tcp, sizeof(tcp)) || be16(tcp) != HTTP_PORT) return 1;
    __u32 zero = 0;
    struct counts *stats = bpf_map_lookup_elem(&counts, &zero);
    if (!stats) return 1;
    __u32 header = ihl + (tcp[12] >> 4) * 4, total = be16(ip+2);
    if ((tcp[12] >> 4) < 5 || total < header || total > ctx->len) goto invalid;
    __u32 len = total - header, seq = be32(tcp+4);
    int synack = (tcp[13] & 0x17) == 0x12;
    int closing = tcp[13] & 5;
    if (!len && !synack && !closing) return 1;
    __u64 cookie = bpf_get_socket_cookie(ctx);
    if (!cookie) { __sync_fetch_and_add(&stats->no_cookie, 1); return 1; }
    struct prefix *row = bpf_map_lookup_elem(&prefixes, &cookie);
    if (synack) {
        if (len) goto invalid; /* TCP Fast Open is not an observed plain origin. */
        if (!row) {
            struct prefix start = {.origin = seq + 1, .local_port = be16(tcp+2)};
            if (!bpf_map_update_elem(&prefixes, &cookie, &start, BPF_NOEXIST)) return 1;
            row = bpf_map_lookup_elem(&prefixes, &cookie);
            if (!row) { __sync_fetch_and_add(&stats->capacity, 1); return 1; }
        }
        if (row->origin != seq+1) __sync_fetch_and_or(&row->conflicting_origin, 1);
        return 1;
    }
    if (!row) {
        if (len) __sync_fetch_and_add(&stats->no_origin, 1);
        return 1;
    }
    if (row->sealed) return 1;
    /* A reset is not stream data. Do not interpret SYN/urgent/non-ACK data. */
    if ((tcp[13] & 0x26) || !(tcp[13] & 0x10)) {
        if (len) __sync_fetch_and_add(&stats->invalid, 1);
        if (closing) __sync_fetch_and_or(&row->sealed, 1);
        return 1;
    }
    /* End the evidence window well before TCP sequence numbers can lap the origin. */
    if (seq - row->origin > 65535 && seq - row->origin < 0x80000000U) {
        __sync_fetch_and_or(&row->sealed, 1);
        return 1;
    }
    int contributed = 0;
#pragma clang loop unroll(full)
    for (int i = 0; i < 13; i++) {
        __u32 offset = row->origin + i - seq;
        if (offset >= len) continue;
        __u8 ch;
        if (bpf_skb_load_bytes(ctx, header + offset, &ch, 1)) goto invalid;
        /* Literal match, digit bit, or invalid. Never retain an arbitrary byte. */
        __u64 value = 1024;
        if (i >= 9 && i <= 11) {
            if (ch >= '0' && ch <= '9') value = 1ULL << (ch - '0');
        } else if (ch == (i == 12 ? ' ' : "HTTP/1.1 "[i])) value = 1;
        __sync_fetch_and_or(&row->slots[i], value);
        contributed = 1;
    }
    if (contributed) __sync_fetch_and_add(&row->pieces, 1);
    if (closing) __sync_fetch_and_or(&row->sealed, 1);
    return 1;
invalid:
    __sync_fetch_and_add(&stats->invalid, 1);
    return 1;
}
char LICENSE[] SEC("license") = "GPL";
