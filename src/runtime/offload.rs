use super::{RejectedFactReason, RuntimeSession};
use crate::ledger::{GroupIssue, GroupKey, group_key, invalid_groups};
use std::collections::BTreeMap;

impl RuntimeSession {
    pub(super) fn offload_export_view(&self) -> Option<Self> {
        if !self.has_udp_offload {
            return None;
        }
        let invalid = invalid_groups(&self.facts);
        if invalid.is_empty() {
            return None;
        }
        let mut view = self.clone();
        view.reject_offload_groups(invalid);
        Some(view)
    }

    pub(super) fn finalize_offload_groups(&mut self) {
        if self.has_udp_offload {
            self.reject_offload_groups(invalid_groups(&self.facts));
        }
    }

    fn reject_offload_groups(&mut self, invalid: BTreeMap<GroupKey, GroupIssue>) {
        if invalid.is_empty() {
            return;
        }
        let facts = std::mem::take(&mut self.facts);
        self.facts = Vec::with_capacity(facts.len());
        for fact in facts {
            if let Some(issue) = group_key(&fact).and_then(|key| invalid.get(&key)) {
                let reason = match issue {
                    GroupIssue::Invalid => RejectedFactReason::InvalidUdpOffload,
                    GroupIssue::Incomplete => RejectedFactReason::IncompleteUdpOffload,
                    GroupIssue::Conflict => RejectedFactReason::ConflictingUdpOffload,
                    GroupIssue::Duplicate => RejectedFactReason::DuplicateUdpOffload,
                };
                self.reject_fact(fact, reason);
            } else {
                self.facts.push(fact);
            }
        }
    }
}
