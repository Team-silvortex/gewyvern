use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    ir::GroupKind, native_group::NativeGroupMemberError, native_group_preparation::*,
};
use leselang_runtime_core::Fuel;

struct Branch {
    name: String,
    allowed: bool,
    secret: Box<u8>,
}
struct Private {
    marker: u8,
    text: &'static str,
}
#[derive(Default)]
struct Trace {
    events: RefCell<Vec<(&'static str, usize)>>,
    drops: Cell<usize>,
    drop_panic: Cell<bool>,
}
struct Prepared {
    buffer: Box<u8>,
    trace: Rc<Trace>,
}
impl Drop for Prepared {
    fn drop(&mut self) {
        self.trace.drops.set(self.trace.drops.get() + 1);
        assert!(
            !self.trace.drop_panic.replace(false),
            "native preparation cleanup unwind"
        );
    }
}
fn branches(count: usize) -> Vec<Branch> {
    (0..count)
        .map(|index| Branch {
            name: format!("m_{index}"),
            allowed: true,
            secret: Box::new(9),
        })
        .collect()
}
fn private() -> Private {
    Private {
        marker: 7,
        text: "private native input",
    }
}

#[test]
fn original_member_slots_and_owned_preparations_are_linked_once_without_native_trait_bounds() {
    let members = branches(2);
    let trace = Rc::new(Trace::default());
    let mut fuel = Fuel::new(20);
    let group = prepare_native_group_members(
        GroupKind::Parallel,
        &members,
        2,
        &mut fuel,
        |branch| {
            trace.events.borrow_mut().push(("name", branch.name.len()));
            Ok(branch.name.as_str())
        },
        |branch| {
            trace
                .events
                .borrow_mut()
                .push(("admit", *branch.secret as usize));
            Ok(())
        },
        |index, branch, fuel| {
            trace.events.borrow_mut().push(("prepare", index));
            fuel.charge(2).map_err(|_| private())?;
            Ok::<_, Private>(Prepared {
                buffer: Box::new(*branch.secret),
                trace: trace.clone(),
            })
        },
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 13);
    assert_eq!(
        *trace.events.borrow(),
        [
            ("name", 3),
            ("name", 3),
            ("admit", 9),
            ("admit", 9),
            ("prepare", 0),
            ("prepare", 1)
        ]
    );
    assert!(std::ptr::eq(group.metadata().members(), &members[..]));
    assert_eq!(
        format!("{group:?}"),
        "PreparedNativeGroup { kind: Parallel, members: 2 }"
    );
    let pointers: Vec<_> = group
        .members()
        .iter()
        .map(|m| (&*m.prepared().buffer as *const u8) as usize)
        .collect();
    for (index, row) in group.members().iter().enumerate() {
        assert_eq!(row.index(), index);
        assert!(std::ptr::eq(row.original(), &members[index]));
        assert_eq!(
            group.metadata().name(index).unwrap().as_ptr(),
            members[index].name.as_ptr()
        );
        assert_eq!(
            format!("{row:?}"),
            format!("PreparedNativeMember {{ index: {index} }}")
        );
    }
    let (metadata, rows) = group.into_parts();
    assert_eq!(metadata.kind(), GroupKind::Parallel);
    for (index, row) in rows.into_iter().enumerate() {
        let (position, original, prepared) = row.into_parts();
        assert_eq!(position, index);
        assert!(std::ptr::eq(original, &members[index]));
        assert_eq!((&*prepared.buffer as *const u8) as usize, pointers[index]);
    }
    assert_eq!(trace.drops.get(), 2);
}

#[test]
fn complete_cold_names_and_native_admission_precede_every_prepare_and_fuel_debit() {
    for scenario in 0..5 {
        let mut members = branches(2);
        match scenario {
            0 => members[1].name = "bad name".into(),
            1 => members[1].name = members[0].name.clone(),
            2 => members[1].allowed = false,
            _ => {}
        }
        let names = Cell::new(0);
        let admissions = Cell::new(0);
        let prepares = Cell::new(0);
        let mut fuel = Fuel::new(20);
        let result = prepare_native_group_members(
            GroupKind::Parallel,
            &members,
            if scenario == 3 {
                1
            } else if scenario == 4 {
                65
            } else {
                2
            },
            &mut fuel,
            |branch| {
                names.set(names.get() + 1);
                Ok(branch.name.as_str())
            },
            |branch| {
                admissions.set(admissions.get() + 1);
                if branch.allowed {
                    Ok(())
                } else {
                    Err(private())
                }
            },
            |_, _, _| {
                prepares.set(prepares.get() + 1);
                Ok::<_, Private>(())
            },
        );
        assert!(matches!(
            result,
            Err(NativeGroupPreparationError::Admission(_))
        ));
        assert_eq!(prepares.get(), 0);
        assert_eq!(fuel.remaining(), 20);
        assert_eq!(names.get(), if scenario < 3 { 2 } else { 0 });
        assert_eq!(admissions.get(), if scenario == 2 { 2 } else { 0 });
        if scenario == 0 {
            assert!(matches!(
                result,
                Err(NativeGroupPreparationError::Admission(
                    NativeGroupMemberError::InvalidName { member_index: 1 }
                ))
            ));
        }
    }
}

#[test]
fn root_and_each_member_charge_share_one_original_meter_and_drop_exhausted_prefixes() {
    for units in 0..4 {
        let members = branches(2);
        let trace = Rc::new(Trace::default());
        let mut fuel = Fuel::new(units);
        let result = prepare_native_group_members(
            GroupKind::Parallel,
            &members,
            2,
            &mut fuel,
            |b| Ok(b.name.as_str()),
            |_| Ok(()),
            |index, _, _| {
                trace.events.borrow_mut().push(("prepare", index));
                Ok::<_, Private>(Prepared {
                    buffer: Box::new(1),
                    trace: trace.clone(),
                })
            },
        );
        let expected = match units {
            0 => None,
            1 => Some(0),
            2 => Some(1),
            _ => {
                assert!(result.is_ok());
                drop(result);
                assert_eq!(trace.drops.get(), 2);
                continue;
            }
        };
        assert!(
            matches!(result, Err(NativeGroupPreparationError::Fuel { member_index }) if member_index == expected)
        );
        assert_eq!(fuel.remaining(), 0);
        assert_eq!(trace.drops.get(), usize::from(units == 2));
    }
}

#[test]
fn late_native_failure_or_unwind_drops_owned_prefix_without_partial_handoff_retry_or_refund() {
    for unwind in [false, true] {
        let members = branches(3);
        let trace = Rc::new(Trace::default());
        let mut fuel = Fuel::new(20);
        let result = catch_unwind(AssertUnwindSafe(|| {
            prepare_native_group_members(
                GroupKind::Parallel,
                &members,
                3,
                &mut fuel,
                |b| Ok(b.name.as_str()),
                |_| Ok(()),
                |index, _, fuel| {
                    trace.events.borrow_mut().push(("prepare", index));
                    fuel.charge(1).map_err(|_| private())?;
                    if index == 2 {
                        if unwind {
                            panic!("native callback unwind")
                        }
                        return Err(private());
                    }
                    Ok(Prepared {
                        buffer: Box::new(1),
                        trace: trace.clone(),
                    })
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            let error = result.unwrap().unwrap_err();
            assert!(!format!("{error:?}: {error}").contains("private"));
            assert!(std::error::Error::source(&error).is_none());
            let NativeGroupPreparationError::Native {
                member_index,
                error,
            } = error
            else {
                panic!()
            };
            assert_eq!(member_index, 2);
            assert_eq!(error.marker, 7);
            assert_eq!(error.text, "private native input");
        }
        assert_eq!(fuel.remaining(), 13);
        assert_eq!(trace.drops.get(), 2);
        assert_eq!(
            *trace.events.borrow(),
            [("prepare", 0), ("prepare", 1), ("prepare", 2)]
        );
    }
}

#[test]
fn prefix_cleanup_unwind_still_releases_all_other_preparations_without_returning_a_batch() {
    let members = branches(3);
    let trace = Rc::new(Trace::default());
    let mut fuel = Fuel::new(20);
    let result = catch_unwind(AssertUnwindSafe(|| {
        prepare_native_group_members(
            GroupKind::Parallel,
            &members,
            3,
            &mut fuel,
            |b| Ok(b.name.as_str()),
            |_| Ok(()),
            |index, _, _| {
                if index == 2 {
                    trace.drop_panic.set(true);
                    return Err(private());
                }
                Ok(Prepared {
                    buffer: Box::new(1),
                    trace: trace.clone(),
                })
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(trace.drops.get(), 2);
    assert_eq!(fuel.remaining(), 16);
}

#[test]
fn sequence_parallel_and_inclusive_sixty_four_member_ceiling_use_the_shared_original_name_gate() {
    for (kind, count) in [
        (GroupKind::Sequence, 1),
        (GroupKind::Parallel, 2),
        (GroupKind::Parallel, 64),
    ] {
        let members = branches(count);
        let mut fuel = Fuel::new(65);
        let group = prepare_native_group_members(
            kind,
            &members,
            count,
            &mut fuel,
            |b| Ok(b.name.as_str()),
            |_| Ok(()),
            |i, _, _| Ok::<_, Private>(i),
        )
        .unwrap();
        assert_eq!(group.members().len(), count);
        assert_eq!(fuel.remaining(), 64 - count as u64);
        for (i, row) in group.members().iter().enumerate() {
            assert_eq!(row.index(), i);
            assert_eq!(*row.prepared(), i);
        }
    }
}
