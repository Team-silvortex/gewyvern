use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_command::reference_group_start::{
    GroupStartContext, ReferenceGroupStartHost, start_group,
};
use leselang_command::reference_publication::{ReferenceRootPublicationHost, publish_atomic};
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectRequest, ExecutionOrder, MergePlan, ResultBinding,
    Step,
};
use leselang_hir::{Effect, HirBranch, Type};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::Fault;

#[path = "support/group_start_fixture.rs"]
mod fixture;

#[derive(Default)]
struct Host {
    trace: Vec<String>,
    next: u64,
    native_effects: usize,
    pending: BTreeMap<ContinuationToken, ContinuationImage>,
    images: Vec<*const ContinuationImage>,
    effect_ids: Vec<*const u8>,
    plan: Option<MergePlan>,
    binding_pointer: Option<*const ResultBinding>,
    error: Option<Fault>,
    unwind_commit: bool,
    unwind_publication: Option<usize>,
    publications: usize,
}
impl Host {
    fn new() -> Self {
        Self {
            next: 10,
            ..Self::default()
        }
    }
    fn commit(&mut self) -> Result<(), Fault> {
        assert!(self.pending.is_empty());
        self.native_effects += 1;
        assert!(!self.unwind_commit, "native commit unwind");
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        Ok(())
    }
}
impl ReferenceGroupStartHost for Host {
    fn check_admission(&mut self, additional: usize) -> Result<(), Fault> {
        self.trace.push(format!("admit:{additional}"));
        Ok(())
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        self.trace.push(format!("reserve:{}", self.next));
        let next = self.next;
        self.next += 1;
        Ok(next)
    }
}
impl ReferenceRootPublicationHost for Host {
    fn commit_atomic(&mut self, request: &EffectRequest) -> Result<(), Fault> {
        self.trace.push("commit:atomic".into());
        self.images.push(std::ptr::from_ref(&request.continuation));
        self.effect_ids.push(request.effect_id.as_ptr());
        self.commit()
    }
    fn commit_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        self.trace.push("commit:group".into());
        assert_eq!(token.as_str(), "merge-10");
        assert_eq!(
            graph.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            plan.result_binding
                .as_ref()
                .map(|b| std::ptr::from_ref(&*b.binding)),
            self.binding_pointer
        );
        self.plan = Some(plan.clone());
        for (_, request) in graph {
            self.images.push(std::ptr::from_ref(&request.continuation));
            self.effect_ids.push(request.effect_id.as_ptr());
        }
        self.commit()
    }
    fn publish_pending(&mut self, image: &ContinuationImage) {
        assert_eq!(self.native_effects, 1);
        assert_eq!(std::ptr::from_ref(image), self.images[self.publications]);
        self.trace.push(format!("publish:{}", image.token.as_str()));
        self.pending.insert(image.token.clone(), image.clone());
        let index = self.publications;
        self.publications += 1;
        assert_ne!(self.unwind_publication, Some(index), "native cache unwind");
    }
}

fn branches() -> [HirBranch; 2] {
    ["a", "b"].map(|name| HirBranch {
        name: name.into(),
        effect: Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: Type::UiFocus,
    })
}
fn context() -> GroupStartContext {
    GroupStartContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: Some(Revision(7)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_100),
        fuel_remaining: 100,
    }
}
fn request() -> EffectRequest {
    let context = context();
    build_effect_request(ReservedEffectInput {
        effect: &branches()[0].effect,
        result_type: Type::UiFocus,
        sequence: 10,
        principal: context.principal,
        capabilities: context.capabilities,
        expected_revision: context.expected_revision,
        deadline_ms: context.deadline_ms,
        deadline_at_ms: context.deadline_at_ms,
        fuel_remaining: context.fuel_remaining,
    })
    .unwrap()
}
fn group(host: &mut Host, sequential: bool, kind: usize) -> Step {
    let binding = fixture::binding(kind);
    host.binding_pointer = binding.as_deref().map(std::ptr::from_ref);
    start_group(
        &branches(),
        if sequential {
            ExecutionOrder::Sequential
        } else {
            ExecutionOrder::Parallel
        },
        context(),
        binding,
        host,
    )
}
fn fault() -> Fault {
    Fault {
        code: "NATIVE".into(),
        message: "original native failure".into(),
    }
}

