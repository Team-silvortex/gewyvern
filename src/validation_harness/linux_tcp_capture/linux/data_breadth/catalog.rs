//! Immutable protocol cases and campaign labels; no target creation or packet interpretation.
use super::{memcached, rtsp, smtp};

#[derive(Clone, Copy)]
pub(super) struct PackageCase {
    pub(super) path: &'static str,
    pub(super) source: &'static str,
    pub(super) terminal_phase: &'static str,
    pub(super) expected_connections: [bool; 2],
}

macro_rules! case {
    ($path:literal, $phase:literal, $expected:expr) => {
        PackageCase {
            path: $path,
            source: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)),
            terminal_phase: $phase,
            expected_connections: $expected,
        }
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Protocol {
    Postgres,
    Redis,
    Mysql,
    Memcached,
    Mqtt,
    Nats,
    Smtp,
    Pop3,
    Rtsp,
}
impl Protocol {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Redis => "redis",
            Self::Mysql => "mysql",
            Self::Memcached => "memcached",
            Self::Mqtt => "mqtt",
            Self::Nats => "nats",
            Self::Smtp => "smtp",
            Self::Pop3 => "pop3",
            Self::Rtsp => "rtsp",
        }
    }
    pub(super) fn port(self) -> u16 {
        match self {
            Self::Postgres => 5432,
            Self::Redis => 6379,
            Self::Mysql => 3306,
            Self::Memcached => 11211,
            Self::Mqtt => 1883,
            Self::Nats => 4222,
            Self::Smtp => 25,
            Self::Pop3 => 110,
            Self::Rtsp => 554,
        }
    }
    pub(super) fn image(self) -> &'static str {
        match self {
            Self::Rtsp => rtsp::IMAGE,
            Self::Smtp | Self::Pop3 => smtp::IMAGE,
            Self::Mqtt => {
                "eclipse-mosquitto@sha256:75d6662dee52c947dc9dd9c29f1d8d7ffef78ddaf38001caa09755681e8bbc98"
            }
            Self::Nats => {
                "nats@sha256:558394a23dd3c36f2d3a64b51865fd09908010e80a542cea5e75c2af543ed62a"
            }
            Self::Mysql => {
                "mysql@sha256:7dcddc01f13bab2f15cde676d44d01f61fc9f99fe7785e86196dfc07d358ae2b"
            }
            Self::Memcached => memcached::IMAGE,
            Self::Postgres => {
                "postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777"
            }
            Self::Redis => {
                "redis@sha256:7958cfbb9d152f87dadfbc89f1b0b15938f0b81ff8982feb327476f4911935c3"
            }
        }
    }
    pub(super) fn cases(self) -> &'static [PackageCase] {
        match self {
            Self::Rtsp => &[
                case!(
                    "protocols/rtsp/options/main.gewy",
                    "receive_options_ok",
                    [true, true]
                ),
                case!(
                    "protocols/rtsp/describe/main.gewy",
                    "receive_describe_ok",
                    [false, true]
                ),
                case!(
                    "protocols/rtsp/setup/main.gewy",
                    "receive_setup_ok",
                    [false, true]
                ),
                case!(
                    "protocols/rtsp/play/main.gewy",
                    "receive_play_ok",
                    [false, true]
                ),
            ],
            Self::Pop3 => &[
                case!(
                    "protocols/pop3/auth/main.gewy",
                    "receive_auth_ok",
                    [false, true]
                ),
                case!(
                    "protocols/pop3/auth-denied/main.gewy",
                    "receive_auth_denied",
                    [true, false]
                ),
                case!(
                    "protocols/pop3/list/main.gewy",
                    "receive_list_ready",
                    [false, true]
                ),
            ],
            Self::Smtp => &[
                case!(
                    "protocols/smtp/session/main.gewy",
                    "receive_banner",
                    [true, true]
                ),
                case!(
                    "protocols/smtp/auth/main.gewy",
                    "receive_auth_ok",
                    [false, true]
                ),
                case!(
                    "protocols/smtp/auth-denied/main.gewy",
                    "receive_auth_denied",
                    [true, false]
                ),
                case!(
                    "protocols/smtp/mail/main.gewy",
                    "receive_mail_ok",
                    [false, true]
                ),
                case!(
                    "protocols/smtp/rcpt/main.gewy",
                    "receive_rcpt_ok",
                    [false, true]
                ),
                case!(
                    "protocols/smtp/rcpt-denied/main.gewy",
                    "receive_rcpt_denied",
                    [false, true]
                ),
                case!(
                    "protocols/smtp/data/main.gewy",
                    "receive_message_queued",
                    [false, true]
                ),
                case!(
                    "protocols/smtp/data-denied/main.gewy",
                    "receive_message_denied",
                    [false, false]
                ),
            ],
            Self::Mqtt => &[
                case!(
                    "protocols/mqtt/connect/main.gewy",
                    "receive_connack",
                    [false, true]
                ),
                case!(
                    "protocols/mqtt/connack/main.gewy",
                    "receive_connack",
                    [true, true]
                ),
                case!(
                    "protocols/mqtt/publish/main.gewy",
                    "receive_puback",
                    [false, true]
                ),
                case!(
                    "protocols/mqtt/pubrec/main.gewy",
                    "receive_pubrec",
                    [false, true]
                ),
                case!(
                    "protocols/mqtt/pubcomp/main.gewy",
                    "receive_pubcomp",
                    [false, true]
                ),
                case!(
                    "protocols/mqtt/subscribe/main.gewy",
                    "receive_suback",
                    [false, true]
                ),
            ],
            Self::Nats => &[
                case!(
                    "protocols/nats/connect/main.gewy",
                    "receive_info",
                    [true, true]
                ),
                case!(
                    "protocols/nats/error/main.gewy",
                    "receive_error",
                    [true, true]
                ),
                case!(
                    "protocols/nats/sub/main.gewy",
                    "receive_message",
                    [false, true]
                ),
            ],
            Self::Mysql => &[
                case!(
                    "protocols/mysql/auth-denied/main.gewy",
                    "receive_auth_denied",
                    [true, false]
                ),
                case!(
                    "protocols/mysql/error/main.gewy",
                    "receive_error",
                    [false, true]
                ),
                case!(
                    "protocols/mysql/auth/main.gewy",
                    "receive_ok",
                    [false, true]
                ),
            ],
            Self::Memcached => &[
                case!(
                    "protocols/memcached/get/main.gewy",
                    "receive_value",
                    [true, true]
                ),
                case!(
                    "protocols/memcached/set/main.gewy",
                    "receive_stored",
                    [true, true]
                ),
                case!(
                    "protocols/memcached/miss/main.gewy",
                    "receive_not_found",
                    [true, false]
                ),
                case!(
                    "protocols/memcached/not-stored/main.gewy",
                    "receive_not_stored",
                    [true, false]
                ),
            ],
            Self::Postgres => &[
                case!(
                    "protocols/postgres/auth-denied/main.gewy",
                    "receive_auth_denied",
                    [true, false]
                ),
                case!(
                    "protocols/postgres/error/main.gewy",
                    "receive_error",
                    [false, true]
                ),
                case!(
                    "protocols/postgres/auth/main.gewy",
                    "receive_auth",
                    [true, true]
                ),
            ],
            Self::Redis => &[
                case!(
                    "protocols/redis/auth-denied/main.gewy",
                    "receive_auth_denied",
                    [true, false]
                ),
                case!(
                    "protocols/redis/error/main.gewy",
                    "receive_error",
                    [true, false]
                ),
                case!(
                    "protocols/redis/get/main.gewy",
                    "receive_bulk",
                    [true, true]
                ),
            ],
        }
    }
}

