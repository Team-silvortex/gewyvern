#[path = "aliases_entry.rs"]
mod aliases_entry;
#[path = "aliases_entry_continuation.rs"]
mod aliases_entry_continuation;
#[path = "aliases_entry_extended.rs"]
mod aliases_entry_extended;
#[path = "aliases_entry_latest_redis.rs"]
mod aliases_entry_latest_redis;
#[path = "aliases_entry_manifest.rs"]
mod aliases_entry_manifest;
#[path = "aliases_entry_manifest_continuation.rs"]
mod aliases_entry_manifest_continuation;
#[path = "aliases_entry_manifest_latest.rs"]
mod aliases_entry_manifest_latest;
#[path = "aliases_entry_tunnel.rs"]
mod aliases_entry_tunnel;
#[path = "aliases_protocol.rs"]
mod aliases_protocol;

pub(crate) use aliases_protocol::PROTOCOL_ALIASES;

#[derive(Clone, Copy)]
pub(super) struct ProtocolAlias {
    pub(super) alias: &'static str,
    pub(super) protocol: &'static str,
    pub(super) entry: Option<&'static str>,
}

pub(super) fn split_protocol_alias(protocol: &str) -> (&str, Option<&str>) {
    PROTOCOL_ALIASES
        .iter()
        .find(|alias| alias.alias == protocol)
        .map(|alias| (alias.protocol, alias.entry))
        .unwrap_or((protocol, None))
}

pub(super) fn resolve_protocol_entry_alias(protocol: &str, entry: &str) -> Option<&'static str> {
    protocol_entry_aliases()
        .find(|alias| alias.protocol == protocol && alias.alias == entry)
        .and_then(|alias| alias.entry)
}

pub(crate) fn protocol_entry_aliases() -> impl Iterator<Item = &'static ProtocolAlias> {
    aliases_entry::PROTOCOL_ENTRY_ALIASES
        .iter()
        .chain(aliases_entry_continuation::PROTOCOL_ENTRY_ALIASES_CONTINUATION.iter())
        .chain(aliases_entry_manifest::PROTOCOL_ENTRY_ALIASES_MANIFEST.iter())
        .chain(
            aliases_entry_manifest_continuation::PROTOCOL_ENTRY_ALIASES_MANIFEST_CONTINUATION
                .iter(),
        )
        .chain(aliases_entry_manifest_latest::PROTOCOL_ENTRY_ALIASES_MANIFEST_LATEST.iter())
        .chain(aliases_entry_extended::PROTOCOL_ENTRY_ALIASES_EXTENDED.iter())
        .chain(aliases_entry_latest_redis::PROTOCOL_ENTRY_ALIASES_LATEST_REDIS.iter())
        .chain(aliases_entry_tunnel::PROTOCOL_ENTRY_ALIASES_TUNNEL.iter())
}
