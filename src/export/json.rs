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
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    pub(crate) fn parse(mut self) -> Result<JsonValue, ExportError> {
        let value = self.parse_value()?;
        self.skip_ws();
        if self.pos != self.input.len() {
            return Err(ExportError::InvalidJson("trailing data".into()));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, ExportError> {
        self.skip_ws();
        let ch = self
            .peek()
            .ok_or_else(|| ExportError::InvalidJson("unexpected eof".into()))?;
        match ch {
            b'n' => {
                self.expect_bytes(b"null")?;
                Ok(JsonValue::Null)
            }
            b't' => {
                self.expect_bytes(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            b'f' => {
                self.expect_bytes(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            b'"' => Ok(JsonValue::String(self.parse_string()?)),
            b'[' => self.parse_array(),
            b'{' => self.parse_object(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(ExportError::InvalidJson("invalid token".into())),
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, ExportError> {
        self.consume(b'[')?;
        let mut values = Vec::new();
        loop {
            self.skip_ws();
            if self.try_consume(b']') {
                break;
            }
            values.push(self.parse_value()?);
            self.skip_ws();
            if self.try_consume(b']') {
                break;
            }
            self.consume(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_object(&mut self) -> Result<JsonValue, ExportError> {
        self.consume(b'{')?;
        let mut map = BTreeMap::new();
        loop {
            self.skip_ws();
            if self.try_consume(b'}') {
                break;
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.consume(b':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            if self.try_consume(b'}') {
                break;
            }
            self.consume(b',')?;
        }
        Ok(JsonValue::Object(map))
    }

    fn parse_string(&mut self) -> Result<String, ExportError> {
        self.consume(b'"')?;
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            self.pos += 1;
            match ch {
                b'"' => return Ok(value),
                b'\\' => {
                    let escaped = self
                        .peek()
                        .ok_or_else(|| ExportError::InvalidJson("bad escape".into()))?;
                    self.pos += 1;
                    match escaped {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        _ => return Err(ExportError::InvalidJson("unsupported escape".into())),
                    }
                }
                _ => value.push(ch as char),
            }
        }
        Err(ExportError::InvalidJson("unterminated string".into()))
    }

    fn parse_number(&mut self) -> Result<JsonValue, ExportError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        let raw = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| ExportError::InvalidJson("bad number".into()))?;
        let value = raw
            .parse::<i64>()
            .map_err(|_| ExportError::InvalidJson("bad number".into()))?;
        Ok(JsonValue::Number(value))
    }

    fn expect_bytes(&mut self, bytes: &[u8]) -> Result<(), ExportError> {
        if self.input.get(self.pos..self.pos + bytes.len()) == Some(bytes) {
            self.pos += bytes.len();
            Ok(())
        } else {
            Err(ExportError::InvalidJson("unexpected token".into()))
        }
    }

    fn consume(&mut self, ch: u8) -> Result<(), ExportError> {
        if self.try_consume(ch) {
            Ok(())
        } else {
            Err(ExportError::InvalidJson("unexpected token".into()))
        }
    }

    fn try_consume(&mut self, ch: u8) -> bool {
        if self.peek() == Some(ch) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }
}

pub(crate) fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
