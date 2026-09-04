use std::io::{self, Read};
use std::path::Path;

use silvortex_bounded_io::open_bounded_regular_file;

const MACH_O_ARM64_HEADER_BYTES: usize = 8;
const MAX_FAT_ARCHITECTURES: usize = 32;
const MAX_MACH_O_HEADER_BYTES: usize = 8 + MAX_FAT_ARCHITECTURES * 32;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;

pub fn is_mach_o_arm64(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01])
        || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xcf, 0x01, 0x00, 0x00, 0x0c])
    {
        return true;
    }
    let Some(magic_bytes) = bytes.get(..4) else {
        return false;
    };
    let Ok(magic): Result<[u8; 4], _> = magic_bytes.try_into() else {
        return false;
    };
    let (big_endian, entry_bytes) = match magic {
        [0xca, 0xfe, 0xba, 0xbe] => (true, 20),
        [0xca, 0xfe, 0xba, 0xbf] => (true, 32),
        [0xbe, 0xba, 0xfe, 0xca] => (false, 20),
        [0xbf, 0xba, 0xfe, 0xca] => (false, 32),
        _ => return false,
    };
    let Some(count_bytes) = bytes.get(4..8).and_then(|value| value.try_into().ok()) else {
        return false;
    };
    let count = if big_endian {
        u32::from_be_bytes(count_bytes)
    } else {
        u32::from_le_bytes(count_bytes)
    } as usize;
    if count == 0 || count > MAX_FAT_ARCHITECTURES {
        return false;
    }
    let Some(table_bytes) = count
        .checked_mul(entry_bytes)
        .and_then(|value| value.checked_add(8))
    else {
        return false;
    };
    if bytes.len() < table_bytes {
        return false;
    }
    (0..count).any(|index| {
        let offset = 8 + index * entry_bytes;
        let Some(raw) = bytes
            .get(offset..offset + 4)
            .and_then(|value| value.try_into().ok())
        else {
            return false;
        };
        let cpu_type = if big_endian {
            u32::from_be_bytes(raw)
        } else {
            u32::from_le_bytes(raw)
        };
        cpu_type == CPU_TYPE_ARM64
    })
}

pub fn file_is_mach_o_arm64(path: &Path) -> io::Result<bool> {
    let mut file = open_bounded_regular_file(path, u64::MAX)?;
    let mut header = [0_u8; MAX_MACH_O_HEADER_BYTES];
    let mut read = 0;
    while read < header.len() {
        let count = file.read(&mut header[read..])?;
        if count == 0 {
            break;
        }
        read += count;
    }
    Ok(read >= MACH_O_ARM64_HEADER_BYTES && is_mach_o_arm64(&header[..read]))
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

    #[test]
    fn recognizes_bounded_universal_mach_o_with_arm64_slice() {
        let mut universal = Vec::new();
        universal.extend_from_slice(&[0xca, 0xfe, 0xba, 0xbe]);
        universal.extend_from_slice(&2_u32.to_be_bytes());
        for (cpu_type, offset) in [(0x0100_0007_u32, 0x4000_u32), (CPU_TYPE_ARM64, 0x8000)] {
            universal.extend_from_slice(&cpu_type.to_be_bytes());
            universal.extend_from_slice(&0_u32.to_be_bytes());
            universal.extend_from_slice(&offset.to_be_bytes());
            universal.extend_from_slice(&0x1000_u32.to_be_bytes());
            universal.extend_from_slice(&14_u32.to_be_bytes());
        }
        assert!(is_mach_o_arm64(&universal));

        let mut x86_only = universal.clone();
        x86_only[28..32].copy_from_slice(&[0x01, 0x00, 0x00, 0x07]);
        assert!(!is_mach_o_arm64(&x86_only));

        let mut excessive = universal.clone();
        excessive[4..8].copy_from_slice(&33_u32.to_be_bytes());
        assert!(!is_mach_o_arm64(&excessive));
        assert!(!is_mach_o_arm64(&universal[..27]));
    }
}
