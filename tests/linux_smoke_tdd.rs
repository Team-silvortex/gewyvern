#[cfg(target_os = "linux")]
use gewyvern::loader::{linux_tracepoint_smoke_failures, LINUX_SMOKE_FRAGMENT_ID};

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn linux_tracepoint_attach_smoke() {
    let failures =
        linux_tracepoint_smoke_failures("syscalls/sys_enter_nanosleep").unwrap();
    assert!(failures.is_empty(), "linux attach smoke should succeed");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn linux_tracepoint_attach_failure_becomes_structured_record() {
    let failures =
        linux_tracepoint_smoke_failures("syscalls/definitely_missing_smoke_event").unwrap();

    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].fragment_id, LINUX_SMOKE_FRAGMENT_ID);
    assert_eq!(failures[0].hookpoint.label(), "tracepoint:syscalls/definitely_missing_smoke_event");
    assert!(!failures[0].error.is_empty());
}
