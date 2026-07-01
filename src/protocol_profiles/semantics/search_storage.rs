use super::common::summary;
use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn s3_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "list-buckets" => (
            "s3-list-buckets-path",
            "S3-compatible bucket inventory request over an HTTP-style object-storage endpoint",
            Some("GET service root"),
        ),
        "head-object" => (
            "s3-head-object-path",
            "object metadata probe used to verify existence, permissions, and cache validators",
            Some("HEAD bucket/object"),
        ),
        "put-object" => (
            "s3-put-object-path",
            "object upload or replacement request where authorization and body transfer both matter",
            Some("PUT bucket/object"),
        ),
        "get-object" => (
            "s3-get-object-path",
            "object download request where status, range handling, and identity policy are primary",
            Some("GET bucket/object"),
        ),
        "delete-object" => (
            "s3-delete-object-path",
            "object delete request where idempotency, versioning, and permission denials are common",
            Some("DELETE bucket/object"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("object_storage_protocol_entry"),
    )
}

pub(super) fn elasticsearch_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "health" => (
            "elasticsearch-health-path",
            "Elasticsearch-compatible cluster health probe over the HTTP API",
            Some("GET _cluster/health"),
        ),
        "search" => (
            "elasticsearch-search-path",
            "search query request where query shape, routing, and response status matter",
            Some("GET/POST _search"),
        ),
        "index" => (
            "elasticsearch-index-path",
            "single-document index or update request over the HTTP document API",
            Some("PUT/POST index/_doc"),
        ),
        "bulk" => (
            "elasticsearch-bulk-path",
            "bulk indexing request where partial failures can hide behind HTTP success",
            Some("POST _bulk"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("search_datastore_protocol_entry"),
    )
}

pub(super) fn etcd_entry_semantics(entry: &str) -> Option<ProtocolEntrySemanticsSummary> {
    let (category, operator_focus, typical_signal) = match entry {
        "health" => (
            "etcd-health-path",
            "etcd cluster health probe used before trusting KV or watch results",
            Some("GET /health or Maintenance/Status"),
        ),
        "range" => (
            "etcd-range-path",
            "etcd KV range request where revision, quorum, and key prefix shape matter",
            Some("RangeRequest"),
        ),
        "put" => (
            "etcd-put-path",
            "etcd KV write request where revision advancement and compare failures matter",
            Some("PutRequest"),
        ),
        "watch" => (
            "etcd-watch-path",
            "etcd watch stream where compaction, cancel, and progress notifications matter",
            Some("WatchCreateRequest"),
        ),
        "lease" => (
            "etcd-lease-path",
            "etcd lease grant, keepalive, revoke, or TTL flow tied to key liveness",
            Some("LeaseGrant/KeepAlive"),
        ),
        _ => return None,
    };
    summary(
        category,
        operator_focus,
        typical_signal,
        None,
        None,
        Some("coordination_datastore_protocol_entry"),
    )
}
