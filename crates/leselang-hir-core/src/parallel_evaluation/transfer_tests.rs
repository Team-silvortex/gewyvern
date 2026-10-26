use super::*;

type Core<'a> = ParallelReplyCore<'a, (), (), Box<u8>, (), Box<u8>>;

struct Domain;
impl HostResultDomain<u8> for Domain {
    type Error = ();
    fn matches_type(&self, _: &u8) -> bool {
        true
    }
    fn validate_value(&self, _: &u8) -> Result<(), ()> {
        Ok(())
    }
}
impl ReplyAuthority<Box<u8>> for Domain {
    type Error = ();
    fn authorize(&self, _: &Box<u8>) -> Result<(), ()> {
        Ok(())
    }
}

fn branches() -> Vec<ComputedBranch<(), ()>> {
    (0..4)
        .map(|i| ComputedBranch {
            name: format!("m{i}"),
            value: (),
            result_type: (),
        })
        .collect()
}

#[test]
fn cold_and_terminal_parallel_cores_refuse_transfer_without_changing_state() {
    let branches = branches();
    let core = Core::new(&branches);
    let Err(core) = core.try_into_saved() else {
        panic!()
    };
    assert_eq!(core.status(), ParallelStatus::Ready);
    for end in [
        ParallelEnd::Completed,
        ParallelEnd::Cancelled,
        ParallelEnd::Failed,
        ParallelEnd::HostUncertain,
    ] {
        let core = Core {
            branches: &branches,
            state: State::Terminal(end),
        };
        let Err(core) = core.try_into_saved() else {
            panic!()
        };
        assert_eq!(core.status(), ParallelStatus::Terminal(end));
    }
}

#[test]
fn inconsistent_parallel_slot_returns_every_converted_prefix_and_untouched_suffix() {
    let branches = branches();
    let domain = ();
    for bad_index in 0..4 {
        for terminal in [false, true] {
            let mut pointers = Vec::new();
            let members = (0..4)
                .map(|i| {
                    let id = Box::new(i as u8);
                    pointers.push(&*id as *const u8);
                    let mut pending = PendingReply::new(id, (), &domain);
                    if i == bad_index {
                        pending.cancel();
                        if terminal {
                            Member::Pending(pending)
                        } else {
                            Member::Moving
                        }
                    } else {
                        Member::Pending(pending)
                    }
                })
                .collect();
            let core = Core {
                branches: &branches,
                state: State::Awaiting {
                    members,
                    remaining: 4,
                },
            };
            let Err(core) = core.try_into_saved() else {
                panic!()
            };
            assert_eq!(core.status(), ParallelStatus::Awaiting { remaining: 4 });
            let State::Awaiting { members, .. } = core.state else {
                panic!()
            };
            for (i, member) in members.into_iter().enumerate() {
                match member {
                    Member::Pending(pending) if i != bad_index => {
                        let (id, (), declaration) = pending.try_into_parts().unwrap();
                        assert_eq!(&*id as *const u8, pointers[i]);
                        assert_eq!(*id, i as u8);
                        assert!(std::ptr::eq(declaration, &domain));
                    }
                    Member::Pending(pending) => {
                        assert!(terminal);
                        assert_eq!(
                            pending.status(),
                            leselang_runtime_core::ReplyStatus::Terminal(ReplyEnd::Cancelled)
                        );
                    }
                    Member::Moving => {
                        assert_eq!(i, bad_index);
                        assert!(!terminal);
                    }
                    _ => panic!(),
                }
            }
        }
    }
}

#[test]
fn inconsistent_parallel_slot_preserves_original_accepted_reply_prefix() {
    let branches = branches();
    let domain = Domain;
    let mut pending = PendingReply::new(Box::new(7), (), &domain);
    let input = Box::new(19);
    let pointer = &*input as *const u8;
    let accepted = pending.try_accept(&Box::new(7), input, &domain).unwrap();
    let core: ParallelReplyCore<'_, (), (), Box<u8>, Domain, Box<u8>> = ParallelReplyCore {
        branches: &branches,
        state: State::Awaiting {
            members: vec![
                Member::Accepted(accepted),
                Member::Moving,
                Member::Moving,
                Member::Moving,
            ],
            remaining: 3,
        },
    };
    let Err(core) = core.try_into_saved() else {
        panic!()
    };
    assert_eq!(core.status(), ParallelStatus::Awaiting { remaining: 3 });
    let State::Awaiting { mut members, .. } = core.state else {
        panic!()
    };
    let Member::Accepted(accepted) = members.remove(0) else {
        panic!()
    };
    assert_eq!(&**accepted.as_parts().3 as *const u8, pointer);
    assert!(std::ptr::eq(accepted.as_parts().2, &domain));
    assert!(members.iter().all(|m| matches!(m, Member::Moving)));
}
