use std::fmt;

use crate::MAX_GROUP_GRAPH_MEMBERS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeOrder {
    Sequential,
    Parallel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeSummaryError {
    InvalidLimits,
    Closed,
    MemberLimit,
}

impl fmt::Display for OutcomeSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "outcome member limit exceeds safety ceiling",
            Self::Closed => "outcome summary pass is closed",
            Self::MemberLimit => "outcome summary member limit exceeded",
        })
    }
}
impl std::error::Error for OutcomeSummaryError {}

/// Mutable observations, not a complete group, authentic receipt or execution grant.
pub struct OutcomeSummary<Stop> {
    pub observed_members: usize,
    pub output_items: usize,
    pub first_stop: Option<Stop>,
}

/// Bounded accounting for original ordered, already-admitted native outcomes.
/// This owns no success payloads, identities, callbacks, journal or evaluator.
/// Hosts supply original value counts and terminal stops in declared order.
/// Pending rows are not observations. Limits bound observations, not completeness;
/// zero observations are allowed and the output limit is explicit native policy.
/// Counts saturate rather than wrap, matching bounded native output accounting.
///
/// Sequential mode creates an output stop at the first over-limit success only
/// when no earlier stop exists. Parallel mode creates it only at finish, replacing
/// any earlier stop. All other terminal observations retain the first stop.
/// Native factories own exact faults and bounded work. Stops need no Clone/Debug/
/// Eq/serde/Default/Send/Sync bounds; ignored stops and uncalled factories release
/// before handoff. Any refusal or native observation/cleanup unwind closes the
/// whole pass and releases partial state. A second cleanup panic may abort.
///
/// This is not settlement, receipt authenticity or permission to retry. Native
/// side effects cannot be rolled back or sandboxed. Hosts still validate values,
/// aggregate bounds, original graph/captures/authority and receipt correspondence,
/// and own same-snapshot storage commit before publishing recovery. No source or
/// saved body execution, fuel construction, dispatch, replay or storage writes.
#[must_use = "retain one summary pass; dropping it discards unhanded observations"]
pub struct OrderedOutcomeSummary<Stop> {
    staged: Option<OutcomeSummary<Stop>>,
    order: OutcomeOrder,
    max_members: usize,
    max_output_items: usize,
}

impl<Stop> OrderedOutcomeSummary<Stop> {
    pub fn new(
        order: OutcomeOrder,
        max_members: usize,
        max_output_items: usize,
    ) -> Result<Self, OutcomeSummaryError> {
        if max_members > MAX_GROUP_GRAPH_MEMBERS {
            return Err(OutcomeSummaryError::InvalidLimits);
        }
        Ok(Self {
            staged: Some(OutcomeSummary {
                observed_members: 0,
                output_items: 0,
                first_stop: None,
            }),
            order,
            max_members,
            max_output_items,
        })
    }

    pub fn has_stop(&self) -> Result<bool, OutcomeSummaryError> {
        self.staged
            .as_ref()
            .map(|state| state.first_stop.is_some())
            .ok_or(OutcomeSummaryError::Closed)
    }

    pub fn observe_success(
        &mut self,
        items: usize,
        overflow: impl FnOnce(usize, usize) -> Stop,
    ) -> Result<(), OutcomeSummaryError> {
        let mut staged = self.staged.take();
        let mut overflow = Some(overflow);
        let outcome = (|| {
            let state = staged.as_mut().ok_or(OutcomeSummaryError::Closed)?;
            if state.observed_members >= self.max_members {
                return Err(OutcomeSummaryError::MemberLimit);
            }
            state.observed_members += 1;
            state.output_items = state.output_items.saturating_add(items);
            if self.order == OutcomeOrder::Sequential
                && state.output_items > self.max_output_items
                && state.first_stop.is_none()
            {
                let factory = overflow.take().ok_or(OutcomeSummaryError::Closed)?;
                state.first_stop = Some(factory(state.output_items, self.max_output_items));
            }
            Ok(())
        })();
        drop(overflow);
        if outcome.is_ok() {
            self.staged = staged;
        }
        outcome
    }

    pub fn observe_stop(&mut self, stop: Stop) -> Result<(), OutcomeSummaryError> {
        let mut staged = self.staged.take();
        let mut stop = Some(stop);
        let outcome = (|| {
            let state = staged.as_mut().ok_or(OutcomeSummaryError::Closed)?;
            if state.observed_members >= self.max_members {
                return Err(OutcomeSummaryError::MemberLimit);
            }
            state.observed_members += 1;
            if state.first_stop.is_none() {
                state.first_stop = stop.take();
            }
            Ok(())
        })();
        drop(stop);
        if outcome.is_ok() {
            self.staged = staged;
        }
        outcome
    }

    /// Consume observations. Parallel overflow wins here, not during row admission.
    /// Native recovery calls this only when checking a committed group receipt.
    pub fn finish(
        mut self,
        overflow: impl FnOnce(usize, usize) -> Stop,
    ) -> Result<OutcomeSummary<Stop>, OutcomeSummaryError> {
        let mut overflow = Some(overflow);
        let outcome = (|| {
            let mut state = self.staged.take().ok_or(OutcomeSummaryError::Closed)?;
            if self.order == OutcomeOrder::Parallel && state.output_items > self.max_output_items {
                let factory = overflow.take().ok_or(OutcomeSummaryError::Closed)?;
                let previous = state
                    .first_stop
                    .replace(factory(state.output_items, self.max_output_items));
                drop(previous);
            }
            Ok(state)
        })();
        drop(overflow);
        outcome
    }
}

impl<Stop> fmt::Debug for OrderedOutcomeSummary<Stop> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OrderedOutcomeSummary")
            .field("order", &self.order)
            .field("max_members", &self.max_members)
            .field("max_output_items", &self.max_output_items)
            .field("closed", &self.staged.is_none())
            .finish()
    }
}
