//! Test-only, thread-local accounting. The native capture executable uses System directly.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use serde_json::{Value, json};

#[derive(Clone, Copy, Default)]
pub(super) struct Counts {
    values: [u64; 5],
}

impl Counts {
    pub(super) fn since(self, previous: Self) -> Self {
        Self {
            values: std::array::from_fn(|i| self.values[i].saturating_sub(previous.values[i])),
        }
    }

    pub(super) fn add(&mut self, other: Self) {
        for (value, increment) in self.values.iter_mut().zip(other.values) {
            *value = value.saturating_add(increment);
        }
    }

    pub(in super::super) fn report(self) -> Value {
        json!({"alloc_calls":self.values[0],"dealloc_calls":self.values[1],
            "realloc_calls":self.values[2],"requested_bytes":self.values[3],"released_bytes":self.values[4]})
    }
}

thread_local! {
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
}

pub(super) fn snapshot() -> Counts {
    COUNTS.with(|cell| cell.get().unwrap_or_default())
}

pub(in super::super) struct Guard;

pub(in super::super) fn enable() -> Guard {
    COUNTS.with(|cell| {
        assert!(cell.get().is_none());
        cell.set(Some(Counts::default()));
    });
    Guard
}

impl Drop for Guard {
    fn drop(&mut self) {
        COUNTS.with(|cell| cell.set(None));
    }
}

fn count(values: [u64; 5]) {
    // No allocation, locking or panic is permitted inside an allocator callback.
    let _ = COUNTS.try_with(|cell| {
        if let Some(mut counts) = cell.get() {
            counts.add(Counts { values });
            cell.set(Some(counts));
        }
    });
}

struct CountingSystem;

#[global_allocator]
static ALLOCATOR: CountingSystem = CountingSystem;

// SAFETY: all allocations and layouts are passed unchanged to System.
unsafe impl GlobalAlloc for CountingSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded from the GlobalAlloc caller.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            count([1, 0, 0, layout.size() as u64, 0]);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded from the GlobalAlloc caller.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            count([1, 0, 0, layout.size() as u64, 0]);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count([0, 1, 0, 0, layout.size() as u64]);
        // SAFETY: forwarded from the GlobalAlloc caller.
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: forwarded from the GlobalAlloc caller.
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() {
            count([0, 0, 1, size as u64, layout.size() as u64]);
        }
        next
    }
}

#[test]
fn accounting_is_scoped_and_balances_owned_allocations() {
    let guard = enable();
    let mut values = Vec::<u64>::with_capacity(4);
    values.push(std::hint::black_box(42));
    values.reserve(64);
    std::hint::black_box(&values);
    drop(values);
    let counts = snapshot();
    drop(guard);
    assert_eq!(counts.values[0], 1);
    assert_eq!(counts.values[1], 1);
    assert_eq!(counts.values[2], 1);
    assert_eq!(counts.values[3], counts.values[4]);
    assert_eq!(snapshot().values, [0; 5]);
}
