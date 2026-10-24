//! Product root publication after the original native journal boundary.

use leselang_runtime_core::Fault;

use crate::reference_group_start::PreparedGroupStart;
use crate::{
    ContinuationImage, ContinuationToken, EffectRequest, MergePlan, Step, StructuredEffectBatch,
};

/// Trusted owner of live admission, graph validation, storage and local caches.
/// Commit hooks must recheck admission and commit the original whole request or
/// graph before returning success. A prepared request is not a dispatch grant.
///
/// Cache publication copies the borrowed image using the owner's existing rules.
/// No hook is retried or compensated. A commit error/unwind may have native
/// effects; a cache-hook unwind after commit can leave a published prefix. This
/// adapter adds no transaction around native storage and in-memory publication.
pub trait ReferenceRootPublicationHost {
    fn commit_atomic(&mut self, request: &EffectRequest) -> Result<(), Fault>;
    fn commit_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault>;
    fn publish_pending(&mut self, image: &ContinuationImage);
}

/// The original owned request becomes visible only after native commit and cache
/// publication. Native validation remains authoritative; this is not ingress.
pub fn publish_atomic<Host: ReferenceRootPublicationHost + ?Sized>(
    request: EffectRequest,
    host: &mut Host,
) -> Step {
    if let Err(error) = host.commit_atomic(&request) {
        return Step::Fault(error);
    }
    host.publish_pending(&request.continuation);
    Step::Effect(Box::new(request))
}

// Kept private to the adapter: only successful group preparation supplies this
// path, including its nonempty sequential-branch invariant.
pub(crate) fn publish_group<Host: ReferenceRootPublicationHost + ?Sized>(
    prepared: PreparedGroupStart,
    host: &mut Host,
) -> Step {
    let PreparedGroupStart {
        merge_token,
        plan,
        branches: mut named,
    } = prepared;
    let order = plan.order;
    let graph = named
        .iter()
        .map(|branch| (branch.branch.as_str(), &branch.request))
        .collect::<Vec<_>>();
    if let Err(error) = host.commit_group(&merge_token, &plan, &graph) {
        return Step::Fault(error);
    }
    for branch in &named {
        host.publish_pending(&branch.request.continuation);
    }
    if order.is_sequential() {
        return Step::Effect(Box::new(named.remove(0).request));
    }
    Step::Effects(Box::new(StructuredEffectBatch {
        merge_token,
        branches: named,
    }))
}