#[test]
fn atomic_commit_and_cache_borrow_the_owned_request_before_visible_handoff() {
    let mut host = Host::new();
    let request = request();
    let wire = serde_json::to_vec(&request).unwrap();
    let pointer = request.effect_id.as_ptr();
    let Step::Effect(visible) =
        publish_atomic(request, &mut host as &mut dyn ReferenceRootPublicationHost)
    else {
        panic!()
    };
    assert_eq!(host.trace, ["commit:atomic", "publish:continuation-10"]);
    assert_eq!(visible.effect_id.as_ptr(), pointer);
    assert_eq!(host.effect_ids, [pointer]);
    assert_eq!(serde_json::to_vec(&*visible).unwrap(), wire);
    assert_eq!(
        host.pending.get(&visible.continuation.token),
        Some(&visible.continuation)
    );
    assert_eq!(host.next, 10, "publication must not allocate identities");
}

#[test]
fn all_twelve_group_shapes_commit_once_then_publish_every_prefix_in_order() {
    for sequential in [false, true] {
        for kind in 0..6 {
            let mut host = Host::new();
            let step = group(&mut host, sequential, kind);
            let mut trace = vec!["admit:2".to_string()];
            trace.extend((10..host.next).map(|n| format!("reserve:{n}")));
            trace.extend(
                [
                    "commit:group",
                    "publish:continuation-11",
                    "publish:continuation-12",
                ]
                .map(str::to_string),
            );
            assert_eq!(host.trace, trace);
            assert_eq!(host.native_effects, 1);
            assert_eq!(host.pending.len(), 2);
            assert_eq!(
                host.plan.as_ref().unwrap().order.is_sequential(),
                sequential
            );
            match step {
                Step::Effect(first) => {
                    assert!(sequential);
                    assert_eq!(first.effect_id, "effect-11");
                    assert_eq!(first.effect_id.as_ptr(), host.effect_ids[0]);
                    assert_eq!(host.pending[&first.continuation.token], first.continuation);
                }
                Step::Effects(batch) => {
                    assert!(!sequential);
                    assert_eq!(batch.merge_token.as_str(), "merge-10");
                    for (i, branch) in batch.branches.iter().enumerate() {
                        assert_eq!(branch.request.effect_id.as_ptr(), host.effect_ids[i]);
                        assert_eq!(
                            host.pending[&branch.request.continuation.token],
                            branch.request.continuation
                        );
                    }
                }
                step => panic!("unexpected {step:?}"),
            }
        }
    }
}

#[test]
fn preparation_refusal_never_reaches_commit_or_cache_and_keeps_consumed_ids() {
    for defect in 0..4 {
        let mut host = Host::new();
        let mut branches = branches().to_vec();
        match defect {
            0 => branches.clear(),
            1 => branches[1].name = "a".into(),
            2 => branches[1].result_type = Type::Structured,
            3 => branches[0].effect = Effect::All { branches: vec![] },
            _ => unreachable!(),
        }
        assert!(matches!(
            start_group(
                &branches,
                ExecutionOrder::Sequential,
                context(),
                None,
                &mut host
            ),
            Step::Fault(_)
        ));
        assert!(host.pending.is_empty());
        assert_eq!(host.native_effects, 0);
        assert_eq!(host.next, if defect == 2 { 13 } else { 10 });
    }
}

#[test]
fn commit_errors_keep_original_owned_fault_without_cache_or_native_compensation() {
    for mode in 0..13 {
        let mut host = Host::new();
        let error = fault();
        let pointer = error.message.as_ptr();
        host.error = Some(error);
        let result = if mode == 0 {
            publish_atomic(request(), &mut host)
        } else {
            group(&mut host, mode > 6, (mode - 1) % 6)
        };
        let Step::Fault(error) = result else { panic!() };
        assert_eq!(error, fault());
        assert_eq!(error.message.as_ptr(), pointer);
        assert!(host.pending.is_empty());
        assert_eq!(host.native_effects, 1);
        assert_eq!(host.publications, 0);
        assert_eq!(host.next == 10, mode == 0);
    }
}

#[test]
fn commit_unwinds_stop_publication_without_retry_or_rollback() {
    for atomic in [false, true] {
        let mut host = Host::new();
        host.unwind_commit = true;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if atomic {
                    publish_atomic(request(), &mut host)
                } else {
                    group(&mut host, true, 4)
                }
            }))
            .is_err()
        );
        assert_eq!(host.native_effects, 1);
        assert_eq!(host.publications, 0);
        assert!(host.pending.is_empty());
    }
}

#[test]
fn cache_unwinds_keep_the_committed_native_work_and_original_published_prefix() {
    for (atomic, stop) in [(true, 0), (false, 0), (false, 1)] {
        let mut host = Host::new();
        host.unwind_publication = Some(stop);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if atomic {
                    publish_atomic(request(), &mut host)
                } else {
                    group(&mut host, false, 4)
                }
            }))
            .is_err()
        );
        assert_eq!(host.native_effects, 1);
        assert_eq!(host.publications, stop + 1);
        assert_eq!(host.pending.len(), stop + 1);
    }
}
