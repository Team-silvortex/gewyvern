use serde::{Deserialize, Serialize};

use crate::StringListBuilder;

pub const MAX_SCALAR_STRING_BYTES: usize = 4_096;
pub const MAX_STRING_LIST_ITEMS: usize = 64;

/// Closed data types, independent of expressions, host operations and authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarType {
    Integer,
    Boolean,
    String,
    None,
    OptionalString,
    StringList,
}

/// Ordered text with bounded wire decoding; direct construction remains unchecked.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct StringListValue(pub Vec<String>);

impl StringListValue {
    pub fn is_bounded(&self) -> bool {
        self.0.len() <= MAX_STRING_LIST_ITEMS
            && self
                .0
                .iter()
                .try_fold(MAX_SCALAR_STRING_BYTES, |remaining, item| {
                    remaining.checked_sub(item.len())
                })
                .is_some()
    }
}

impl<'de> Deserialize<'de> for StringListValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TextSeed(usize);
        impl<'de> serde::de::DeserializeSeed<'de> for TextSeed {
            type Value = String;
            fn deserialize<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<String, D::Error> {
                struct TextVisitor(usize);
                impl serde::de::Visitor<'_> for TextVisitor {
                    type Value = String;
                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("a bounded string list entry")
                    }
                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<String, E> {
                        if value.len() > self.0 {
                            return Err(E::custom("string list exceeds 4096 bytes"));
                        }
                        Ok(value.to_owned())
                    }
                    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<String, E> {
                        if value.len() > self.0 {
                            return Err(E::custom("string list exceeds 4096 bytes"));
                        }
                        Ok(value)
                    }
                }
                deserializer.deserialize_string(TextVisitor(self.0))
            }
        }
        struct ListVisitor;
        impl<'de> serde::de::Visitor<'de> for ListVisitor {
            type Value = StringListValue;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("at most 64 strings totaling at most 4096 bytes")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                // Never reserve from an untrusted length hint or collect an unbounded payload.
                let mut items = StringListBuilder::new();
                while let Some(item) =
                    sequence.next_element_seed(TextSeed(items.remaining_bytes()))?
                {
                    // The seed checks bytes/types first, retaining legacy error precedence.
                    items
                        .try_push(item)
                        .map_err(|_| serde::de::Error::custom("string list exceeds 64 entries"))?;
                }
                Ok(items.finish())
            }
        }
        deserializer.deserialize_seq(ListVisitor)
    }
}

/// Absent and empty text are distinct; missing tagged payload is not absence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct OptionalStringValue(pub Option<String>);

impl<'de> Deserialize<'de> for OptionalStringValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = OptionalStringValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an explicit null or bounded string")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(OptionalStringValue(None))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.len() > MAX_SCALAR_STRING_BYTES {
                    return Err(E::custom("optional string exceeds 4096 bytes"));
                }
                self.visit_string(value.to_owned())
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                if value.len() > MAX_SCALAR_STRING_BYTES {
                    return Err(E::custom("optional string exceeds 4096 bytes"));
                }
                Ok(OptionalStringValue(Some(value)))
            }
        }
        // deserialize_any rejects a missing enum payload instead of treating it as null.
        deserializer.deserialize_any(Visitor)
    }
}

/// Language data, not a host handle, capability, expression or execution grant.
///
/// The legacy plain-string decoder and public Rust constructors are unchecked:
/// embedding ingress must call `is_bounded()` before using arbitrary values.
/// Optional/list wire decoding has its original incremental bounds. A serde
/// decoder may allocate input before these visitors run; this is not a sandbox.
///
/// ```
/// use leselang_runtime_core::{ScalarType, ScalarValue, StringListValue};
/// let value = ScalarValue::StringList(StringListValue(vec!["ready".into(), "".into()]));
/// assert_eq!(value.scalar_type(), ScalarType::StringList);
/// assert!(value.is_bounded());
/// assert_eq!(value.text(), None);
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ScalarValue {
    Integer(u64),
    Boolean(bool),
    String(String),
    None,
    OptionalString(OptionalStringValue),
    StringList(StringListValue),
}

impl ScalarValue {
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Integer(_) => ScalarType::Integer,
            Self::Boolean(_) => ScalarType::Boolean,
            Self::String(_) => ScalarType::String,
            Self::None => ScalarType::None,
            Self::OptionalString(_) => ScalarType::OptionalString,
            Self::StringList(_) => ScalarType::StringList,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::OptionalString(value) => value.0.as_deref(),
            _ => None,
        }
    }

    pub fn is_bounded(&self) -> bool {
        match self {
            Self::StringList(value) => value.is_bounded(),
            _ => self
                .text()
                .is_none_or(|value| value.len() <= MAX_SCALAR_STRING_BYTES),
        }
    }
}
