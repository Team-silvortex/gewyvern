use super::*;
use std::{cell::Cell, rc::Rc};

struct Native(Box<u8>, Rc<Cell<usize>>);
impl Drop for Native {
    fn drop(&mut self) {
        self.1.set(self.1.get() + 1);
    }
}
type Core<'a> = SequenceReplyCore<'a, (), (), Native, ()>;
fn branches() -> Vec<ComputedBranch<(), ()>> {
    (0..2)
        .map(|i| ComputedBranch {
            name: format!("m{i}"),
            value: (),
            result_type: (),
        })
        .collect()
}
fn native(drops: &Rc<Cell<usize>>, value: u8) -> Native {
    Native(Box::new(value), drops.clone())
}
fn prepared<'a>(
    drops: &Rc<Cell<usize>>,
    domain: &'a (),
    count: usize,
) -> VecDeque<PreparedSequenceMember<'a, Native, (), Native>> {
    (0..count)
        .map(|i| PreparedSequenceMember {
            identity: native(drops, i as u8),
            declaration: domain,
            request: native(drops, i as u8),
        })
        .collect()
}

#[test]
fn seq_transfer_needs_no_native_equality_or_payload_traits() {
    let branches = branches();
    let drops = Rc::new(Cell::new(0));
    let domain = ();
    let core = Core::new(&branches);
    let queued = prepared(&drops, &domain, 2);
    let pointers: Vec<_> = queued
        .iter()
        .map(|row| (&*row.identity.0 as *const u8, &*row.request.0 as *const u8))
        .collect();
    let prefix: Vec<SequenceAccepted<'_, (), (), Native, (), Native>> = Vec::new();
    let Ok(rows) = core.try_into_saved(queued, prefix) else {
        panic!()
    };
    assert_eq!(drops.get(), 0);
    for (index, row) in rows.into_iter().enumerate() {
        let SavedSequenceReply::Prepared(member) = row.reply else {
            panic!()
        };
        assert_eq!(
            (
                &*member.identity.0 as *const u8,
                &*member.request.0 as *const u8
            ),
            pointers[index]
        );
    }
    assert_eq!(drops.get(), 4);
}

#[test]
fn malformed_private_seq_frontiers_return_original_owners_without_indexing_or_rearming() {
    let branches = branches();
    let domain = ();
    for case in 0..8 {
        let drops = Rc::new(Cell::new(0));
        let next = match case {
            0 => usize::MAX,
            1 | 7 => 1,
            2 | 3 => 2,
            _ => 0,
        };
        let state = if matches!(case, 3 | 5 | 6) {
            let mut pending = PendingReply::new(native(&drops, 9), (), &domain);
            if case == 6 {
                pending.cancel();
            }
            State::Awaiting(pending)
        } else if case == 7 {
            State::Terminal(SequenceEnd::Completed)
        } else {
            State::Ready
        };
        let core = Core {
            branches: &branches,
            next,
            state,
        };
        let status = core.status();
        let queued = prepared(&drops, &domain, if matches!(case, 4 | 6) { 1 } else { 2 });
        let pointers: Vec<_> = queued
            .iter()
            .map(|row| &*row.request.0 as *const u8)
            .collect();
        let prefix: Vec<SequenceAccepted<'_, (), (), Native, (), Native>> = Vec::new();
        let before = drops.get();
        let Err((core, queued, prefix)) = core.try_into_saved(queued, prefix) else {
            panic!()
        };
        assert_eq!(core.status(), status);
        assert!(prefix.is_empty());
        assert_eq!(
            queued
                .iter()
                .map(|row| &*row.request.0 as *const u8)
                .collect::<Vec<_>>(),
            pointers
        );
        assert_eq!(drops.get(), before);
    }
}
