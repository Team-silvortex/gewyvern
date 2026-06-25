mod common;
mod data;
mod discovery;
mod identity;
mod media;
mod transport;

use super::ProtocolEntrySemanticsSummary;

pub(super) fn built_in_protocol_entry_semantics(
    protocol: &str,
    entry: &str,
) -> Option<ProtocolEntrySemanticsSummary> {
    match protocol {
        "amqp" => identity::amqp_entry_semantics(entry),
        "arp" => discovery::arp_entry_semantics(entry),
        "ftp" => identity::ftp_entry_semantics(entry),
        "http" => transport::http_entry_semantics(entry),
        "http3" => transport::http3_entry_semantics(entry),
        "icmp" => transport::icmp_entry_semantics(entry),
        "icmpv6" => transport::icmpv6_entry_semantics(entry),
        "imap" => identity::imap_entry_semantics(entry),
        "kerberos" => identity::kerberos_entry_semantics(entry),
        "ldap" => identity::ldap_entry_semantics(entry),
        "mdns" => discovery::mdns_entry_semantics(entry),
        "mysql" => data::mysql_entry_semantics(entry),
        "ndp" => discovery::ndp_entry_semantics(entry),
        "pop3" => identity::pop3_entry_semantics(entry),
        "postgres" => data::postgres_entry_semantics(entry),
        "quic" => transport::quic_entry_semantics(entry),
        "radius" => identity::radius_entry_semantics(entry),
        "redis" => data::redis_entry_semantics(entry),
        "snmp" => identity::snmp_entry_semantics(entry),
        "socks5" => transport::socks5_entry_semantics(entry),
        "smtp" => identity::smtp_entry_semantics(entry),
        "sip" => media::sip_entry_semantics(entry),
        "ssdp" => discovery::ssdp_entry_semantics(entry),
        "stun" => transport::stun_entry_semantics(entry),
        "ssh" => identity::ssh_entry_semantics(entry),
        "hy2" => transport::hy2_entry_semantics(entry),
        "wireguard" => transport::wireguard_entry_semantics(entry),
        _ => None,
    }
}
