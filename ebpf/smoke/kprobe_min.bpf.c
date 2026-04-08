#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("kprobe/ip_route_output_flow")
int handle_kprobe(void *ctx) {
    return 0;
}

char LICENSE[] SEC("license") = "GPL";
