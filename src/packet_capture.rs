//! Capture-to-fact boundary for external producers. This does not attach probes
//! or authenticate observations; consumers must retain advisory ingest trust.
use std::net::SocketAddr;
use std::time::SystemTime;

use crate::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId, UdpOffloadSegment,
};

pub const MAX_UDP_CAPTURE_PREFIX: usize = 128;

#[derive(Clone, Copy, Debug)]
pub struct PacketObservation {
    pub id: FactId,
    pub session: SessionId,
    pub ts: SystemTime,
    pub cpu: u32,
    pub ifindex: u32,
    pub netns: u32,
    pub socket_cookie: u64,
    pub direction: PacketDir,
}

/// Host-order lengths; `total_len` includes the IP header for both families.
/// Only the actually observed prefix is supplied, never a zero-padded buffer.
/// The producer must first establish direct UDP framing (no IPv6 extensions,
/// fragmentation or jumbograms); this adapter does not parse raw IP headers,
/// verify checksums, retain endpoint addresses or establish process identity.
#[derive(Clone, Copy, Debug)]
pub struct CapturedUdpDatagram<'a> {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub ip_header_len: u8,
    pub total_len: u32,
    pub payload_len: u16,
    pub payload_prefix: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureDecodeError {
    AddressFamily,
    IpHeader,
    DatagramLength,
    PayloadPrefix,
    SocketIdentity,
    CpuOverflow,
    OffloadMetadata,
}

impl CaptureDecodeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AddressFamily => "capture_address_family_mismatch",
            Self::IpHeader => "capture_ip_header_unsupported",
            Self::DatagramLength => "capture_datagram_length_invalid",
            Self::PayloadPrefix => "capture_payload_prefix_invalid",
            Self::SocketIdentity => "capture_socket_identity_missing",
            Self::CpuOverflow => "capture_cpu_overflow",
            Self::OffloadMetadata => "capture_udp_offload_invalid",
        }
    }
}

impl std::fmt::Display for CaptureDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for CaptureDecodeError {}

