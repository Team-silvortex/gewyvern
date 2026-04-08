#include <bpf/libbpf.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
    struct bpf_object *obj = NULL;
    struct bpf_program *prog = NULL;
    struct bpf_link *link = NULL;
    const char *path = NULL;
    const char *symbol = NULL;
    int err = 0;

    if (argc != 3) {
        fprintf(stderr, "usage: %s <bpf-object> <kprobe-symbol>\n", argv[0]);
        return 2;
    }

    path = argv[1];
    symbol = argv[2];
    obj = bpf_object__open_file(path, NULL);
    if (libbpf_get_error(obj)) {
        err = (int)libbpf_get_error(obj);
        fprintf(stderr, "open failed: %s\n", strerror(-err));
        return 1;
    }

    err = bpf_object__load(obj);
    if (err) {
        fprintf(stderr, "load failed: %s\n", strerror(-err));
        goto cleanup;
    }

    prog = bpf_object__find_program_by_name(obj, "handle_kprobe");
    if (!prog) {
        fprintf(stderr, "program lookup failed\n");
        err = -ENOENT;
        goto cleanup;
    }

    link = bpf_program__attach_kprobe(prog, false, symbol);
    if (libbpf_get_error(link)) {
        err = (int)libbpf_get_error(link);
        link = NULL;
        fprintf(stderr, "attach failed: %s\n", strerror(-err));
        goto cleanup;
    }

    puts("linux kprobe smoke ok");

cleanup:
    bpf_link__destroy(link);
    bpf_object__close(obj);
    return err ? 1 : 0;
}
