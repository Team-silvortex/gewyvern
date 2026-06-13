use super::shelves::{
    amqp_shelf, coap_shelf, dhcp_shelf, dns_shelf, ftp_shelf, gtpu_shelf, http_shelf, http3_shelf,
    hy2_shelf, imap_shelf, kerberos_shelf, ldap_shelf, mdns_shelf, memcached_shelf, mqtt_shelf,
    mysql_shelf, ntp_shelf, pop3_shelf, postgres_shelf, quic_shelf, radius_shelf, redis_shelf,
    rtsp_shelf, sip_shelf, smtp_shelf, snmp_shelf, socks5_shelf, ssdp_shelf, ssh_shelf, stun_shelf,
    tls_shelf, wireguard_shelf,
};
use super::{ProtocolShelfSummary, ProtocolSummary, ProtocolSurfaceSummary};

pub(super) fn built_in_protocol_surface(
    summary: ProtocolSummary,
    selected_entry: String,
) -> ProtocolSurfaceSummary {
    let selected = summary
        .entries
        .iter()
        .find(|entry| entry.mode == selected_entry)
        .expect("selected entry should exist in protocol summary");
    let sibling_entries = summary
        .entries
        .iter()
        .map(|entry| entry.mode.clone())
        .collect::<Vec<_>>();
    ProtocolSurfaceSummary {
        protocol: summary.protocol.clone(),
        entry: selected.mode.clone(),
        default_entry: summary.default_entry.clone(),
        selected_is_default: selected.default,
        protocol_aliases: summary.aliases.clone(),
        entry_aliases: selected.aliases.clone(),
        sibling_entries,
        shelf: protocol_shelf(&summary.protocol, &selected.mode),
    }
}

fn protocol_shelf(protocol: &str, entry: &str) -> Option<ProtocolShelfSummary> {
    let (key, label, page, entries) = match protocol {
        "dns" => dns_shelf(entry)?,
        "http" => http_shelf(entry)?,
        "hy2" => hy2_shelf(entry)?,
        "tls" => tls_shelf(entry)?,
        "quic" => quic_shelf(entry)?,
        "stun" => stun_shelf(entry)?,
        "coap" => coap_shelf(entry)?,
        "ntp" => ntp_shelf(entry)?,
        "dhcp" => dhcp_shelf(entry)?,
        "wireguard" => wireguard_shelf(entry)?,
        "mdns" => mdns_shelf(entry)?,
        "ssdp" => ssdp_shelf(entry)?,
        "mysql" => mysql_shelf(entry)?,
        "postgres" => postgres_shelf(entry)?,
        "mqtt" => mqtt_shelf(entry)?,
        "memcached" => memcached_shelf(entry)?,
        "radius" => radius_shelf(entry)?,
        "gtpu" => gtpu_shelf(entry)?,
        "redis" => redis_shelf(entry)?,
        "amqp" => amqp_shelf(entry)?,
        "http3" => http3_shelf(entry)?,
        "smtp" => smtp_shelf(entry)?,
        "imap" => imap_shelf(entry)?,
        "pop3" => pop3_shelf(entry)?,
        "kerberos" => kerberos_shelf(entry)?,
        "ftp" => ftp_shelf(entry)?,
        "rtsp" => rtsp_shelf(entry)?,
        "ssh" => ssh_shelf(entry)?,
        "socks5" => socks5_shelf(entry)?,
        "sip" => sip_shelf(entry)?,
        "ldap" => ldap_shelf(entry)?,
        "snmp" => snmp_shelf(entry)?,
        _ => return None,
    };
    Some(ProtocolShelfSummary {
        key: key.to_string(),
        label: label.to_string(),
        page: page.to_string(),
        entries: entries.iter().map(|entry| (*entry).to_string()).collect(),
    })
}
