mod core;
mod extended;

pub(super) type ShelfMatch = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
);

pub(super) use core::{
    coap_shelf, dhcp_shelf, dns_shelf, ftp_shelf, gtpu_shelf, http_shelf, http3_shelf, https_shelf,
    hy2_shelf, mdns_shelf, memcached_shelf, mqtt_shelf, mysql_shelf, ntp_shelf, postgres_shelf,
    quic_shelf, radius_shelf, redis_shelf, snmp_shelf, ssdp_shelf, stun_shelf, tls_shelf,
    wireguard_shelf,
};
pub(super) use extended::{
    amqp_shelf, imap_shelf, kerberos_shelf, ldap_shelf, pop3_shelf, rtsp_shelf, sip_shelf,
    smtp_shelf, socks5_shelf, ssh_shelf,
};
