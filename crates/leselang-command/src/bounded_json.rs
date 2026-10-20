//! Bounded product-wire encoding, not semantic admission or durable storage.
//!
//! Both operations borrow the value and run its serializer once. Callers must
//! bound recursive structures before entry. A custom serializer is trusted code:
//! its CPU, allocations and side effects are not limited or rolled back here.

use std::io::{self, Write};

use serde::Serialize;

#[derive(Debug)]
pub enum BoundedJsonError {
    Limit { attempted: usize, limit: usize },
    Serialize(serde_json::Error),
}

impl std::fmt::Display for BoundedJsonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit { limit, .. } => write!(formatter, "encoded JSON exceeds {limit} bytes"),
            Self::Serialize(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BoundedJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit { .. } => None,
            Self::Serialize(error) => Some(error),
        }
    }
}

/// Encode compact JSON without retaining more than `limit` output bytes.
/// Allocator capacity is not a memory quota. Errors never return partial output.
pub fn encode<T: Serialize + ?Sized>(value: &T, limit: usize) -> Result<Vec<u8>, BoundedJsonError> {
    let mut output = Vec::with_capacity(limit.min(4 * 1024));
    serialize_bounded(value, limit, Some(&mut output))?;
    Ok(output)
}

/// Count the same compact JSON writes without allocating an output buffer.
/// This is not a validation certificate for a later, possibly stateful serializer.
pub fn measure<T: Serialize + ?Sized>(value: &T, limit: usize) -> Result<usize, BoundedJsonError> {
    serialize_bounded(value, limit, None)
}

fn serialize_bounded<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
    output: Option<&mut Vec<u8>>,
) -> Result<usize, BoundedJsonError> {
    let mut writer = BoundedWriter {
        output,
        bytes: 0,
        limit,
        overflow: None,
    };
    let result = serde_json::to_writer(&mut writer, value);
    // A custom serializer may catch a failed write; the refusal remains sticky.
    if let Some(attempted) = writer.overflow {
        return Err(BoundedJsonError::Limit { attempted, limit });
    }
    result.map_err(BoundedJsonError::Serialize)?;
    Ok(writer.bytes)
}

struct BoundedWriter<'a> {
    output: Option<&'a mut Vec<u8>>,
    bytes: usize,
    limit: usize,
    overflow: Option<usize>,
}

impl Write for BoundedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.overflow.is_none() {
            match self.bytes.checked_add(buffer.len()) {
                Some(size) if size <= self.limit => {
                    if let Some(output) = &mut self.output {
                        output.extend_from_slice(buffer);
                    }
                    self.bytes += buffer.len();
                    return Ok(buffer.len());
                }
                size => self.overflow = Some(size.unwrap_or(usize::MAX)),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "encoded value exceeds limit",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
