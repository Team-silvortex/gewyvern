use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::super::load::metrics::Latency;
use super::ValidationError;

pub(super) const SIZES: [usize; 8] = [1, 7, 8, 63, 64, 65, 511, 4096];
pub(super) const MESSAGES: usize = SIZES.len() * 128;

#[derive(Clone, Copy, Debug)]
pub(super) enum Mode {
    Variable,
    Split,
    Coalesced,
}

impl Mode {
    fn group(self) -> usize {
        if matches!(self, Self::Coalesced) {
            4
        } else {
            1
        }
    }
    fn size(self, exchange: usize, id: usize) -> usize {
        (0..self.group())
            .map(|i| SIZES[(exchange * self.group() + i + id) % SIZES.len()])
            .sum()
    }
    pub(super) fn bytes(self, id: usize) -> u64 {
        (0..MESSAGES / self.group())
            .map(|i| self.size(i, id) as u64)
            .sum()
    }
}

pub(super) fn byte_at(offset: u64, id: usize, epoch: usize) -> u8 {
    (offset.wrapping_mul(131) ^ (offset >> 8) ^ (id as u64 * 47) ^ (epoch as u64 * 83)) as u8
}

fn verify(bytes: &[u8], offset: u64, id: usize, epoch: usize) -> Result<(), ValidationError> {
    if bytes
        .iter()
        .enumerate()
        .any(|(i, byte)| *byte != byte_at(offset + i as u64, id, epoch))
    {
        return Err(ValidationError::new(
            "mixed traffic full-byte integrity mismatch",
        ));
    }
    Ok(())
}

fn client(
    mut socket: TcpStream,
    id: usize,
    epoch: usize,
    mode: Mode,
) -> Result<Value, ValidationError> {
    let mut offset = 0;
    let mut latency = Latency::new();
    let exchanges = MESSAGES / mode.group();
    for exchange in 0..exchanges {
        let bytes: Vec<u8> = (0..mode.size(exchange, id))
            .map(|i| byte_at(offset + i as u64, id, epoch))
            .collect();
        let started = Instant::now();
        socket.write_all(&bytes)?;
        if exchange + 1 == exchanges {
            // FIN must not discard the final outstanding response.
            socket.shutdown(Shutdown::Write)?;
        }
        let mut response = vec![0; bytes.len()];
        socket.read_exact(&mut response)?;
        verify(&response, offset, id, epoch)?;
        latency.record(started.elapsed());
        offset += bytes.len() as u64;
    }
    if socket.read(&mut [0])? != 0 {
        return Err(ValidationError::new("mixed server sent trailing bytes"));
    }
    Ok(
        json!({"id":id,"logical_messages":MESSAGES,"exchanges":exchanges,"response_bytes":offset,
        "full_byte_integrity":true,"half_close_and_eof":true,"exchange_rtt":latency.report()}),
    )
}

fn server(
    mut socket: TcpStream,
    id: usize,
    epoch: usize,
    mode: Mode,
) -> Result<(), ValidationError> {
    let mut offset = 0;
    for exchange in 0..MESSAGES / mode.group() {
        let mut request = vec![0; mode.size(exchange, id)];
        socket.read_exact(&mut request)?;
        verify(&request, offset, id, epoch)?;
        if matches!(mode, Mode::Split) {
            let mut sent = 0;
            for end in [1, 8, 63, request.len()] {
                let end = end.min(request.len());
                if end > sent {
                    socket.write_all(&request[sent..end])?;
                    sent = end;
                    if sent < request.len() {
                        thread::sleep(Duration::from_micros(100));
                    }
                }
            }
        } else {
            socket.write_all(&request)?;
        }
        offset += request.len() as u64;
    }
    if socket.read(&mut [0])? != 0 {
        return Err(ValidationError::new("mixed client sent trailing bytes"));
    }
    socket.shutdown(Shutdown::Write)?;
    Ok(())
}

pub(super) struct Traffic {
    sockets: Vec<TcpStream>,
    failed: Arc<AtomicBool>,
    clients: Vec<JoinHandle<Result<Value, ValidationError>>>,
    servers: Vec<JoinHandle<Result<(), ValidationError>>>,
}

impl Traffic {
    pub(super) fn spawn(
        pairs: Vec<(TcpStream, TcpStream)>,
        epoch: usize,
        mode: Mode,
    ) -> Result<Self, ValidationError> {
        let mut this = Self {
            sockets: Vec::new(),
            failed: Arc::new(AtomicBool::new(false)),
            clients: Vec::new(),
            servers: Vec::new(),
        };
        for (id, (socket, peer)) in pairs.into_iter().enumerate() {
            for endpoint in [&socket, &peer] {
                endpoint.set_nodelay(true)?;
                endpoint.set_read_timeout(Some(Duration::from_secs(5)))?;
                endpoint.set_write_timeout(Some(Duration::from_secs(5)))?;
                this.sockets.push(endpoint.try_clone()?);
            }
            let failed = Arc::clone(&this.failed);
            this.servers.push(
                thread::Builder::new()
                    .name(format!("mixed-server-{id}"))
                    .spawn(move || {
                        let result = server(peer, id, epoch, mode);
                        if result.is_err() {
                            failed.store(true, Ordering::Release);
                        }
                        result
                    })?,
            );
            let failed = Arc::clone(&this.failed);
            this.clients.push(
                thread::Builder::new()
                    .name(format!("mixed-client-{id}"))
                    .spawn(move || {
                        let result = client(socket, id, epoch, mode);
                        if result.is_err() {
                            failed.store(true, Ordering::Release);
                        }
                        result
                    })?,
            );
        }
        Ok(this)
    }
    pub(super) fn finished(&self) -> bool {
        self.clients.iter().all(JoinHandle::is_finished)
    }
    pub(super) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    pub(super) fn finish(mut self) -> Result<Vec<Value>, ValidationError> {
        let mut reports = Vec::new();
        while let Some(task) = self.clients.pop() {
            reports.push(
                task.join()
                    .map_err(|_| ValidationError::new("mixed client panicked"))??,
            );
        }
        while let Some(task) = self.servers.pop() {
            task.join()
                .map_err(|_| ValidationError::new("mixed server panicked"))??;
        }
        reports.sort_by_key(|r| r["id"].as_u64());
        Ok(reports)
    }
}

impl Drop for Traffic {
    fn drop(&mut self) {
        for socket in &self.sockets {
            let _ = socket.shutdown(Shutdown::Both);
        }
        for task in self.clients.drain(..) {
            let _ = task.join();
        }
        for task in self.servers.drain(..) {
            let _ = task.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grouping_preserves_byte_totals_without_equating_messages_and_exchanges() {
        for id in 0..3 {
            assert_eq!(Mode::Variable.bytes(id), 616320);
            assert_eq!(Mode::Variable.bytes(id), Mode::Coalesced.bytes(id));
            assert_eq!(Mode::Split.bytes(id), Mode::Coalesced.bytes(id));
        }
        assert_eq!(MESSAGES / Mode::Coalesced.group(), 256);
        assert_ne!(byte_at(64, 0, 0), byte_at(64, 1, 0));
        assert_ne!(byte_at(64, 0, 0), byte_at(64, 0, 1));
    }
}
