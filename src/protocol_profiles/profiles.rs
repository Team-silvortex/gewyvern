#[path = "access_and_media.rs"]
mod access_and_media;
#[path = "data_and_queue.rs"]
mod data_and_queue;
#[path = "mail_and_directory.rs"]
mod mail_and_directory;
#[path = "network_control.rs"]
mod network_control;
#[path = "secure_transport.rs"]
mod secure_transport;
#[path = "web_protocols.rs"]
mod web_protocols;

#[derive(Clone, Copy)]
pub(super) struct ProtocolEntryProfile {
    pub(super) mode: &'static str,
    pub(super) dsl_path: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct ProtocolProfile {
    pub(super) name: &'static str,
    pub(super) default_entry: &'static str,
    pub(super) entries: &'static [ProtocolEntryProfile],
}

pub(super) const PROTOCOL_REGISTRY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/protocols");
pub(super) const PACKAGED_SHARE_ROOT: &str = "/usr/share/gewyvern";
pub(super) const PROTOCOL_PROFILES: &[ProtocolProfile] = &[
    web_protocols::DNS_PROFILE,
    web_protocols::HTTPS_PROFILE,
    web_protocols::HTTP_PROFILE,
    web_protocols::HTTP3_PROFILE,
    secure_transport::HY2_PROFILE,
    secure_transport::TLS_PROFILE,
    secure_transport::QUIC_PROFILE,
    network_control::STUN_PROFILE,
    network_control::COAP_PROFILE,
    network_control::NTP_PROFILE,
    network_control::DHCP_PROFILE,
    network_control::ARP_PROFILE,
    network_control::ICMP_PROFILE,
    network_control::ICMPV6_PROFILE,
    network_control::NDP_PROFILE,
    network_control::BGP_PROFILE,
    network_control::OSPF_PROFILE,
    network_control::GRE_PROFILE,
    network_control::VXLAN_PROFILE,
    network_control::GENEVE_PROFILE,
    network_control::L2TP_PROFILE,
    network_control::PPTP_PROFILE,
    secure_transport::WIREGUARD_PROFILE,
    secure_transport::IPSEC_PROFILE,
    network_control::MDNS_PROFILE,
    network_control::SSDP_PROFILE,
    data_and_queue::POSTGRES_PROFILE,
    data_and_queue::MYSQL_PROFILE,
    data_and_queue::MEMCACHED_PROFILE,
    data_and_queue::AMQP_PROFILE,
    data_and_queue::REDIS_PROFILE,
    data_and_queue::MQTT_PROFILE,
    data_and_queue::RADIUS_PROFILE,
    data_and_queue::GTPU_PROFILE,
    access_and_media::FTP_PROFILE,
    mail_and_directory::SMTP_PROFILE,
    mail_and_directory::IMAP_PROFILE,
    mail_and_directory::POP3_PROFILE,
    mail_and_directory::KERBEROS_PROFILE,
    access_and_media::RTSP_PROFILE,
    access_and_media::SSH_PROFILE,
    access_and_media::SOCKS5_PROFILE,
    access_and_media::SIP_PROFILE,
    mail_and_directory::LDAP_PROFILE,
    mail_and_directory::SNMP_PROFILE,
];

pub(super) fn find_protocol_profile(protocol: &str) -> Option<&'static ProtocolProfile> {
    PROTOCOL_PROFILES
        .iter()
        .find(|profile| profile.name == protocol)
}
