#[path = "aliases_entry.rs"]
mod aliases_entry;
#[path = "aliases_entry_continuation.rs"]
mod aliases_entry_continuation;
#[path = "aliases_entry_latest_redis.rs"]
mod aliases_entry_latest_redis;
#[path = "aliases_entry_manifest.rs"]
mod aliases_entry_manifest;
#[path = "aliases_entry_manifest_continuation.rs"]
mod aliases_entry_manifest_continuation;
#[path = "aliases_entry_manifest_latest.rs"]
mod aliases_entry_manifest_latest;
#[path = "aliases_entry_manifest_media.rs"]
mod aliases_entry_manifest_media;
#[path = "aliases_entry_redis_extended.rs"]
mod aliases_entry_redis_extended;
#[path = "aliases_entry_redis_structures.rs"]
mod aliases_entry_redis_structures;
#[path = "aliases_entry_remote_access.rs"]
mod aliases_entry_remote_access;
#[path = "aliases_entry_stream_messaging.rs"]
mod aliases_entry_stream_messaging;
#[path = "aliases_entry_tls.rs"]
mod aliases_entry_tls;
#[path = "aliases_entry_transport_and_cache.rs"]
mod aliases_entry_transport_and_cache;
#[path = "aliases_entry_tunnel.rs"]
mod aliases_entry_tunnel;
#[path = "aliases_protocol.rs"]
mod aliases_protocol;
#[path = "aliases_protocol_data_access.rs"]
mod aliases_protocol_data_access;

#[derive(Clone, Copy)]
pub(super) struct ProtocolAlias {
    pub(super) alias: &'static str,
    pub(super) protocol: &'static str,
    pub(super) entry: Option<&'static str>,
}

pub(super) fn split_protocol_alias(protocol: &str) -> (&str, Option<&str>) {
    protocol_aliases()
        .find(|alias| alias.alias == protocol)
        .map(|alias| (alias.protocol, alias.entry))
        .unwrap_or((protocol, None))
}

pub(crate) fn protocol_aliases() -> impl Iterator<Item = &'static ProtocolAlias> {
    aliases_protocol::PROTOCOL_ALIASES_CORE
        .iter()
        .chain(aliases_protocol_data_access::PROTOCOL_ALIASES_DATA_ACCESS.iter())
}

pub(super) fn resolve_protocol_entry_alias(protocol: &str, entry: &str) -> Option<&'static str> {
    protocol_entry_aliases()
        .find(|alias| alias.protocol == protocol && alias.alias == entry)
        .and_then(|alias| alias.entry)
}

pub(crate) fn protocol_entry_aliases() -> impl Iterator<Item = &'static ProtocolAlias> {
    aliases_entry::PROTOCOL_ENTRY_ALIASES
        .iter()
        .chain(aliases_entry_transport_and_cache::PROTOCOL_ENTRY_ALIASES_TRANSPORT_AND_CACHE.iter())
        .chain(aliases_entry_continuation::PROTOCOL_ENTRY_ALIASES_CONTINUATION.iter())
        .chain(aliases_entry_manifest::PROTOCOL_ENTRY_ALIASES_MANIFEST.iter())
        .chain(aliases_entry_manifest_media::PROTOCOL_ENTRY_ALIASES_MANIFEST_MEDIA.iter())
        .chain(
            aliases_entry_manifest_continuation::PROTOCOL_ENTRY_ALIASES_MANIFEST_CONTINUATION
                .iter(),
        )
        .chain(aliases_entry_manifest_latest::PROTOCOL_ENTRY_ALIASES_MANIFEST_LATEST.iter())
        .chain(aliases_entry_latest_redis::PROTOCOL_ENTRY_ALIASES_LATEST_REDIS.iter())
        .chain(aliases_entry_redis_extended::PROTOCOL_ENTRY_ALIASES_REDIS_EXTENDED.iter())
        .chain(aliases_entry_redis_structures::PROTOCOL_ENTRY_ALIASES_REDIS_STRUCTURES.iter())
        .chain(aliases_entry_tunnel::PROTOCOL_ENTRY_ALIASES_TUNNEL.iter())
        .chain(aliases_entry_tls::PROTOCOL_ENTRY_ALIASES_TLS.iter())
        .chain(aliases_entry_stream_messaging::PROTOCOL_ENTRY_ALIASES_STREAM_MESSAGING.iter())
        .chain(aliases_entry_remote_access::PROTOCOL_ENTRY_ALIASES_REMOTE_ACCESS.iter())
}
