mod common;
mod data;
mod discovery;
mod identity;
mod media;
mod search_storage;
mod telemetry;
mod transport;

use super::ProtocolEntrySemanticsSummary;

pub(super) fn built_in_protocol_entry_semantics(
    protocol: &str,
    entry: &str,
) -> Option<ProtocolEntrySemanticsSummary> {
    match protocol {
        "amqp" => identity::amqp_entry_semantics(entry),
        "arp" => discovery::arp_entry_semantics(entry),
        "bgp" => discovery::bgp_entry_semantics(entry),
        "cassandra" => data::cassandra_entry_semantics(entry),
        "ftp" => identity::ftp_entry_semantics(entry),
        "geneve" => transport::geneve_entry_semantics(entry),
        "gre" => discovery::gre_entry_semantics(entry),
        "gtpu" => discovery::gtpu_entry_semantics(entry),
        "dns" => discovery::dns_entry_semantics(entry),
        "elasticsearch" => search_storage::elasticsearch_entry_semantics(entry),
        "etcd" => search_storage::etcd_entry_semantics(entry),
        "zookeeper" => search_storage::zookeeper_entry_semantics(entry),
        "consul" => search_storage::consul_entry_semantics(entry),
        "coap" => discovery::coap_entry_semantics(entry),
        "dhcp" => discovery::dhcp_entry_semantics(entry),
        "https" => transport::https_entry_semantics(entry),
        "http" => transport::http_entry_semantics(entry),
        "http3" => transport::http3_entry_semantics(entry),
        "grpc" => transport::grpc_entry_semantics(entry),
        "websocket" => transport::websocket_entry_semantics(entry),
        "graphql" => transport::graphql_entry_semantics(entry),
        "otlp" => telemetry::otlp_entry_semantics(entry),
        "prometheus" => telemetry::prometheus_entry_semantics(entry),
        "loki" => telemetry::loki_entry_semantics(entry),
        "jaeger" => telemetry::jaeger_entry_semantics(entry),
        "icmp" => transport::icmp_entry_semantics(entry),
        "icmpv6" => transport::icmpv6_entry_semantics(entry),
        "imap" => identity::imap_entry_semantics(entry),
        "ipsec" => transport::ipsec_entry_semantics(entry),
        "kerberos" => identity::kerberos_entry_semantics(entry),
        "kafka" => data::kafka_entry_semantics(entry),
        "l2tp" => transport::l2tp_entry_semantics(entry),
        "ldap" => identity::ldap_entry_semantics(entry),
        "mdns" => discovery::mdns_entry_semantics(entry),
        "memcached" => data::memcached_entry_semantics(entry),
        "mongodb" => data::mongodb_entry_semantics(entry),
        "mssql" => data::mssql_entry_semantics(entry),
        "mqtt" => data::mqtt_entry_semantics(entry),
        "mysql" => data::mysql_entry_semantics(entry),
        "nats" => data::nats_entry_semantics(entry),
        "ndp" => discovery::ndp_entry_semantics(entry),
        "ntp" => discovery::ntp_entry_semantics(entry),
        "ospf" => discovery::ospf_entry_semantics(entry),
        "pop3" => identity::pop3_entry_semantics(entry),
        "postgres" => data::postgres_entry_semantics(entry),
        "pptp" => transport::pptp_entry_semantics(entry),
        "quic" => transport::quic_entry_semantics(entry),
        "radius" => identity::radius_entry_semantics(entry),
        "rdp" => transport::rdp_entry_semantics(entry),
        "redis" => data::redis_entry_semantics(entry),
        "rtsp" => media::rtsp_entry_semantics(entry),
        "s3" => search_storage::s3_entry_semantics(entry),
        "snmp" => identity::snmp_entry_semantics(entry),
        "smb" => identity::smb_entry_semantics(entry),
        "socks5" => transport::socks5_entry_semantics(entry),
        "smtp" => identity::smtp_entry_semantics(entry),
        "sip" => media::sip_entry_semantics(entry),
        "ssdp" => discovery::ssdp_entry_semantics(entry),
        "stun" => transport::stun_entry_semantics(entry),
        "tls" => transport::tls_entry_semantics(entry),
        "ssh" => identity::ssh_entry_semantics(entry),
        "hy2" => transport::hy2_entry_semantics(entry),
        "vxlan" => transport::vxlan_entry_semantics(entry),
        "wireguard" => transport::wireguard_entry_semantics(entry),
        _ => None,
    }
}
