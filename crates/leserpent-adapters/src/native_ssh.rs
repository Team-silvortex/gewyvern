use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use russh::{ChannelMsg, client, keys::HashAlg};
use russh_sftp::{
    client::SftpSession,
    protocol::{FileAttributes, OpenFlags, StatusCode},
};
use tokio::io::AsyncWriteExt;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSshError {
    Authentication,
    HostKeyRejected,
    Transport,
    UploadRejected,
    CommandRejected,
    InvalidResponse,
}

pub(crate) struct NativeSshJob<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub host_key_sha256: &'a str,
    pub password: &'a str,
    pub staging_path: &'a str,
    pub artifact: &'a [u8],
    pub artifact_sha256: &'a str,
    pub command: &'a str,
    pub stdin: &'a [u8],
    pub max_stdout_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeSshClient {
    timeout: Duration,
}

impl Default for NativeSshClient {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
        }
    }
}

impl NativeSshClient {
    pub fn with_timeout(timeout: Duration) -> Result<Self, String> {
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err("SSH timeout must be between 1ms and 300s".into());
        }
        Ok(Self { timeout })
    }

    pub fn execute(&self, job: NativeSshJob<'_>) -> Result<Zeroizing<Vec<u8>>, NativeSshError> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| NativeSshError::Transport)?;
                    runtime.block_on(async {
                        match tokio::time::timeout(self.timeout, execute_native(&job)).await {
                            Ok(result) => result,
                            Err(_) => {
                                let _ = tokio::time::timeout(
                                    Duration::from_secs(60),
                                    remove_timed_out_staging(&job),
                                )
                                .await;
                                Err(NativeSshError::Transport)
                            }
                        }
                    })
                })
                .join()
                .map_err(|_| NativeSshError::Transport)?
        })
    }
}

struct PinnedHostKey {
    expected: String,
    checked: Arc<AtomicBool>,
    matched: Arc<AtomicBool>,
}

impl client::Handler for PinnedHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let actual = server_public_key.fingerprint(HashAlg::Sha256).to_string();
        let matched = actual == self.expected;
        self.matched.store(matched, Ordering::Release);
        self.checked.store(true, Ordering::Release);
        Ok(matched)
    }
}

async fn execute_native(job: &NativeSshJob<'_>) -> Result<Zeroizing<Vec<u8>>, NativeSshError> {
    let mut session = connect_authenticated(job).await?;
    let sftp = open_sftp(&mut session).await?;
    let execution = async {
        remove_staging_if_present(&sftp, job.staging_path).await?;
        upload_and_verify(&sftp, job).await?;
        run_command(&mut session, job).await
    }
    .await;
    let _ = sftp.remove_file(job.staging_path).await;
    let _ = sftp.close().await;
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await;
    execution
}

async fn connect_authenticated(
    job: &NativeSshJob<'_>,
) -> Result<client::Handle<PinnedHostKey>, NativeSshError> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(15)),
        ..Default::default()
    });
    let host_key_checked = Arc::new(AtomicBool::new(false));
    let host_key_matched = Arc::new(AtomicBool::new(false));
    let handler = PinnedHostKey {
        expected: job.host_key_sha256.to_string(),
        checked: host_key_checked.clone(),
        matched: host_key_matched.clone(),
    };
    let connection = client::connect(config, (job.host, job.port), handler).await;
    if host_key_checked.load(Ordering::Acquire) && !host_key_matched.load(Ordering::Acquire) {
        return Err(NativeSshError::HostKeyRejected);
    }
    let mut session = connection.map_err(|_| NativeSshError::Transport)?;
    if !host_key_matched.load(Ordering::Acquire) {
        return Err(NativeSshError::HostKeyRejected);
    }
    let authentication = session
        .authenticate_password(job.username, job.password)
        .await
        .map_err(|_| NativeSshError::Authentication)?;
    if !authentication.success() {
        return Err(NativeSshError::Authentication);
    }
    Ok(session)
}

async fn open_sftp(
    session: &mut client::Handle<PinnedHostKey>,
) -> Result<SftpSession, NativeSshError> {
    let channel = session
        .channel_open_session()
        .await
        .map_err(|_| NativeSshError::Transport)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|_| NativeSshError::Transport)?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|_| NativeSshError::Transport)
}

async fn remove_staging_if_present(
    sftp: &SftpSession,
    staging_path: &str,
) -> Result<(), NativeSshError> {
    match sftp.remove_file(staging_path).await {
        Ok(()) => Ok(()),
        Err(russh_sftp::client::error::Error::Status(status))
            if status.status_code == StatusCode::NoSuchFile =>
        {
            Ok(())
        }
        Err(_) => Err(NativeSshError::UploadRejected),
    }
}

async fn upload_and_verify(
    sftp: &SftpSession,
    job: &NativeSshJob<'_>,
) -> Result<(), NativeSshError> {
    let mut attributes = FileAttributes::empty();
    attributes.permissions = Some(0o100700);
    let mut file = sftp
        .open_with_flags_and_attributes(
            job.staging_path,
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
            attributes,
        )
        .await
        .map_err(|_| NativeSshError::UploadRejected)?;
    file.write_all(job.artifact)
        .await
        .map_err(|_| NativeSshError::UploadRejected)?;
    file.flush()
        .await
        .map_err(|_| NativeSshError::UploadRejected)?;
    file.shutdown()
        .await
        .map_err(|_| NativeSshError::UploadRejected)?;
    let metadata = sftp
        .metadata(job.staging_path)
        .await
        .map_err(|_| NativeSshError::UploadRejected)?;
    if metadata.size != Some(job.artifact.len() as u64)
        || metadata.permissions.map(|mode| mode & 0o777) != Some(0o700)
        || job.artifact_sha256.len() != 64
    {
        return Err(NativeSshError::UploadRejected);
    }
    Ok(())
}

async fn run_command(
    session: &mut client::Handle<PinnedHostKey>,
    job: &NativeSshJob<'_>,
) -> Result<Zeroizing<Vec<u8>>, NativeSshError> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|_| NativeSshError::Transport)?;
    channel
        .exec(true, job.command)
        .await
        .map_err(|_| NativeSshError::Transport)?;
    channel
        .data(job.stdin)
        .await
        .map_err(|_| NativeSshError::Transport)?;
    channel.eof().await.map_err(|_| NativeSshError::Transport)?;

    let mut stdout = Zeroizing::new(Vec::new());
    let mut exit_status = None;
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => {
                if stdout.len().saturating_add(data.len()) > job.max_stdout_bytes {
                    return Err(NativeSshError::InvalidResponse);
                }
                stdout.extend_from_slice(&data);
            }
            ChannelMsg::ExtendedData { .. } => {}
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => exit_status = Some(status),
            _ => {}
        }
    }
    if exit_status != Some(0) {
        return Err(NativeSshError::CommandRejected);
    }
    Ok(stdout)
}

async fn remove_timed_out_staging(job: &NativeSshJob<'_>) -> Result<(), NativeSshError> {
    for attempt in 0..3 {
        if remove_timed_out_staging_once(job).await.is_ok() {
            return Ok(());
        }
        if attempt < 2 {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    Err(NativeSshError::Transport)
}

async fn remove_timed_out_staging_once(job: &NativeSshJob<'_>) -> Result<(), NativeSshError> {
    let mut session = connect_authenticated(job).await?;
    let sftp = open_sftp(&mut session).await?;
    remove_staging_if_present(&sftp, job.staging_path).await?;
    let _ = sftp.close().await;
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await;
    Ok(())
}
