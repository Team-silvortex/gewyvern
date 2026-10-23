//! Bounded product-wire JSON, not semantic admission or durable storage.
//!
//! Encoding operations borrow the value and run its serializer once. Callers must
//! bound recursive structures before entry. A custom serializer is trusted code:
//! its CPU, allocations and side effects are not limited or rolled back here.
//! Decoding checks the entire input byte length before entering serde, retains
//! serde_json's recursion limit and rejects trailing data. It does not bound the
//! decoded allocation size or sandbox a custom deserializer. Callers still own
//! semantic validation, authorization and any durable state transition.

use std::io::{self, Write};

use serde::{Deserialize, Serialize};

/// Separate from encoding errors to preserve exhaustive matches on the released API.
#[derive(Debug)]
pub enum BoundedJsonDecodeError {
    Limit { size: usize, limit: usize },
    Deserialize(serde_json::Error),
}

impl std::fmt::Display for BoundedJsonDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit { limit, .. } => write!(formatter, "input JSON exceeds {limit} bytes"),
            Self::Deserialize(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BoundedJsonDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit { .. } => None,
            Self::Deserialize(error) => Some(error),
        }
    }
}

/// Decode one complete JSON value, allowing the result to borrow from `bytes`.
/// Whitespace counts toward the input limit. No deserializer runs on oversized
/// input; once entered, its side effects are not rolled back on a parse failure.
pub fn decode<'de, T: Deserialize<'de>>(
    bytes: &'de [u8],
    limit: usize,
) -> Result<T, BoundedJsonDecodeError> {
    if bytes.len() > limit {
        return Err(BoundedJsonDecodeError::Limit {
            size: bytes.len(),
            limit,
        });
    }
    serde_json::from_slice(bytes).map_err(BoundedJsonDecodeError::Deserialize)
}

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
