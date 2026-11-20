use std::time::Duration;
use time::OffsetDateTime;

use super::FileAttributes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub filename: String,
    pub longname: String,
    pub attrs: FileAttributes,
}

impl File {
    /// Omits `longname` and set dummy `attributes`. This is mainly used for [`crate::server::Handler::realpath`] as per the standard
    pub fn dummy<S: Into<String>>(filename: S) -> Self {
        Self {
            filename: filename.into(),
            longname: "".to_string(),
            attrs: FileAttributes::default(),
        }
    }

    /// Implies the use of longname
    pub fn new<S: Into<String>>(filename: S, attrs: FileAttributes) -> Self {
        let mut file = Self {
            filename: filename.into(),
            longname: "".to_string(),
            attrs,
        };
        file.longname = file.longname();
        file
    }

    /// Get formed longname
    pub fn longname(&self) -> String {
        let directory = if self.attrs.is_dir() { "d" } else { "-" };
        let permissions = self.attrs.permissions().to_string();

        let size = self.attrs.size.unwrap_or(0);
        let mtime = self.attrs.mtime.unwrap_or(0);

        let datetime = OffsetDateTime::UNIX_EPOCH + Duration::from_secs(mtime as u64);
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let delayed = format!(
            "{} {:02} {:04} {:02}:{:02}",
            months[datetime.month() as usize - 1],
            datetime.day(),
            datetime.year(),
            datetime.hour(),
            datetime.minute(),
        );

        format!(
            "{directory}{permissions} 0 {} {} {size} {delayed} {}",
            if let Some(user) = &self.attrs.user {
                user.to_string()
            } else {
                self.attrs.uid.unwrap_or(0).to_string()
            },
            if let Some(group) = &self.attrs.group {
                group.to_string()
            } else {
                self.attrs.gid.unwrap_or(0).to_string()
            },
            self.filename
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longname_preserves_utc_format_across_sftp_timestamp_range() {
        for (seconds, expected) in [
            (0, "Jan 01 1970 00:00"),
            (951_827_696, "Feb 29 2000 12:34"),
            (u32::MAX, "Feb 07 2106 06:28"),
        ] {
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o100700);
            attrs.size = Some(42);
            attrs.uid = Some(1000);
            attrs.gid = Some(1001);
            attrs.mtime = Some(seconds);
            let file = File::new("artifact", attrs);
            assert_eq!(
                file.longname,
                format!("-rwx------ 0 1000 1001 42 {expected} artifact")
            );
        }
    }
}
