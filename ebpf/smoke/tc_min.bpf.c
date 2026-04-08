#include <linux/bpf.h>
#include <linux/pkt_cls.h>
#include <bpf/bpf_helpers.h>

SEC("classifier/tc_ingress")
int handle_tc_ingress(struct __sk_buff *skb) {
    return TC_ACT_OK;
}

char LICENSE[] SEC("license") = "GPL";
