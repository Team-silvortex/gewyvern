#[path = "core_cache_and_messaging.rs"]
mod core_cache_and_messaging;
#[path = "core_data_and_control.rs"]
mod core_data_and_control;
#[path = "core_stream_messaging.rs"]
mod core_stream_messaging;
#[path = "core_web_and_datagram.rs"]
mod core_web_and_datagram;
#[path = "core_web_rpc.rs"]
mod core_web_rpc;

pub(crate) use core_cache_and_messaging::{
    cassandra_shelf, elasticsearch_shelf, etcd_shelf, ftp_shelf, http3_shelf, memcached_shelf,
    mongodb_shelf, mqtt_shelf, mssql_shelf, mysql_shelf, postgres_shelf, redis_shelf,
};
pub(crate) use core_data_and_control::{
    gtpu_shelf, mdns_shelf, quic_shelf, radius_shelf, ssdp_shelf,
};
pub(crate) use core_stream_messaging::{kafka_shelf, nats_shelf};
pub(crate) use core_web_and_datagram::{
    arp_shelf, bgp_shelf, coap_shelf, dhcp_shelf, dns_shelf, geneve_shelf, gre_shelf, http_shelf,
    https_shelf, hy2_shelf, icmp_shelf, icmpv6_shelf, ipsec_shelf, l2tp_shelf, ndp_shelf,
    ntp_shelf, ospf_shelf, pptp_shelf, snmp_shelf, stun_shelf, tls_shelf, vxlan_shelf,
    wireguard_shelf,
};
pub(crate) use core_web_rpc::{graphql_shelf, grpc_shelf, s3_shelf, websocket_shelf};
