use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::ValidationError;
use super::metrics::Latency;

type Task<T> = JoinHandle<Result<T, ValidationError>>;

pub(in super::super) struct Traffic {
    stop: Arc<AtomicBool>,
    sockets: Vec<TcpStream>,
    starts: Vec<mpsc::Sender<Instant>>,
    ready: mpsc::Receiver<u64>,
    clients: Vec<Task<Latency>>,
    servers: Vec<Task<()>>,
}

fn exchange(socket: &mut TcpStream, sequence: u64, id: u8) -> Result<(), ValidationError> {
    let mut request = [id; 64];
    request[..8].copy_from_slice(&sequence.to_le_bytes());
    socket.write_all(&request)?;
    let mut response = [0; 64];
    socket.read_exact(&mut response)?;
    if response != request {
        return Err(ValidationError::new(
            "load response failed byte/sequence integrity",
        ));
    }
    Ok(())
}

fn client(
    mut socket: TcpStream,
    id: u8,
    stop: Arc<AtomicBool>,
    ready: mpsc::Sender<u64>,
    start: mpsc::Receiver<Instant>,
) -> Result<Latency, ValidationError> {
    let warmup = Instant::now() + Duration::from_secs(2);
    let mut sequence = 0;
    while Instant::now() < warmup && !stop.load(Ordering::Relaxed) {
        exchange(&mut socket, sequence, id)?;
        sequence += 1;
    }
    ready
        .send(sequence)
        .map_err(|e| ValidationError::new(e.to_string()))?;
    let deadline = start
        .recv_timeout(Duration::from_secs(30))
        .map_err(|e| ValidationError::new(e.to_string()))?;
    let mut latency = Latency::new();
    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        let begin = Instant::now();
        exchange(&mut socket, sequence, id)?;
        latency.record(begin.elapsed());
        sequence += 1;
    }
    Ok(latency)
}

fn server(mut socket: TcpStream) -> Result<(), ValidationError> {
    loop {
        let mut request = [0; 64];
        // A clean close is valid only between complete requests.
        if socket.read(&mut request[..1])? == 0 {
            return Ok(());
        }
        socket.read_exact(&mut request[1..])?;
        socket.write_all(&request)?;
    }
}

impl Traffic {
    pub(in super::super) fn spawn(
        pairs: Vec<(TcpStream, TcpStream)>,
    ) -> Result<Self, ValidationError> {
        let (tx, rx) = mpsc::channel();
        let mut this = Self {
            stop: Arc::new(AtomicBool::new(false)),
            sockets: Vec::new(),
            starts: Vec::new(),
            ready: rx,
            clients: Vec::new(),
            servers: Vec::new(),
        };
        for (id, (client_socket, peer)) in pairs.into_iter().enumerate() {
            for socket in [&client_socket, &peer] {
                socket.set_nodelay(true)?;
                socket.set_read_timeout(Some(Duration::from_secs(5)))?;
                socket.set_write_timeout(Some(Duration::from_secs(5)))?;
                this.sockets.push(socket.try_clone()?);
            }
            this.servers.push(
                thread::Builder::new()
                    .name(format!("capture-load-server-{id}"))
                    .spawn(move || server(peer))?,
            );
            let (begin_tx, begin_rx) = mpsc::channel();
            this.starts.push(begin_tx);
            let ready = tx.clone();
            let stop = Arc::clone(&this.stop);
            this.clients.push(
                thread::Builder::new()
                    .name(format!("capture-load-client-{id}"))
                    .spawn(move || {
                        let result =
                            client(client_socket, id as u8, Arc::clone(&stop), ready, begin_rx);
                        if result.is_err() {
                            stop.store(true, Ordering::Relaxed);
                        }
                        result
                    })?,
            );
        }
        Ok(this)
    }

    pub(in super::super) fn warmup(&self) -> Result<u64, ValidationError> {
        let mut count = 0;
        for _ in &self.clients {
            count += self
                .ready
                .recv_timeout(Duration::from_secs(15))
                .map_err(|e| ValidationError::new(e.to_string()))?;
        }
        Ok(count)
    }

    pub(in super::super) fn start(&self, deadline: Instant) -> Result<(), ValidationError> {
        for sender in &self.starts {
            sender
                .send(deadline)
                .map_err(|e| ValidationError::new(e.to_string()))?;
        }
        Ok(())
    }

    pub(in super::super) fn failed(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub(in super::super) fn finish(mut self) -> Result<Latency, ValidationError> {
        let mut combined = Latency::new();
        while let Some(task) = self.clients.pop() {
            combined.merge(
                task.join()
                    .map_err(|_| ValidationError::new("load client panicked"))??,
            );
        }
        // Close duplicate client descriptors so the peers observe EOF.
        for socket in self.sockets.iter().step_by(2) {
            socket.shutdown(Shutdown::Write)?;
        }
        while let Some(task) = self.servers.pop() {
            task.join()
                .map_err(|_| ValidationError::new("load server panicked"))??;
        }
        Ok(combined)
    }
}

impl Drop for Traffic {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.starts.clear();
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
