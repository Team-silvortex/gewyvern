use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputLimits {
    stdout_bytes: usize,
    stderr_bytes: usize,
}

impl OutputLimits {
    pub(crate) const fn new(stdout_bytes: usize, stderr_bytes: usize) -> Self {
        Self {
            stdout_bytes,
            stderr_bytes,
        }
    }
}

pub(crate) fn run_command_output(
    command: &mut Command,
    timeout: Duration,
    limits: OutputLimits,
    description: &str,
) -> io::Result<Output> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{description} requires a non-zero timeout"),
        ));
    }
    if limits.stdout_bytes == 0 || limits.stderr_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{description} requires non-zero output limits"),
        ));
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to run {description}: {error}"),
        )
    })?;
    let Some(stdout) = child.stdout.take() else {
        terminate_child(&mut child);
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!("failed to capture {description} stdout"),
        ));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child(&mut child);
        return Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!("failed to capture {description} stderr"),
        ));
    };
    let stdout_reader =
        match spawn_stream_reader(stdout, limits.stdout_bytes, description, "stdout") {
            Ok(reader) => reader,
            Err(error) => {
                terminate_child(&mut child);
                return Err(error);
            }
        };
    let stderr_reader =
        match spawn_stream_reader(stderr, limits.stderr_bytes, description, "stderr") {
            Ok(reader) => reader,
            Err(error) => {
                terminate_child(&mut child);
                return Err(error);
            }
        };
    let started = Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate_child(&mut child);
                return Err(io::Error::new(
                    error.kind(),
                    format!("failed to wait for {description}: {error}"),
                ));
            }
        }

        let elapsed = started.elapsed();
        if elapsed >= timeout {
            let termination = terminate_child(&mut child)
                .map(|error| format!("; child termination warning: {error}"))
                .unwrap_or_default();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "{description} timed out after {:.3}s{termination}",
                    timeout.as_secs_f64()
                ),
            ));
        }

        thread::sleep(COMMAND_POLL_INTERVAL.min(timeout.saturating_sub(elapsed)));
    };

    let stdout = receive_stream(stdout_reader, started, timeout, description, "stdout")?;
    let stderr = receive_stream(stderr_reader, started, timeout, description, "stderr")?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn read_stream(mut stream: impl Read, max_bytes: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut overflowed = false;
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if !overflowed {
            let remaining = max_bytes.saturating_sub(bytes.len());
            let retained = remaining.min(read);
            bytes.extend_from_slice(&buffer[..retained]);
            overflowed = retained < read;
        }
    }
    if overflowed {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("captured output exceeded {max_bytes} bytes"),
        ))
    } else {
        Ok(bytes)
    }
}

fn spawn_stream_reader(
    stream: impl Read + Send + 'static,
    max_bytes: usize,
    description: &str,
    stream_name: &str,
) -> io::Result<Receiver<io::Result<Vec<u8>>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name(format!("bounded-{stream_name}-reader"))
        .spawn(move || {
            let _ = sender.send(read_stream(stream, max_bytes));
        })
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to start {description} {stream_name} reader: {error}"),
            )
        })?;
    Ok(receiver)
}

fn receive_stream(
    reader: Receiver<io::Result<Vec<u8>>>,
    started: Instant,
    timeout: Duration,
    description: &str,
    stream_name: &str,
) -> io::Result<Vec<u8>> {
    let remaining = timeout.saturating_sub(started.elapsed());
    reader
        .recv_timeout(remaining)
        .map_err(|error| match error {
            RecvTimeoutError::Timeout => io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "{description} timed out after {:.3}s while draining {stream_name}",
                    timeout.as_secs_f64()
                ),
            ),
            RecvTimeoutError::Disconnected => io::Error::new(
                io::ErrorKind::BrokenPipe,
                format!("{description} {stream_name} reader terminated unexpectedly"),
            ),
        })?
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to read {description} {stream_name}: {error}"),
            )
        })
}

fn terminate_child(child: &mut Child) -> Option<String> {
    let mut warnings = Vec::new();
    if let Err(error) = child.kill()
        && error.kind() != io::ErrorKind::InvalidInput
    {
        warnings.push(format!("kill failed: {error}"));
    }
    if let Err(error) = child.wait() {
        warnings.push(format!("wait failed: {error}"));
    }
    (!warnings.is_empty()).then(|| warnings.join(", "))
}
