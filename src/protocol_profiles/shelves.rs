mod core;
mod extended;

use super::ProtocolShelfSummary;

pub(super) type ShelfMatch = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
);

pub(super) use core::{
    arp_shelf, bgp_shelf, coap_shelf, dhcp_shelf, dns_shelf, ftp_shelf, gre_shelf, gtpu_shelf,
    http_shelf, http3_shelf, https_shelf, hy2_shelf, icmp_shelf, icmpv6_shelf, ipsec_shelf,
    mdns_shelf, memcached_shelf, mqtt_shelf, mysql_shelf, ndp_shelf, ntp_shelf, ospf_shelf,
    postgres_shelf, quic_shelf, radius_shelf, redis_shelf, snmp_shelf, ssdp_shelf, stun_shelf,
    tls_shelf, wireguard_shelf,
};
pub(super) use extended::{
    amqp_shelf, imap_shelf, kerberos_shelf, ldap_shelf, pop3_shelf, rtsp_shelf, sip_shelf,
    smtp_shelf, socks5_shelf, ssh_shelf,
};

pub(super) fn built_in_protocol_shelf(protocol: &str, entry: &str) -> Option<ProtocolShelfSummary> {
    let (key, label, page, entries) = match protocol {
        "dns" => dns_shelf(entry)?,
        "https" => https_shelf(entry)?,
        "http" => http_shelf(entry)?,
        "hy2" => hy2_shelf(entry)?,
        "tls" => tls_shelf(entry)?,
        "quic" => quic_shelf(entry)?,
        "stun" => stun_shelf(entry)?,
        "coap" => coap_shelf(entry)?,
        "ntp" => ntp_shelf(entry)?,
        "dhcp" => dhcp_shelf(entry)?,
        "arp" => arp_shelf(entry)?,
        "icmp" => icmp_shelf(entry)?,
        "icmpv6" => icmpv6_shelf(entry)?,
        "ndp" => ndp_shelf(entry)?,
        "bgp" => bgp_shelf(entry)?,
        "ospf" => ospf_shelf(entry)?,
        "gre" => gre_shelf(entry)?,
        "wireguard" => wireguard_shelf(entry)?,
        "ipsec" => ipsec_shelf(entry)?,
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
