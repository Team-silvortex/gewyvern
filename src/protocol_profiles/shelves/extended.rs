#[path = "extended_access_and_media.rs"]
mod extended_access_and_media;
#[path = "extended_identity_and_directory.rs"]
mod extended_identity_and_directory;
#[path = "extended_mail_and_queue.rs"]
mod extended_mail_and_queue;

pub(crate) use extended_access_and_media::{
    rdp_shelf, rtsp_shelf, sip_shelf, smb_shelf, socks5_shelf, ssh_shelf,
};
pub(crate) use extended_identity_and_directory::{
    imap_shelf, kerberos_shelf, ldap_shelf, pop3_shelf,
};
pub(crate) use extended_mail_and_queue::{amqp_shelf, smtp_shelf};
