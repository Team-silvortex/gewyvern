use super::*;
use std::io::{self, Write};

// Frozen pre-migration encoders; independent of the shared product codec.
pub(super) fn frozen_encode_json_capped<T: Serialize>(
    value: &T,
    limit: usize,
    label: &str,
) -> Result<Vec<u8>, Fault> {
    let mut writer = CappedWriter::new(limit);
    if let Err(error) = serde_json::to_writer(&mut writer, value) {
        if writer.overflowed {
            return Err(Fault {
                code: "LSV3002".to_string(),
                message: format!("{label} exceeds {limit} bytes"),
            });
        }
        return Err(Fault {
            code: "LSV3001".to_string(),
            message: format!("failed to encode {label}: {error}"),
        });
    }
    Ok(writer.output)
}

pub(super) fn frozen_validate_json_size_capped<T: Serialize>(
    value: &T,
    limit: usize,
    label: &str,
) -> Result<(), Fault> {
    let mut writer = CappedCounter::new(limit);
    if let Err(error) = serde_json::to_writer(&mut writer, value) {
        if writer.overflowed {
            return Err(Fault {
                code: "LSV3002".to_string(),
                message: format!("{label} exceeds {limit} bytes"),
            });
        }
        return Err(Fault {
            code: "LSV3001".to_string(),
            message: format!("failed to encode {label}: {error}"),
        });
    }
    Ok(())
}

struct CappedWriter {
    output: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl CappedWriter {
    fn new(limit: usize) -> Self {
        Self {
            output: Vec::new(),
            limit,
            overflowed: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.output.len().saturating_add(buffer.len()) > self.limit {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "continuation exceeds limit",
            ));
        }
        self.output.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct CappedCounter {
    bytes: usize,
    limit: usize,
    overflowed: bool,
}

impl CappedCounter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: 0,
            limit,
            overflowed: false,
        }
    }
}

impl Write for CappedCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.bytes.saturating_add(buffer.len()) > self.limit {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "encoded value exceeds limit",
            ));
        }
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn compare<T: Serialize>(value: &T) -> usize {
    let size = serde_json::to_vec(value).unwrap().len();
    let mut count = 0;
    for limit in [
        0,
        1,
        size - 1,
        size,
        size + 1,
        MAX_CONTINUATION_BYTES,
        MAX_JOURNAL_ENTRY_BYTES,
    ] {
        for label in [
            "continuation",
            "effect request",
            "terminal step",
            "structured merge output",
        ] {
            assert_eq!(
                encode_json_capped(value, limit, label),
                frozen_encode_json_capped(value, limit, label),
                "encode: {label} limit {limit}"
            );
            assert_eq!(
                validate_json_size_capped(value, limit, label),
                frozen_validate_json_size_capped(value, limit, label),
                "measure: {label} limit {limit}"
            );
            count += 1;
        }
    }
    count
}

#[test]
fn bounded_json_migration_keeps_all_operation_images_and_terminal_wire_errors() {
    let mut count = 0;
    for request in continuation_contract_tests::requests() {
        count += compare(&request);
        count += compare(&request.continuation);
        count += compare(&request.operation);
        count += compare(&request.budget);
        count += compare(&Step::Effect(Box::new(request)));
    }
    assert_eq!(count, 70 * 5 * 7 * 4);
}

#[test]
fn bounded_json_migration_keeps_native_serializer_faults_and_oversize_precedence() {
    struct Refuses;
    impl Serialize for Refuses {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("native codec refused"))
        }
    }
    for limit in [0, 1, 2, 3, 100] {
        for label in ["continuation", "terminal step", "effect error"] {
            assert_eq!(
                encode_json_capped(&(1, Refuses), limit, label),
                frozen_encode_json_capped(&(1, Refuses), limit, label)
            );
            assert_eq!(
                validate_json_size_capped(&(1, Refuses), limit, label),
                frozen_validate_json_size_capped(&(1, Refuses), limit, label)
            );
        }
    }
    let error = encode_json_capped(&Refuses, 0, "effect error").unwrap_err();
    assert_eq!(error.code, "LSV3001");
    assert_eq!(
        error.message,
        "failed to encode effect error: native codec refused"
    );
}

#[test]
fn caught_write_errors_can_no_longer_escape_as_successful_partial_json() {
    struct Swallows;
    impl Serialize for Swallows {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq;
            let mut sequence = serializer.serialize_seq(None)?;
            let _ = sequence.serialize_element(&"x".repeat(100));
            sequence.end()
        }
    }
    // The old writer returned a malformed prefix when the serializer caught IO errors.
    assert!(frozen_encode_json_capped(&Swallows, 10, "terminal step").is_ok());
    assert!(frozen_validate_json_size_capped(&Swallows, 10, "terminal step").is_ok());
    let expected = Fault {
        code: "LSV3002".into(),
        message: "terminal step exceeds 10 bytes".into(),
    };
    assert_eq!(
        encode_json_capped(&Swallows, 10, "terminal step"),
        Err(expected.clone())
    );
    assert_eq!(
        validate_json_size_capped(&Swallows, 10, "terminal step"),
        Err(expected)
    );
}