impl CapturedUdpDatagram<'_> {
    /// Accepts one already-normalized segment, preserving its aggregate origin.
    /// The supplied prefix must contain all bytes up to the capture prefix cap.
    /// This does not split a buffer or prove that the other segments were seen.
    pub fn into_offload_segment_fact(
        self,
        observation: PacketObservation,
        provenance: UdpOffloadSegment,
    ) -> Result<FactEnvelope, CaptureDecodeError> {
        if provenance.ip_header_len != self.ip_header_len {
            return Err(CaptureDecodeError::OffloadMetadata);
        }
        let mut fact = self.into_fact(observation)?;
        let FactKind::PacketMeta(packet) = &mut fact.kind else {
            return Err(CaptureDecodeError::OffloadMetadata);
        };
        if !provenance.matches_packet(packet) {
            return Err(CaptureDecodeError::OffloadMetadata);
        }
        packet.udp_offload = Some(provenance);
        Ok(fact)
    }

    pub fn into_fact(
        self,
        observation: PacketObservation,
    ) -> Result<FactEnvelope, CaptureDecodeError> {
        let ipv6 = match (self.source, self.destination) {
            (SocketAddr::V4(_), SocketAddr::V4(_)) => false,
            (SocketAddr::V6(_), SocketAddr::V6(_)) => true,
            _ => return Err(CaptureDecodeError::AddressFamily),
        };
        if if ipv6 {
            self.ip_header_len != 40
        } else {
            !(20..=60).contains(&self.ip_header_len) || !self.ip_header_len.is_multiple_of(4)
        } {
            return Err(CaptureDecodeError::IpHeader);
        }
        if self.total_len != u32::from(self.ip_header_len) + 8 + u32::from(self.payload_len)
            || self.total_len > u32::from(u16::MAX) + if ipv6 { 40 } else { 0 }
        {
            return Err(CaptureDecodeError::DatagramLength);
        }
        let bytes = self.payload_prefix;
        if bytes.len() > MAX_UDP_CAPTURE_PREFIX || bytes.len() > usize::from(self.payload_len) {
            return Err(CaptureDecodeError::PayloadPrefix);
        }
        if observation.socket_cookie == 0 || self.source.port() == 0 || self.destination.port() == 0
        {
            return Err(CaptureDecodeError::SocketIdentity);
        }
        let cpu = u16::try_from(observation.cpu).map_err(|_| CaptureDecodeError::CpuOverflow)?;
        let outbound = observation.direction == PacketDir::Egress;
        let at = |offset| bytes.get(offset).copied();
        Ok(FactEnvelope {
            id: observation.id,
            session: observation.session,
            ts: observation.ts,
            cpu: CpuId(cpu),
            ifindex: Some(observation.ifindex),
            fragment_id: "udp_packet_meta_fragment".into(),
            kind: FactKind::PacketMeta(PacketMetaFact {
                udp_offload: None,
                netns: observation.netns,
                sk_cookie: Some(observation.socket_cookie),
                dir: observation.direction,
                local_port: Some(if outbound {
                    self.source.port()
                } else {
                    self.destination.port()
                }),
                remote_port: Some(if outbound {
                    self.destination.port()
                } else {
                    self.source.port()
                }),
                l3_proto: if ipv6 { 0x86dd } else { 0x0800 },
                l4_proto: 17,
                tot_len: self.total_len,
                payload_byte0: at(0),
                payload_byte1: at(1),
                payload_prefix2: bytes.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]])),
                payload_prefix4: bytes
                    .get(..4)
                    .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
                payload_byte4: at(4),
                payload_byte5: at(5),
                payload_byte9: at(9),
                payload_byte10: at(10),
                payload_byte13: at(13),
                payload_bytes: bytes
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (i as u16, *b))
                    .collect(),
                tcp_flags: 0,
                seq: None,
                ack: None,
                window: None,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> PacketObservation {
        PacketObservation {
            id: FactId(1),
            session: SessionId(1),
            ts: SystemTime::UNIX_EPOCH,
            cpu: 3,
            ifindex: 2,
            netns: 1,
            socket_cookie: 42,
            direction: PacketDir::Egress,
        }
    }

    fn packet(ipv6: bool) -> CapturedUdpDatagram<'static> {
        CapturedUdpDatagram {
            source: if ipv6 {
                "[fd00::1]:4000"
            } else {
                "127.0.0.1:4000"
            }
            .parse()
            .unwrap(),
            destination: if ipv6 { "[fd00::2]:53" } else { "127.0.0.2:53" }
                .parse()
                .unwrap(),
            ip_header_len: if ipv6 { 40 } else { 20 },
            total_len: if ipv6 { 60 } else { 40 },
            payload_len: 12,
            payload_prefix: &[0x44, 0x11, 0x80, 3],
        }
    }

    #[test]
    fn families_preserve_lengths_directions_and_only_observed_bytes() {
        for ipv6 in [false, true] {
            for direction in [PacketDir::Ingress, PacketDir::Egress] {
                let mut observation = observation();
                observation.direction = direction;
                let fact = packet(ipv6).into_fact(observation).unwrap();
                let FactKind::PacketMeta(meta) = &fact.kind else {
                    panic!("packet fact expected")
                };
                assert_eq!(meta.l3_proto, if ipv6 { 0x86dd } else { 0x0800 });
                assert_eq!(meta.tot_len, if ipv6 { 60 } else { 40 });
                assert_eq!(
                    meta.local_port,
                    Some(if direction == PacketDir::Egress {
                        4000
                    } else {
                        53
                    })
                );
                assert_eq!(
                    meta.remote_port,
                    Some(if direction == PacketDir::Egress {
                        53
                    } else {
                        4000
                    })
                );
                assert_eq!(meta.payload_bytes.len(), 4);
                assert_eq!(meta.payload_prefix4, Some(0x44118003));
                assert_eq!(meta.payload_byte4, None);
                assert_eq!(
                    crate::export::fact_from_json(&crate::export::fact_to_json(&fact)).unwrap(),
                    fact
                );
            }
        }
    }

    #[test]
    fn malformed_family_headers_lengths_and_prefix_are_rejected() {
        let mut bad = packet(true);
        bad.destination = packet(false).destination;
        assert_eq!(
            bad.into_fact(observation()),
            Err(CaptureDecodeError::AddressFamily)
        );
        for (ipv6, header) in [
            (true, 20),
            (true, 48),
            (false, 19),
            (false, 21),
            (false, 64),
        ] {
            let mut bad = packet(ipv6);
            bad.ip_header_len = header;
            assert_eq!(
                bad.into_fact(observation()),
                Err(CaptureDecodeError::IpHeader)
            );
        }
        for ipv6 in [false, true] {
            let mut bad = packet(ipv6);
            bad.total_len -= 1;
            assert_eq!(
                bad.into_fact(observation()),
                Err(CaptureDecodeError::DatagramLength)
            );
            bad.payload_len = u16::MAX;
            bad.total_len = u32::from(bad.ip_header_len) + 8 + u32::from(bad.payload_len);
            assert_eq!(
                bad.into_fact(observation()),
                Err(CaptureDecodeError::DatagramLength)
            );
        }
        let mut bad = packet(true);
        bad.payload_prefix = &[0; 13];
        assert_eq!(
            bad.into_fact(observation()),
            Err(CaptureDecodeError::PayloadPrefix)
        );
        bad.payload_prefix = &[0; 129];
        bad.payload_len = 129;
        bad.total_len = 48 + 129;
        assert_eq!(
            bad.into_fact(observation()),
            Err(CaptureDecodeError::PayloadPrefix)
        );
    }

    #[test]
    fn identity_and_cpu_are_checked_without_truncation() {
        let mut bad = observation();
        bad.cpu = u32::from(u16::MAX) + 1;
        assert_eq!(
            packet(true).into_fact(bad),
            Err(CaptureDecodeError::CpuOverflow)
        );
        bad = observation();
        bad.socket_cookie = 0;
        assert_eq!(
            packet(true).into_fact(bad),
            Err(CaptureDecodeError::SocketIdentity)
        );
        let mut packet = packet(true);
        packet.source.set_port(0);
        assert_eq!(
            packet.into_fact(observation()),
            Err(CaptureDecodeError::SocketIdentity)
        );
    }

    #[test]
    fn ip_length_limits_and_zero_payload_do_not_create_bytes() {
        for ipv6 in [false, true] {
            let mut packet = packet(ipv6);
            packet.payload_len = if ipv6 { 65527 } else { 65507 };
            packet.total_len = u32::from(packet.ip_header_len) + 8 + u32::from(packet.payload_len);
            assert!(packet.into_fact(observation()).is_ok());
            packet.payload_len = 0;
            packet.total_len = u32::from(packet.ip_header_len) + 8;
            packet.payload_prefix = &[];
            let FactKind::PacketMeta(meta) = packet.into_fact(observation()).unwrap().kind else {
                panic!()
            };
            assert!(meta.payload_bytes.is_empty());
            assert_eq!(meta.payload_byte0, None);
        }
    }
}
