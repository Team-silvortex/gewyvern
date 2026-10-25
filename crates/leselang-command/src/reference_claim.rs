//! Product dispatch-claim coordination, without queue or lease ownership.

use crate::{
    ContinuationImage, ContinuationToken, DispatchClaim, DispatchLease, SchedulerPressure,
};
use leselang_runtime_core::{Fault, validate_clock};
use std::collections::{BTreeMap, btree_map::Entry};

/// Trusted synchronous owner. Claim selection, fairness, capacity and lease
/// issuance remain atomic under the native journal owner. Idle and backpressure
/// must be determined there without materializing ineligible work. Adoption is a
/// local-cache operation after commit, not additional dispatch authority. Its
/// failure must not retry or roll back the already committed lease. Input bounds,
/// caller authority, clock provenance and consistent worker limits remain native.
pub trait ReferenceClaimHost {
    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault>;
    fn claim_dispatch(&mut self, now_ms: u64, lease_ms: u64) -> Result<DispatchClaim, Fault>;
    fn adopt_claimed_image(&mut self, image: &ContinuationImage) -> Result<(), Fault>;
}

/// Retains the optional-lease API and its exact backpressure diagnostic.
pub fn claim_effect<Host: ReferenceClaimHost + ?Sized>(
    now_ms: u64,
    lease_ms: u64,
    host: &mut Host,
) -> Result<Option<DispatchLease>, Fault> {
    match try_claim_effect(now_ms, lease_ms, host)? {
        DispatchClaim::Leased(lease) => Ok(Some(*lease)),
        DispatchClaim::Idle => Ok(None),
        DispatchClaim::Backpressured(pressure) => Err(lease_capacity_fault(pressure)),
    }
}

/// Validate clock and duration before cleanup, then adopt only a committed lease.
/// The original lease box and its payload owners are returned without cloning.
pub fn try_claim_effect<Host: ReferenceClaimHost + ?Sized>(
    now_ms: u64,
    lease_ms: u64,
    host: &mut Host,
) -> Result<DispatchClaim, Fault> {
    validate_clock(now_ms)?;
    crate::reference_dispatch::validate_lease_clock(now_ms, lease_ms)?;
    host.expire_due(now_ms)?;
    let claim = host.claim_dispatch(now_ms, lease_ms)?;
    if let DispatchClaim::Leased(lease) = &claim {
        host.adopt_claimed_image(&lease.request.continuation)?;
    }
    Ok(claim)
}

/// Adopt only an already committed native lease. An occupied cache entry keeps
/// its original buffers; only a vacant entry clones the borrowed claimed image.
/// A conflict does not revoke or retry the issued lease, touch completion state,
/// advance the allocation watermark, or authenticate the request.
pub fn adopt_claimed_image(
    pending: &mut BTreeMap<ContinuationToken, ContinuationImage>,
    image: &ContinuationImage,
) -> Result<(), Fault> {
    match pending.entry(image.token.clone()) {
        Entry::Occupied(entry) => validate_claimed_image(entry.get(), image)?,
        Entry::Vacant(entry) => {
            entry.insert(image.clone());
        }
    }
    Ok(())
}

/// Local correspondence only. The owner keeps its occupied entry unchanged and
/// clones a claimed image only for a vacant entry, after the journal has committed.
pub fn validate_claimed_image(
    stored: &ContinuationImage,
    claimed: &ContinuationImage,
) -> Result<(), Fault> {
    if stored != claimed {
        return Err(Fault {
            code: "LSV2005".into(),
            message: "claimed continuation conflicts with local pending state".into(),
        });
    }
    Ok(())
}

/// Maps the owner's pressure observation without recounting or reserving capacity.
pub fn lease_capacity_fault(pressure: SchedulerPressure) -> Fault {
    Fault {
        code: "LSV2502".into(),
        message: format!(
            "dispatch leasing is backpressured: {} active leases, limit {}",
            pressure.active_leases, pressure.limits.max_active_leases
        ),
    }
}
