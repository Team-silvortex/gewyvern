#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("tracepoint/syscalls/sys_enter_nanosleep")
int handle_sys_enter(void *ctx) {
    return 0;
}

char LICENSE[] SEC("license") = "GPL";
