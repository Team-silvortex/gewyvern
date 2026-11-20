use crate::ledger::{FactEnvelope, FactKind};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type GroupKey = (u64, u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum GroupIssue {
    Invalid,
    Conflict,
    Duplicate,
    Incomplete,
}

pub(crate) fn group_key(fact: &FactEnvelope) -> Option<GroupKey> {
    let FactKind::PacketMeta(packet) = &fact.kind else {
        return None;
    };
    packet
        .udp_offload
        .map(|segment| (fact.session.0, segment.aggregate_id))
}

pub(crate) fn same_group(first: &FactEnvelope, next: &FactEnvelope) -> bool {
    let (FactKind::PacketMeta(a), FactKind::PacketMeta(b)) = (&first.kind, &next.kind) else {
        return false;
    };
    let (Some(mut a_origin), Some(b_origin)) = (a.udp_offload, b.udp_offload) else {
        return false;
    };
    a_origin.segment_index = b_origin.segment_index;
    a_origin == b_origin
        && first.session == next.session
        && first.ts == next.ts
        && first.cpu == next.cpu
        && first.ifindex == next.ifindex
        && first.fragment_id == next.fragment_id
        && a.netns == b.netns
        && a.sk_cookie == b.sk_cookie
        && a.dir == b.dir
        && a.local_port == b.local_port
        && a.remote_port == b.remote_port
        && a.l3_proto == b.l3_proto
}

/// Complete-batch audit, independent of arrival order. One session/aggregate ID
/// denotes one hook observation; its fact IDs must be unambiguous in the bundle.
pub(crate) fn invalid_groups(facts: &[FactEnvelope]) -> BTreeMap<GroupKey, GroupIssue> {
    if !facts.iter().any(|fact| group_key(fact).is_some()) {
        return BTreeMap::new();
    }
    struct Group<'a> {
        first: &'a FactEnvelope,
        seen: u16,
        expected: u16,
        issue: Option<GroupIssue>,
    }
    let mut groups: BTreeMap<GroupKey, Group<'_>> = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut duplicate_groups = BTreeSet::new();
    for fact in facts {
        let key = group_key(fact);
        if let Some(previous) = owners.insert(fact.id, key) {
            duplicate_groups.extend(previous);
            duplicate_groups.extend(key);
        }
        let Some(key) = key else { continue };
        let FactKind::PacketMeta(packet) = &fact.kind else {
            continue;
        };
        let Some(segment) = packet.udp_offload else {
            continue;
        };
        let group = groups.entry(key).or_insert_with(|| Group {
            first: fact,
            seen: 0,
            expected: segment.segment_count,
            issue: None,
        });
        let issue = if !segment.matches_packet(packet) {
            Some(GroupIssue::Invalid)
        } else if !same_group(group.first, fact) {
            Some(GroupIssue::Conflict)
        } else if group.seen & (1 << segment.segment_index) != 0 {
            Some(GroupIssue::Duplicate)
        } else {
            group.seen |= 1 << segment.segment_index;
            None
        };
        if let Some(issue) = issue {
            group.issue = Some(group.issue.map_or(issue, |old| old.min(issue)));
        }
    }
    groups
        .into_iter()
        .filter_map(|(key, group)| {
            let mut issue = group.issue;
            if duplicate_groups.contains(&key) {
                issue =
                    Some(issue.map_or(GroupIssue::Duplicate, |old| old.min(GroupIssue::Duplicate)));
            }
            let issue = issue.or_else(|| {
                (group.seen.count_ones() != u32::from(group.expected))
                    .then_some(GroupIssue::Incomplete)
            });
            issue.map(|issue| (key, issue))
        })
        .collect()
}
