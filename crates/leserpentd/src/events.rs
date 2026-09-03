use std::io;
use std::time::{Duration, Instant};

use leserpent_domain::{Revision, RuntimeProjection};
use leserpent_protocol::transport_safety::is_http_header_name;
use leserpent_protocol::{
    EVENT_SCHEMA_VERSION, EventEnvelope, MAX_PROTOCOL_MESSAGE_BYTES, ProtocolEvent,
    RemoteRuntimeProjection, encode_event,
};
use tungstenite::error::Error as WebSocketError;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::http::HeaderValue;
use tungstenite::protocol::{Message, WebSocketConfig};
use tungstenite::{WebSocket, accept_hdr_with_config};

use crate::remote::{PrefixedStream, RemoteTlsStream};
use crate::wire::constant_time_equals;

pub(crate) const MAX_EVENT_SESSIONS: usize = 32;
const EVENT_SUBPROTOCOL: &str = "leserpent.events.v1";
const EVENT_SOCKET_SEND_BUFFER_BYTES: usize = 64 * 1024;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const MAX_INBOUND_FRAMES_PER_TICK: usize = 4;

pub(crate) struct EventSession {
    socket: WebSocket<PrefixedStream<RemoteTlsStream>>,
    requested_after: Option<Revision>,
    last_sent: Option<Revision>,
    last_activity: Instant,
}

impl EventSession {
    #[allow(clippy::result_large_err)]
    pub(crate) fn upgrade(
        stream: RemoteTlsStream,
        prefix: Vec<u8>,
        expected_token: &[u8],
    ) -> Result<Self, String> {
        let requested_after = validate_upgrade(&prefix, expected_token)?;
        let stream = PrefixedStream::new(prefix, stream);
        let config = WebSocketConfig::default()
            .read_buffer_size(4 * 1024)
            .write_buffer_size(0)
            .max_write_buffer_size(MAX_PROTOCOL_MESSAGE_BYTES + 1024)
            .max_message_size(Some(MAX_PROTOCOL_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_PROTOCOL_MESSAGE_BYTES))
            .accept_unmasked_frames(false);
        let callback = |_request: &Request, mut response: Response| {
            response.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                HeaderValue::from_static(EVENT_SUBPROTOCOL),
            );
            Ok(response)
        };
        let mut socket = accept_hdr_with_config(stream, callback, Some(config))
            .map_err(|_| "WebSocket upgrade failed".to_string())?;
        socket
            .get_mut()
            .set_send_buffer_size(EVENT_SOCKET_SEND_BUFFER_BYTES)
            .map_err(|error| error.to_string())?;
        socket
            .get_mut()
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            socket,
            requested_after,
            last_sent: None,
            last_activity: Instant::now(),
        })
    }

    pub(crate) fn poll(&mut self, revision: Revision, runtimes: &[RuntimeProjection]) -> bool {
        if !self.poll_inbound() {
            return false;
        }
        let event = if self.last_sent.is_none() {
            match self.requested_after.take() {
                Some(requested_after) if requested_after > revision => {
                    self.last_sent = Some(revision);
                    Some(ProtocolEvent::ResyncRequired {
                        requested_after,
                        current_revision: revision,
                    })
                }
                Some(requested_after) if requested_after == revision => {
                    self.last_sent = Some(revision);
                    Some(ProtocolEvent::Heartbeat { revision })
                }
                resumed_after => {
                    self.last_sent = Some(revision);
                    Some(snapshot_event(revision, resumed_after, runtimes))
                }
            }
        } else if self.last_sent != Some(revision) {
            let resumed_after = self.last_sent;
            self.last_sent = Some(revision);
            Some(snapshot_event(revision, resumed_after, runtimes))
        } else if self.last_activity.elapsed() >= HEARTBEAT_INTERVAL {
            Some(ProtocolEvent::Heartbeat { revision })
        } else {
            None
        };
        let Some(event) = event else {
            return true;
        };
        let payload = match encode_event(&EventEnvelope {
            schema_version: EVENT_SCHEMA_VERSION,
            event,
        }) {
            Ok(payload) if payload.len() <= MAX_PROTOCOL_MESSAGE_BYTES => payload,
            _ => return false,
        };
        match self.socket.send(Message::text(
            String::from_utf8(payload).expect("event JSON is UTF-8"),
        )) {
            Ok(()) => {
                self.last_activity = Instant::now();
                true
            }
            Err(WebSocketError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    fn poll_inbound(&mut self) -> bool {
        match self.socket.flush() {
            Ok(()) => {}
            Err(WebSocketError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => return false,
        }
        for _ in 0..MAX_INBOUND_FRAMES_PER_TICK {
            match self.socket.read() {
                Ok(Message::Ping(_) | Message::Pong(_)) => self.last_activity = Instant::now(),
                Ok(Message::Close(_)) => return false,
                Ok(Message::Text(_) | Message::Binary(_) | Message::Frame(_)) => {
                    let _ = self.socket.close(None);
                    return false;
                }
                Err(WebSocketError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                    return false;
                }
                Err(_) => return false,
            }
        }
        true
    }
}

pub(crate) fn is_event_upgrade(prefix: &[u8]) -> bool {
    http_header_bytes(prefix)
        .filter(|header| header.is_ascii())
        .and_then(|header| std::str::from_utf8(header).ok())
        .and_then(|header| header.split("\r\n").next())
        .is_some_and(|line| line.starts_with("GET /v1/events"))
}

fn http_header_bytes(prefix: &[u8]) -> Option<&[u8]> {
    prefix
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| &prefix[..position])
}

fn validate_upgrade(prefix: &[u8], expected_token: &[u8]) -> Result<Option<Revision>, String> {
    let header = http_header_bytes(prefix).ok_or("invalid WebSocket headers")?;
    if !header.is_ascii() {
        return Err("invalid WebSocket headers".into());
    }
    let header = std::str::from_utf8(header).map_err(|_| "invalid WebSocket headers")?;
    let mut lines = header.split("\r\n");
    let request_line = lines.next().ok_or("missing WebSocket request line")?;
    let parts = request_line.split(' ').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "GET" || parts[2] != "HTTP/1.1" {
        return Err("invalid WebSocket request line".into());
    }
    let requested_after = parse_event_target(parts[1])?;
    let mut authorization = None;
    let mut subprotocol = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or("invalid WebSocket header")?;
        if !is_http_header_name(name) {
            return Err("invalid WebSocket header".into());
        }
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            if authorization.replace(value).is_some() {
                return Err("duplicate WebSocket authorization".into());
            }
        } else if name.eq_ignore_ascii_case("sec-websocket-protocol")
            && subprotocol.replace(value).is_some()
        {
            return Err("duplicate WebSocket subprotocol".into());
        }
    }
    let token = authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if !constant_time_equals(token.as_bytes(), expected_token) {
        return Err("WebSocket authentication failed".into());
    }
    if subprotocol != Some(EVENT_SUBPROTOCOL) {
        return Err("WebSocket event subprotocol is required".into());
    }
    Ok(requested_after)
}

