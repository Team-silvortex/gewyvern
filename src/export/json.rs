use std::collections::BTreeMap;

use super::ExportError;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
            Self::String(value) => format!("\"{}\"", escape_json(value)),
            Self::Array(items) => {
                let inner = items
                    .iter()
                    .map(JsonValue::render)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{inner}]")
            }
            Self::Object(map) => {
                let inner = map
                    .iter()
                    .map(|(key, value)| format!("\"{}\":{}", escape_json(key), value.render()))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{inner}}}")
            }
        }
    }

    pub(crate) fn as_str(&self) -> Result<&str, ExportError> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected string".into())),
        }
    }

    pub(crate) fn as_i64(&self) -> Result<i64, ExportError> {
        match self {
            Self::Number(value) => Ok(*value),
            _ => Err(ExportError::InvalidShape("expected number".into())),
        }
    }

    pub(crate) fn as_bool(&self) -> Result<bool, ExportError> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(ExportError::InvalidShape("expected bool".into())),
        }
    }

    pub(crate) fn as_array(&self) -> Result<&[JsonValue], ExportError> {
        match self {
            Self::Array(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected array".into())),
        }
    }

    pub(crate) fn as_object(&self) -> Result<&BTreeMap<String, JsonValue>, ExportError> {
        match self {
            Self::Object(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected object".into())),
        }
    }

    pub(crate) fn into_object(self) -> Result<BTreeMap<String, JsonValue>, ExportError> {
        match self {
            Self::Object(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected object".into())),
        }
    }
}

pub(crate) struct JsonParser<'a> {
    input: &'a str,
}

impl<'a> JsonParser<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input }
    }

    pub(crate) fn parse(self) -> Result<JsonValue, ExportError> {
        let value = serde_json::from_str(self.input)
            .map_err(|error| ExportError::InvalidJson(error.to_string()))?;
        convert_json_value(value)
    }
}

fn convert_json_value(value: serde_json::Value) -> Result<JsonValue, ExportError> {
    match value {
        serde_json::Value::Null => Ok(JsonValue::Null),
        serde_json::Value::Bool(value) => Ok(JsonValue::Bool(value)),
        serde_json::Value::Number(value) => {
            value.as_i64().map(JsonValue::Number).ok_or_else(|| {
                ExportError::InvalidJson("number must fit signed 64-bit integer".into())
            })
        }
        serde_json::Value::String(value) => Ok(JsonValue::String(value)),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(convert_json_value)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        serde_json::Value::Object(values) => values
            .into_iter()
            .map(|(key, value)| convert_json_value(value).map(|value| (key, value)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(JsonValue::Object),
    }
}

pub(crate) fn escape_json(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            character if character.is_control() => {
                let code = character as usize;
                escaped.push_str("\\u");
                escaped.push(char::from(HEX[(code >> 12) & 0x0f]));
                escaped.push(char::from(HEX[(code >> 8) & 0x0f]));
                escaped.push(char::from(HEX[(code >> 4) & 0x0f]));
                escaped.push(char::from(HEX[code & 0x0f]));
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{JsonParser, JsonValue};

    #[test]
    fn json_round_trip_preserves_unicode_and_all_control_characters() {
        let value = JsonValue::Object(BTreeMap::from([(
            "键\0".to_string(),
            JsonValue::String("雪😀\0\u{1}\u{8}\u{c}\n\r\t\\\"".to_string()),
        )]));

        let rendered = value.render();

        assert!(serde_json::from_str::<serde_json::Value>(&rendered).is_ok());
        assert_eq!(JsonParser::new(&rendered).parse().unwrap(), value);
    }

    #[test]
    fn parser_accepts_standard_escapes_and_rejects_nonstandard_json() {
        assert_eq!(
            JsonParser::new(r#""\b\f\u96ea\ud83d\ude00""#)
                .parse()
                .unwrap(),
            JsonValue::String("\u{8}\u{c}雪😀".to_string())
        );
        assert!(JsonParser::new(r#"{"value":[1,]}"#).parse().is_err());
        assert!(JsonParser::new("01").parse().is_err());

        let deeply_nested = format!("{}0{}", "[".repeat(256), "]".repeat(256));
        assert!(JsonParser::new(&deeply_nested).parse().is_err());
    }
}
