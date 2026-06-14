#[path = "access_and_media.rs"]
mod access_and_media;
#[path = "data_and_queue.rs"]
mod data_and_queue;
#[path = "mail_and_directory.rs"]
mod mail_and_directory;
#[path = "web_and_datagram.rs"]
mod web_and_datagram;

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
    web_and_datagram::DNS_PROFILE,
    web_and_datagram::HTTPS_PROFILE,
    web_and_datagram::HTTP_PROFILE,
    web_and_datagram::HTTP3_PROFILE,
    web_and_datagram::HY2_PROFILE,
    web_and_datagram::TLS_PROFILE,
    web_and_datagram::QUIC_PROFILE,
    web_and_datagram::STUN_PROFILE,
    web_and_datagram::COAP_PROFILE,
    web_and_datagram::NTP_PROFILE,
    web_and_datagram::DHCP_PROFILE,
    web_and_datagram::WIREGUARD_PROFILE,
    web_and_datagram::MDNS_PROFILE,
    web_and_datagram::SSDP_PROFILE,
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
