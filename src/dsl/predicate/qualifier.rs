use crate::ir::{PayloadByteMatch, PayloadByteSequenceMatch};
use crate::ledger::{PacketDir, QuicFrameType, QuicPacketType};

use super::DslError;

pub(super) struct QualifierPartsCursor<'a> {
    parts: &'a [(usize, String)],
    index: &'a mut usize,
}

impl<'a> QualifierPartsCursor<'a> {
    pub(super) fn new(parts: &'a [(usize, String)], index: &'a mut usize) -> Self {
        Self { parts, index }
    }
}

impl<'a> Iterator for QualifierPartsCursor<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, value) = self.parts.get(*self.index)?;
        *self.index += 1;
        Some(value.as_str())
    }
}

pub(super) fn split_qualifier_parts_with_columns(
    input: &str,
    base_column: usize,
) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in input.char_indices() {
        if ch == ':' {
            let raw = &input[start..idx];
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let leading = raw.find(trimmed).unwrap_or(0);
                parts.push((base_column + start + leading, trimmed.to_string()));
            }
            start = idx + 1;
        }
    }
    let raw_tail = &input[start..];
    let tail = raw_tail.trim();
    if !tail.is_empty() {
        let leading = raw_tail.find(tail).unwrap_or(0);
        parts.push((base_column + start + leading, tail.to_string()));
    }
    parts
}

pub(super) fn qualifier_part_at(parts: &[(usize, String)], index: usize) -> (usize, &str) {
    let (column, value) = &parts[index];
    (*column, value.as_str())
}

pub(super) fn qualifier_part_opt(parts: &[(usize, String)], index: usize) -> Option<(usize, &str)> {
    parts
        .get(index)
        .map(|(column, value)| (*column, value.as_str()))
}

pub(crate) fn parse_named_port(value: &str, predicate: &str) -> Result<u16, DslError> {
    match value {
        "quic" | "https" | "hy2" | "hysteria2" => Ok(443),
        "http" => Ok(80),
        "dhcp_client" | "bootpc" => Ok(68),
        "dhcp_server" | "bootps" | "dhcp" => Ok(67),
        "dhcpv6_client" | "dhcp6_client" => Ok(546),
        "dhcpv6_server" | "dhcp6_server" | "dhcpv6" | "dhcp6" => Ok(547),
        "llmnr" => Ok(5355),
        "nbns" | "netbios-ns" => Ok(137),
        "mdns" => Ok(5353),
        "ssdp" => Ok(1900),
        "wireguard" => Ok(51820),
        "coap" => Ok(5683),
        "tftp" => Ok(69),
        "syslog" => Ok(514),
        "syslog_tls" | "syslog-tls" => Ok(6514),
        "ntp" => Ok(123),
        "stun" => Ok(3478),
        "postgres" => Ok(5432),
        "mysql" => Ok(3306),
        "memcached" => Ok(11211),
        "amqp" => Ok(5672),
        "kafka" => Ok(9092),
        "nats" => Ok(4222),
        "ldap" => Ok(389),
        "redis" => Ok(6379),
        "mqtt" => Ok(1883),
        "radius" => Ok(1812),
        "gtpu" => Ok(2152),
        "vxlan" => Ok(4789),
        "geneve" => Ok(6081),
        "l2tp" => Ok(1701),
        "pptp" => Ok(1723),
        "bgp" => Ok(179),
        "rip" => Ok(520),
        "sip" => Ok(5060),
        "rtsp" => Ok(554),
        "socks" | "socks5" => Ok(1080),
        "ftp" => Ok(21),
        "smb" | "smb2" | "cifs" => Ok(445),
        "rdp" => Ok(3389),
        "smtp" => Ok(25),
        "imap" => Ok(143),
        "pop3" => Ok(110),
        "ssh" => Ok(22),
        "snmp" => Ok(161),
        "snmp_trap" | "snmptrap" => Ok(162),
        "kerberos" => Ok(88),
        other => other
            .parse::<u16>()
            .map_err(|_| DslError::InvalidValue(format!("unknown {predicate} port '{other}'"))),
    }
}

fn parse_u8_literal(value: &str, predicate: &str, field: &str) -> Result<u8, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u8::from_str_radix(hex, 16)
    } else {
        value.parse::<u8>()
    };
    parsed.map_err(|_| DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'")))
}

fn parse_u8_sequence_literal(
    value: &str,
    predicate: &str,
    field: &str,
) -> Result<Vec<u8>, DslError> {
    let bytes = value
        .split(',')
        .map(|byte| parse_u8_literal(byte.trim(), predicate, field))
        .collect::<Result<Vec<_>, _>>()?;
    if bytes.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "invalid {predicate} {field} '{value}'"
        )));
    }
    Ok(bytes)
}

