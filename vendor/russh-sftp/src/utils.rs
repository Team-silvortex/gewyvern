use bytes::Bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::Error;

pub(crate) fn unix(time: SystemTime) -> u32 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as u32,
        Err(error) => {
            // Preserve the old floor-to-seconds and wrapping SFTP v3 u32 cast.
            let duration = error.duration();
            0u32.wrapping_sub(duration.as_secs() as u32)
                .wrapping_sub(u32::from(duration.subsec_nanos() != 0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn unix_seconds_preserve_flooring_and_wrapping() {
        assert_eq!(unix(UNIX_EPOCH), 0);
        assert_eq!(unix(UNIX_EPOCH + Duration::from_nanos(999_999_999)), 0);
        assert_eq!(unix(UNIX_EPOCH + Duration::from_millis(1_500)), 1);
        assert_eq!(unix(UNIX_EPOCH - Duration::from_nanos(1)), u32::MAX);
        assert_eq!(unix(UNIX_EPOCH - Duration::from_secs(1)), u32::MAX);
        assert_eq!(
            unix(UNIX_EPOCH - Duration::from_millis(1_500)),
            u32::MAX - 1
        );
        assert_eq!(
            unix(UNIX_EPOCH + Duration::from_secs(u32::MAX.into())),
            u32::MAX
        );
        assert_eq!(
            unix(UNIX_EPOCH + Duration::from_secs(u32::MAX as u64 + 1)),
            0
        );
    }
}

pub(crate) async fn read_packet<S: AsyncRead + Unpin>(
    stream: &mut S,
    max_length: u32,
) -> Result<Bytes, Error> {
    let length = stream.read_u32().await?;
    if length > max_length {
        return Err(Error::BadMessage("packet length limit exceeded".to_owned()));
    }

    let mut buf = vec![0; length as usize];
    stream.read_exact(&mut buf).await?;

    Ok(Bytes::from(buf))
}
