//! Optional validator-only System accounting; no pointer maps or allocation in callbacks.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Snapshot {
    pub live_requested_bytes: u64,
    pub live_allocations: u64,
}

pub fn snapshot() -> Option<Snapshot> {
    #[cfg(feature = "capture-allocation-audit")]
    if instrumented::ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        return Some(instrumented::snapshot());
    }
    None
}

#[cfg(feature = "capture-allocation-audit")]
pub use instrumented::{AuditSystem, activate};

#[cfg(feature = "capture-allocation-audit")]
mod instrumented {
    use super::Snapshot;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

    pub(super) static ACTIVE: AtomicBool = AtomicBool::new(false);
    static BYTES: AtomicU64 = AtomicU64::new(0);
    static BLOCKS: AtomicU64 = AtomicU64::new(0);

    pub fn activate() {
        // Accounting starts with the process, not here; this marks the owning binary.
        ACTIVE.store(true, Relaxed);
    }

    pub(super) fn snapshot() -> Snapshot {
        Snapshot {
            live_requested_bytes: BYTES.load(Relaxed),
            live_allocations: BLOCKS.load(Relaxed),
        }
    }

    pub struct AuditSystem;

    // SAFETY: all pointers and layouts are forwarded unchanged to System. Only
    // successful allocations/reallocations affect counters; callbacks never allocate.
    unsafe impl GlobalAlloc for AuditSystem {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: forwarded from GlobalAlloc's caller.
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                BYTES.fetch_add(layout.size() as u64, Relaxed);
                BLOCKS.fetch_add(1, Relaxed);
            }
            ptr
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: forwarded from GlobalAlloc's caller.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                BYTES.fetch_add(layout.size() as u64, Relaxed);
                BLOCKS.fetch_add(1, Relaxed);
            }
            ptr
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: forwarded from GlobalAlloc's caller.
            unsafe { System.dealloc(ptr, layout) };
            BYTES.fetch_sub(layout.size() as u64, Relaxed);
            BLOCKS.fetch_sub(1, Relaxed);
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            // SAFETY: forwarded from GlobalAlloc's caller.
            let next = unsafe { System.realloc(ptr, layout, size) };
            if !next.is_null() {
                if size >= layout.size() {
                    BYTES.fetch_add((size - layout.size()) as u64, Relaxed);
                } else {
                    BYTES.fetch_sub((layout.size() - size) as u64, Relaxed);
                }
            }
            next
        }
    }

    #[test]
    fn direct_system_witness_balances_growth_shrink_and_zeroed_allocations() {
        let before = snapshot();
        let small = Layout::from_size_align(64, 8).unwrap();
        let large = Layout::from_size_align(512, 8).unwrap();
        // SAFETY: allocations are checked and each successful pointer is used
        // with its exact current layout, then freed once.
        unsafe {
            let p = AuditSystem.alloc(small);
            assert!(!p.is_null());
            assert_eq!(
                snapshot().live_requested_bytes,
                before.live_requested_bytes + 64
            );
            let p = AuditSystem.realloc(p, small, 512);
            assert!(!p.is_null());
            assert_eq!(
                snapshot().live_requested_bytes,
                before.live_requested_bytes + 512
            );
            let p = AuditSystem.realloc(p, large, 64);
            assert!(!p.is_null());
            AuditSystem.dealloc(p, small);
            let p = AuditSystem.alloc_zeroed(small);
            assert!(!p.is_null());
            assert!(std::slice::from_raw_parts(p, 64).iter().all(|b| *b == 0));
            AuditSystem.dealloc(p, small);
        }
        // Ownership may cross threads; counters must not be thread-local.
        // SAFETY: the allocation is checked, then transferred for one matching free.
        let address = unsafe { AuditSystem.alloc(small) } as usize;
        assert_ne!(address, 0);
        std::thread::spawn(move || {
            // SAFETY: ownership of this allocation moved to this thread; layout matches.
            unsafe { AuditSystem.dealloc(address as *mut u8, small) };
        })
        .join()
        .unwrap();
        assert_eq!(snapshot().live_requested_bytes, before.live_requested_bytes);
        assert_eq!(snapshot().live_allocations, before.live_allocations);
    }
}
