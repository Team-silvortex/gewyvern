//! Deadline-bounded command capture. Unix pipes are cancellable; other platforms
//! retain a fixed worker ceiling even if descendants keep inherited handles open.
use std::io::{self, Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_IO_WORKERS: usize = 64;
static IO_WORKERS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLimits {
    stdout_bytes: usize,
    stderr_bytes: usize,
}
impl OutputLimits {
    pub const fn new(stdout_bytes: usize, stderr_bytes: usize) -> Self {
        Self {
            stdout_bytes,
            stderr_bytes,
        }
    }
}

struct Permit;
impl Permit {
    fn reserve(count: usize) -> io::Result<Vec<Self>> {
        IO_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(count)
                    .filter(|next| *next <= MAX_IO_WORKERS)
            })
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "command I/O worker limit reached",
                )
            })?;
        Ok((0..count).map(|_| Self).collect())
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        IO_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Session {
    child: Child,
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    completed: bool,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if !self.completed {
            // Keep the leader unreaped until all pipes finish or cleanup starts:
            // its reserved PID prevents a signal reaching a reused process-group ID.
            #[cfg(unix)]
            if let Ok(pid) = i32::try_from(self.child.id())
                && pid > 1
            {
                // SAFETY: spawn created this private group; its leader is unreaped.
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        #[cfg(unix)]
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        // Non-Unix blocking handles cannot be portably cancelled. Detached workers
        // keep their permits until I/O returns, rather than replenishing on timeout.
    }
}

/// Captures a direct command without a shell. One deadline covers payload write,
/// both output drains and child completion, not just the leader's lifetime.
/// Failure kills the owned Unix group, not descendants that escape it. Success
/// permits descendants only if they close captured pipes. OS spawn/reaping is
/// not preemptible, so this is not a hard real-time execution sandbox.
pub fn run_command_output(
    command: &mut Command,
    stdin_payload: Option<&[u8]>,
    timeout: Duration,
    limits: OutputLimits,
    description: &str,
) -> io::Result<Output> {
    run(command, stdin_payload, timeout, limits).map_err(|error| {
        let message = if error.kind() == io::ErrorKind::TimedOut {
            format!(
                "{description} timed out after {:.3}s: {error}",
                timeout.as_secs_f64()
            )
        } else {
            format!("{description}: {error}")
        };
        io::Error::new(error.kind(), message)
    })
}

fn run(
    command: &mut Command,
    payload: Option<&[u8]>,
    timeout: Duration,
    limits: OutputLimits,
) -> io::Result<Output> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .filter(|_| !timeout.is_zero())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "requires a finite non-zero timeout",
            )
        })?;
    if limits.stdout_bytes == 0 || limits.stderr_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "requires non-zero output limits",
        ));
    }
    let mut permits = Permit::reserve(2 + usize::from(payload.is_some()))?.into_iter();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    if payload.is_some() {
        command.stdin(Stdio::piped());
    }
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut session = Session {
        child,
        stop: Arc::new(AtomicBool::new(false)),
        workers: Vec::new(),
        completed: false,
    };
    let stdout = session
        .child
        .stdout
        .take()
        .ok_or_else(|| missing_pipe("stdout"))?;
    let stderr = session
        .child
        .stderr
        .take()
        .ok_or_else(|| missing_pipe("stderr"))?;
    #[cfg(unix)]
    {
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
    }
    let out = spawn_worker(&mut session, permits.next(), "stdout", move |stop| {
        read_stream(stdout, limits.stdout_bytes, deadline, &stop, "stdout")
    })?;
    let err = spawn_worker(&mut session, permits.next(), "stderr", move |stop| {
        read_stream(stderr, limits.stderr_bytes, deadline, &stop, "stderr")
    })?;
    let input = if let Some(payload) = payload {
        let stdin = session
            .child
            .stdin
            .take()
            .ok_or_else(|| missing_pipe("stdin"))?;
        #[cfg(unix)]
        nonblocking(&stdin)?;
        let payload = payload.to_vec();
        Some(spawn_worker(
            &mut session,
            permits.next(),
            "stdin",
            move |stop| write_payload(stdin, &payload, deadline, &stop),
        )?)
    } else {
        None
    };
    let mut stdout = None;
    let mut stderr = None;
    let mut written = input.is_none().then_some(());
    loop {
        // The clock wins over late buffered success from a worker.
        check_deadline(
            deadline,
            &session.stop,
            if stdout.is_none() {
                "draining stdout"
            } else if stderr.is_none() {
                "draining stderr"
            } else if written.is_none() {
                "writing stdin"
            } else {
                "waiting for child"
            },
        )?;
        receive(&out, &mut stdout)?;
        receive(&err, &mut stderr)?;
        if let Some(input) = &input {
            receive(input, &mut written)?;
        }
        if stdout.is_some()
            && stderr.is_some()
            && written.is_some()
            && let Some(status) = session.child.try_wait()?
        {
            session.completed = true;
            check_deadline(deadline, &session.stop, "waiting for child")?;
            return Ok(Output {
                status,
                stdout: stdout.take().ok_or_else(|| missing_pipe("stdout"))?,
                stderr: stderr.take().ok_or_else(|| missing_pipe("stderr"))?,
            });
        }
        thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

fn missing_pipe(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::BrokenPipe,
        format!("command {name} unavailable"),
    )
}
fn spawn_worker<T: Send + 'static>(
    session: &mut Session,
    permit: Option<Permit>,
    name: &str,
    run: impl FnOnce(Arc<AtomicBool>) -> io::Result<T> + Send + 'static,
) -> io::Result<Receiver<io::Result<T>>> {
    let permit = permit.ok_or_else(|| io::Error::other("command I/O permit unavailable"))?;
    let (send, receive) = mpsc::sync_channel(1);
    let stop = Arc::clone(&session.stop);
    let worker = thread::Builder::new()
        .name(format!("bounded-{name}"))
        .spawn(move || {
            let _permit = permit;
            let _ = send.send(run(stop));
        })?;
    session.workers.push(worker);
    Ok(receive)
}
fn receive<T>(receiver: &Receiver<io::Result<T>>, output: &mut Option<T>) -> io::Result<()> {
    if output.is_none() {
        match receiver.try_recv() {
            Ok(result) => *output = Some(result?),
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("command I/O worker terminated"));
            }
        }
    }
    Ok(())
}
fn check_deadline(deadline: Instant, stop: &AtomicBool, phase: &str) -> io::Result<()> {
    if Instant::now() >= deadline || stop.load(Ordering::Acquire) {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("command timed out while {phase}"),
        ))
    } else {
        Ok(())
    }
}
fn read_stream(
    mut stream: impl Read,
    limit: usize,
    deadline: Instant,
    stop: &AtomicBool,
    name: &str,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    let phase = format!("draining {name}");
    loop {
        check_deadline(deadline, stop, &phase)?;
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(output),
            Ok(count) => {
                if count > limit.saturating_sub(output.len()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{name} captured output exceeded {limit} bytes"),
                    ));
                }
                output.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL_INTERVAL),
            Err(error) => return Err(error),
        }
    }
}
fn write_payload(
    mut stream: impl Write,
    payload: &[u8],
    deadline: Instant,
    stop: &AtomicBool,
) -> io::Result<()> {
    let mut offset = 0;
    while offset < payload.len() {
        check_deadline(deadline, stop, "writing stdin")?;
        match stream.write(&payload[offset..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "command stdin write returned zero",
                ));
            }
            Ok(count) => offset += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL_INTERVAL),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
#[cfg(unix)]
fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    // SAFETY: the borrowed pipe owns this valid descriptor for both calls.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
