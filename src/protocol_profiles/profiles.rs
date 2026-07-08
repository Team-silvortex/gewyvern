#[path = "access_and_media.rs"]
mod access_and_media;
#[path = "data_and_queue.rs"]
mod data_and_queue;
#[path = "database_protocols.rs"]
mod database_protocols;
#[path = "mail_and_directory.rs"]
mod mail_and_directory;
#[path = "network_control.rs"]
mod network_control;
#[path = "redis_profile.rs"]
mod redis_profile;
#[path = "secure_transport.rs"]
mod secure_transport;
#[path = "stream_messaging.rs"]
mod stream_messaging;
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
    web_protocols::GRPC_PROFILE,
    web_protocols::WEBSOCKET_PROFILE,
    web_protocols::GRAPHQL_PROFILE,
    web_protocols::S3_PROFILE,
    web_protocols::OTLP_PROFILE,
    web_protocols::PROMETHEUS_PROFILE,
    web_protocols::LOKI_PROFILE,
    web_protocols::JAEGER_PROFILE,
    web_protocols::SYSLOG_PROFILE,
    secure_transport::HY2_PROFILE,
    secure_transport::TLS_PROFILE,
    secure_transport::QUIC_PROFILE,
    network_control::STUN_PROFILE,
    network_control::COAP_PROFILE,
    network_control::TFTP_PROFILE,
    network_control::NTP_PROFILE,
    network_control::DHCP_PROFILE,
    network_control::DHCPV6_PROFILE,
    network_control::ARP_PROFILE,
    network_control::ICMP_PROFILE,
    network_control::ICMPV6_PROFILE,
    network_control::NDP_PROFILE,
    network_control::BGP_PROFILE,
    network_control::OSPF_PROFILE,
    network_control::RIP_PROFILE,
    network_control::GRE_PROFILE,
    network_control::VXLAN_PROFILE,
    network_control::GENEVE_PROFILE,
    network_control::L2TP_PROFILE,
    network_control::PPTP_PROFILE,
    secure_transport::WIREGUARD_PROFILE,
    secure_transport::IPSEC_PROFILE,
    network_control::MDNS_PROFILE,
    network_control::LLMNR_PROFILE,
    network_control::NBNS_PROFILE,
    network_control::SSDP_PROFILE,
    data_and_queue::POSTGRES_PROFILE,
    data_and_queue::MYSQL_PROFILE,
    data_and_queue::MONGODB_PROFILE,
    data_and_queue::CASSANDRA_PROFILE,
    database_protocols::MSSQL_PROFILE,
    database_protocols::ELASTICSEARCH_PROFILE,
    database_protocols::ETCD_PROFILE,
    database_protocols::ZOOKEEPER_PROFILE,
    database_protocols::CONSUL_PROFILE,
    data_and_queue::MEMCACHED_PROFILE,
    data_and_queue::AMQP_PROFILE,
    redis_profile::REDIS_PROFILE,
    data_and_queue::MQTT_PROFILE,
    stream_messaging::KAFKA_PROFILE,
    stream_messaging::NATS_PROFILE,
    data_and_queue::RADIUS_PROFILE,
    data_and_queue::GTPU_PROFILE,
    access_and_media::FTP_PROFILE,
    mail_and_directory::SMTP_PROFILE,
    mail_and_directory::IMAP_PROFILE,
    mail_and_directory::POP3_PROFILE,
    mail_and_directory::KERBEROS_PROFILE,
    access_and_media::RTSP_PROFILE,
    access_and_media::SSH_PROFILE,
    access_and_media::SMB_PROFILE,
    access_and_media::RDP_PROFILE,
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