impl Protocol {
    pub(super) fn selected_command_count(self) -> usize {
        match self {
            Self::Postgres => 10,
            Self::Redis => 21,
            Self::Mysql => 5,
            Self::Memcached => 13,
            Self::Mqtt => 9,
            Self::Nats => 16,
            Self::Smtp => 19,
            Self::Pop3 => 20,
            Self::Rtsp => 11,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Group {
    Data,
    MysqlMemcached,
    Messaging,
    Smtp,
    Pop3,
    Rtsp,
}

impl Group {
    pub(super) fn selection(self) -> (&'static str, &'static [Protocol]) {
        match self {
            Group::MysqlMemcached => ("mysql-memcached", &[Protocol::Mysql, Protocol::Memcached]),
            Group::Messaging => ("messaging-breadth", &[Protocol::Mqtt, Protocol::Nats]),
            Group::Data => ("data-breadth", &[Protocol::Postgres, Protocol::Redis]),
            Group::Smtp => ("smtp-breadth", &[Protocol::Smtp]),
            Group::Pop3 => ("pop3-breadth", &[Protocol::Pop3]),
            Group::Rtsp => ("rtsp-breadth", &[Protocol::Rtsp]),
        }
    }
    pub(super) fn checks(self) -> &'static [&'static str] {
        match self {
            Group::Rtsp => &[
                "real_MediaMTX_RTSP_1_0",
                "uncaptured_native_publisher_and_synthetic_PCMU_seed",
                "OPTIONS_and_missing_DESCRIBE_recovery",
                "SDP_and_content_length_checked",
                "unsupported_UDP_then_TCP_SETUP",
                "PLAY_keepalive_PAUSE_resume_TEARDOWN",
                "CSeq_and_session_identity_checked",
                "kernel_bytes_match_independent_client",
                "same_target_unselected_neighbor_excluded",
                "existing_protocol_packages_and_replay",
                "owned_internal_network_cleanup",
                "fresh_HTTP_capture_after_data",
            ],
            Group::Pop3 => &[
                "real_Mailpit_POP3",
                "uncaptured_SMTP_seed_then_captured_mailbox_retrieval",
                "auth_denial_and_unauthorized_STAT",
                "LIST_STAT_and_RETR_declared_sizes_agree",
                "Mailpit_extra_CRLF_disclosed_and_raw_bytes_preserved",
                "UIDL_unique_and_RETR_bodies_match_seed",
                "matched_packet_status_cross_check",
                "missing_and_deleted_message_errors",
                "dot_unstuffed_body_not_status_lines",
                "delete_mark_reset_and_restored_list",
                "kernel_bytes_match_independent_client",
                "same_target_unselected_neighbor_excluded",
                "existing_protocol_packages_and_replay",
                "owned_internal_network_cleanup",
                "fresh_HTTP_capture_after_data",
            ],
            Group::Smtp => &[
                "real_Mailpit_SMTP",
                "auth_denial_and_unauthenticated_submission_refusal",
                "multiline_EHLO_and_bounded_CRLF_replies",
                "bad_sequence_and_recipient_rejection_then_same_connection_submission",
                "dot_stuffed_body_and_second_transaction",
                "matched_packet_status_cross_check",
                "kernel_bytes_match_independent_client",
                "same_target_unselected_neighbor_excluded",
                "existing_protocol_packages_and_replay",
                "owned_internal_network_cleanup",
                "fresh_HTTP_capture_after_data",
            ],
            Group::Messaging => &[
                "real_Mosquitto_and_NATS",
                "MQTT_identifier_refusal_persistent_session_resume_and_QoS_acks",
                "NATS_auth_denial_permission_error_then_same_connection_recovery",
                "binary_empty_and_control_looking_message_bodies",
                "matched_packet_status_cross_check",
                "kernel_bytes_match_independent_client",
                "same_target_unselected_neighbor_excluded",
                "existing_protocol_packages_and_replay",
                "owned_internal_network_and_anonymous_volume_cleanup",
                "fresh_HTTP_capture_after_data",
            ],
            Group::MysqlMemcached => &[
                "real_MySQL_and_Memcached",
                "MySQL_native_auth_query_error_and_recovery",
                "Memcached_binary_miss_not_stored_oversized_and_recovery",
                "matched_packet_status_cross_check",
                "kernel_bytes_match_independent_client",
                "same_target_unselected_neighbor_excluded",
                "existing_protocol_packages_and_replay",
                "owned_internal_network_and_anonymous_volume_cleanup",
                "fresh_HTTP_capture_after_data",
            ],
            Group::Data => &[
                "real_PostgreSQL_and_Redis",
                "auth_denial_query_error_then_recovery",
                "PostgreSQL_failed_transaction_rollback_and_null_empty",
                "Redis_empty_nil_binary_and_pipelined_replies",
                "bounded_partial_client_reads_not_packet_segmentation",
                "same_target_unselected_denial_excluded",
                "kernel_bytes_match_independent_client",
                "existing_protocol_packages_and_replay",
                "unobserved_process_route_and_egress_not_invented",
                "owned_internal_network_and_anonymous_volume_cleanup",
                "fresh_HTTP_capture_after_data",
            ],
        }
    }
}
