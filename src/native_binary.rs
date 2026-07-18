use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

const MACH_O_ARM64_HEADER_BYTES: usize = 8;

pub fn is_mach_o_arm64(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01])
        || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xcf, 0x01, 0x00, 0x00, 0x0c])
}

pub fn file_is_mach_o_arm64(path: &Path) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let mut header = [0_u8; MACH_O_ARM64_HEADER_BYTES];
    let mut read = 0;
    while read < header.len() {
        let count = file.read(&mut header[read..])?;
        if count == 0 {
            break;
        }
        read += count;
    }
    Ok(read == header.len() && is_mach_o_arm64(&header))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_64_bit_arm_mach_o_headers() {
        assert!(is_mach_o_arm64(&[
            0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01,
        ]));
        assert!(is_mach_o_arm64(&[
            0xfe, 0xed, 0xfa, 0xcf, 0x01, 0x00, 0x00, 0x0c,
        ]));
        assert!(!is_mach_o_arm64(&[
            0xcf, 0xfa, 0xed, 0xfe, 0x07, 0x00, 0x00, 0x01,
        ]));
        assert!(!is_mach_o_arm64(b"#! /bin/sh"));
        assert!(!is_mach_o_arm64(&[0xcf, 0xfa, 0xed, 0xfe]));
    }
}