pub(super) fn parse_payload_byte_match<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
) -> Result<PayloadByteMatch, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let offset = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} byte_at offset qualifier"))
    })?;
    let mask = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} byte_at mask qualifier"))
    })?;
    let value = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} byte_at value qualifier"))
    })?;
    Ok(PayloadByteMatch {
        offset: parse_u16_literal(offset, predicate, "byte_at_offset")?,
        mask: parse_u8_literal(mask, predicate, "byte_at_mask")?,
        value: parse_u8_literal(value, predicate, "byte_at_value")?,
    })
}

pub(super) fn parse_payload_byte_sequence_match<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
) -> Result<PayloadByteSequenceMatch, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let offset = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} bytes_at offset qualifier"))
    })?;
    let bytes = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!(
            "missing {subject} bytes_at byte sequence qualifier"
        ))
    })?;
    Ok(PayloadByteSequenceMatch {
        offset: parse_u16_literal(offset, predicate, "bytes_at_offset")?,
        bytes: parse_u8_sequence_literal(bytes, predicate, "bytes_at")?,
    })
}

pub(super) fn parse_scope_qualifier<'a, I>(
    part: &str,
    parts: &mut I,
    predicate: &str,
    subject: &str,
    dir: &mut Option<PacketDir>,
    local_port: &mut Option<u16>,
    remote_port: &mut Option<u16>,
) -> Result<bool, DslError>
where
    I: Iterator<Item = &'a str>,
{
    match part {
        "egress" | "local_to_remote" => {
            *dir = Some(PacketDir::Egress);
            Ok(true)
        }
        "ingress" | "remote_to_local" => {
            *dir = Some(PacketDir::Ingress);
            Ok(true)
        }
        "local" | "sport" => {
            let port = parts.next().ok_or_else(|| {
                DslError::InvalidValue(format!("missing {subject} local port qualifier"))
            })?;
            *local_port = Some(parse_named_port(port, predicate)?);
            Ok(true)
        }
        "remote" | "dport" => {
            let port = parts.next().ok_or_else(|| {
                DslError::InvalidValue(format!("missing {subject} remote port qualifier"))
            })?;
            *remote_port = Some(parse_named_port(port, predicate)?);
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(super) fn parse_u8_mask_value_qualifier<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
    field: &str,
) -> Result<(u8, u8), DslError>
where
    I: Iterator<Item = &'a str>,
{
    let mask = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} mask qualifier")))?;
    let value = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} value qualifier")))?;
    Ok((
        parse_u8_literal(mask, predicate, field)?,
        parse_u8_literal(value, predicate, &format!("{field}_value"))?,
    ))
}

pub(super) fn parse_u16_qualifier<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
    field: &str,
) -> Result<u16, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let value = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} qualifier")))?;
    parse_u16_literal(value, predicate, field)
}

pub(super) fn parse_u32_qualifier<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
    field: &str,
) -> Result<u32, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let value = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} qualifier")))?;
    parse_u32_literal(value, predicate, field)
}

fn parse_u16_literal(value: &str, predicate: &str, field: &str) -> Result<u16, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else {
        value.parse::<u16>()
    };
    parsed.map_err(|_| DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'")))
}

fn parse_u32_literal(value: &str, predicate: &str, field: &str) -> Result<u32, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        value.parse::<u32>()
    };
    parsed.map_err(|_| DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'")))
}

pub(crate) fn parse_quic_packet_type(value: &str) -> Result<QuicPacketType, DslError> {
    match value {
        "initial" => Ok(QuicPacketType::Initial),
        "0rtt" | "zero_rtt" => Ok(QuicPacketType::ZeroRtt),
        "handshake" => Ok(QuicPacketType::Handshake),
        "retry" => Ok(QuicPacketType::Retry),
        other => Err(DslError::InvalidValue(format!(
            "unknown QUIC packet type '{other}'"
        ))),
    }
}

pub(crate) fn parse_quic_frame_type(value: &str) -> Result<QuicFrameType, DslError> {
    match value {
        "crypto" => Ok(QuicFrameType::Crypto),
        "ack" => Ok(QuicFrameType::Ack),
        "stream" => Ok(QuicFrameType::Stream),
        "datagram" => Ok(QuicFrameType::Datagram),
        "connection_close" | "close" => Ok(QuicFrameType::ConnectionClose),
        other => Err(DslError::InvalidValue(format!(
            "unknown QUIC frame type '{other}'"
        ))),
    }
}