fn parse_event_target(target: &str) -> Result<Option<Revision>, String> {
    if target == "/v1/events" {
        return Ok(None);
    }
    let value = target
        .strip_prefix("/v1/events?after_revision=")
        .ok_or("invalid WebSocket event target")?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid WebSocket event cursor".into());
    }
    value
        .parse::<u64>()
        .map(Revision)
        .map(Some)
        .map_err(|_| "invalid WebSocket event cursor".into())
}

fn snapshot_event(
    revision: Revision,
    resumed_after: Option<Revision>,
    runtimes: &[RuntimeProjection],
) -> ProtocolEvent {
    ProtocolEvent::RuntimeSnapshot {
        revision,
        resumed_after,
        runtimes: runtimes
            .iter()
            .cloned()
            .map(RemoteRuntimeProjection::from)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &[u8] = b"0123456789abcdef0123456789abcdef";

    fn upgrade_request(target: &str, token: &str, protocols: &[&str]) -> Vec<u8> {
        let mut request = format!(
            "GET {target} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n"
        );
        for protocol in protocols {
            request.push_str(&format!("Sec-WebSocket-Protocol: {protocol}\r\n"));
        }
        request.push_str("Upgrade: websocket\r\nConnection: Upgrade\r\n\r\n");
        request.into_bytes()
    }

    #[test]
    fn event_target_cursor_is_strict_and_bounded_to_u64() {
        assert_eq!(parse_event_target("/v1/events").unwrap(), None);
        assert_eq!(
            parse_event_target("/v1/events?after_revision=42").unwrap(),
            Some(Revision(42))
        );
        for target in [
            "/v1/events?after_revision=",
            "/v1/events?after_revision=-1",
            "/v1/events?after_revision=1&token=secret",
            "/v1/events/other",
        ] {
            assert!(parse_event_target(target).is_err());
        }
    }

    #[test]
    fn event_upgrade_requires_exact_auth_subprotocol_and_target() {
        assert_eq!(
            validate_upgrade(
                &upgrade_request(
                    "/v1/events?after_revision=42",
                    std::str::from_utf8(TOKEN).unwrap(),
                    &[EVENT_SUBPROTOCOL],
                ),
                TOKEN,
            )
            .unwrap(),
            Some(Revision(42))
        );
        assert!(
            validate_upgrade(
                &upgrade_request(
                    "/v1/events",
                    "fedcba9876543210fedcba9876543210",
                    &[EVENT_SUBPROTOCOL],
                ),
                TOKEN,
            )
            .is_err()
        );
        assert!(
            validate_upgrade(
                &upgrade_request("/v1/events", std::str::from_utf8(TOKEN).unwrap(), &[],),
                TOKEN,
            )
            .is_err()
        );
        assert!(
            validate_upgrade(
                &upgrade_request(
                    "/v1/events",
                    std::str::from_utf8(TOKEN).unwrap(),
                    &[EVENT_SUBPROTOCOL, EVENT_SUBPROTOCOL],
                ),
                TOKEN,
            )
            .is_err()
        );
        assert!(
            validate_upgrade(
                &upgrade_request(
                    "/v1/events?after_revision=1&token=secret",
                    std::str::from_utf8(TOKEN).unwrap(),
                    &[EVENT_SUBPROTOCOL],
                ),
                TOKEN,
            )
            .is_err()
        );
    }

    #[test]
    fn event_upgrade_parses_only_the_ascii_header_prefix() {
        let mut request = upgrade_request(
            "/v1/events",
            std::str::from_utf8(TOKEN).unwrap(),
            &[EVENT_SUBPROTOCOL],
        );
        request.extend_from_slice(&[0x82, 0xff, 0x00]);

        assert!(is_event_upgrade(&request));
        assert_eq!(validate_upgrade(&request, TOKEN).unwrap(), None);

        let malformed = String::from_utf8(upgrade_request(
            "/v1/events",
            std::str::from_utf8(TOKEN).unwrap(),
            &[EVENT_SUBPROTOCOL],
        ))
        .unwrap()
        .replace("Upgrade: websocket", "Upgrade : websocket");
        assert!(validate_upgrade(malformed.as_bytes(), TOKEN).is_err());
    }
}
