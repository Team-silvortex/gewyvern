#[path = "core_cache_and_messaging.rs"]
mod core_cache_and_messaging;
#[path = "core_data_and_control.rs"]
mod core_data_and_control;
#[path = "core_web_and_datagram.rs"]
mod core_web_and_datagram;

pub(crate) use core_cache_and_messaging::{
    ftp_shelf, http3_shelf, memcached_shelf, mqtt_shelf, mysql_shelf, postgres_shelf, redis_shelf,
};
pub(crate) use core_data_and_control::{
    gtpu_shelf, mdns_shelf, quic_shelf, radius_shelf, ssdp_shelf,
};
pub(crate) use core_web_and_datagram::{
    coap_shelf, dhcp_shelf, dns_shelf, http_shelf, https_shelf, hy2_shelf, ntp_shelf, snmp_shelf,
    stun_shelf, tls_shelf, wireguard_shelf,
};
