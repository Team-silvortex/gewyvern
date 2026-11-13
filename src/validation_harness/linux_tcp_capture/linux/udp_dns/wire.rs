//! Bounded DNS fixture codec. This is not a recursive resolver or a DNSSEC validator.
use serde::Serialize;

use super::ValidationError;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct Reply {
    pub id: u16,
    pub question: String,
    pub rcode: u8,
    pub truncated: bool,
    pub addresses: Vec<[u8; 4]>,
}

fn invalid() -> ValidationError {
    ValidationError::new("invalid or unsupported bounded DNS message")
}

fn word(bytes: &[u8], offset: usize) -> Result<u16, ValidationError> {
    let value = bytes.get(offset..offset + 2).ok_or_else(invalid)?;
    Ok(u16::from_be_bytes([value[0], value[1]]))
}

fn name(bytes: &[u8], cursor: &mut usize) -> Result<String, ValidationError> {
    let mut at = *cursor;
    let mut end = None;
    let mut labels = Vec::new();
    let mut expanded = 1usize;
    for _ in 0..128 {
        let length = *bytes.get(at).ok_or_else(invalid)?;
        if length & 0xc0 == 0xc0 {
            let pointer = usize::from(word(bytes, at)? & 0x3fff);
            if pointer < 12 || pointer >= at {
                return Err(invalid());
            }
            end.get_or_insert(at + 2);
            at = pointer;
        } else if length == 0 {
            *cursor = end.unwrap_or(at + 1);
            return Ok(format!("{}.", labels.join(".")));
        } else {
            if length > 63 {
                return Err(invalid());
            }
            let length = usize::from(length);
            let label = bytes.get(at + 1..at + 1 + length).ok_or_else(invalid)?;
            if !label
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
            {
                return Err(invalid());
            }
            expanded += length + 1;
            if expanded > 255 {
                return Err(invalid());
            }
            labels.push(String::from_utf8_lossy(label).to_ascii_lowercase());
            at += length + 1;
        }
    }
    Err(invalid())
}

pub(super) fn query(question: &str, id: u16) -> Result<Vec<u8>, ValidationError> {
    let mut bytes = vec![0; 12];
    bytes[..2].copy_from_slice(&id.to_be_bytes());
    bytes[2] = 1;
    bytes[5] = 1;
    for label in question.trim_end_matches('.').split('.') {
        if label.is_empty()
            || label.len() > 63
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(invalid());
        }
        bytes.push(label.len() as u8);
        bytes.extend_from_slice(label.as_bytes());
    }
    if bytes.len() - 12 + 1 > 255 {
        return Err(invalid());
    }
    bytes.extend_from_slice(&[0, 0, 1, 0, 1]);
    Ok(bytes)
}

pub(super) fn parse_reply(bytes: &[u8], query: &[u8]) -> Result<Reply, ValidationError> {
    if bytes.len() < 12
        || bytes.len() > 512
        || query.len() < 12
        || word(bytes, 0)? != word(query, 0)?
        || word(bytes, 2)? & 0xf800 != 0x8000
        || word(bytes, 4)? != 1
        || word(bytes, 8)? != 0
        || word(bytes, 10)? != 0
    {
        return Err(invalid());
    }
    let mut cursor = 12;
    let question = name(bytes, &mut cursor)?;
    let mut query_cursor = 12;
    if question != name(query, &mut query_cursor)?
        || word(bytes, cursor)? != 1
        || word(bytes, cursor + 2)? != 1
    {
        return Err(invalid());
    }
    cursor += 4;
    let count = word(bytes, 6)?;
    if count > 8 {
        return Err(invalid());
    }
    let mut addresses = Vec::new();
    for _ in 0..count {
        if name(bytes, &mut cursor)? != question
            || word(bytes, cursor)? != 1
            || word(bytes, cursor + 2)? != 1
            || word(bytes, cursor + 8)? != 4
        {
            return Err(invalid());
        }
        cursor += 10;
        addresses.push(
            bytes
                .get(cursor..cursor + 4)
                .ok_or_else(invalid)?
                .try_into()
                .map_err(|_| invalid())?,
        );
        cursor += 4;
    }
    if cursor != bytes.len() {
        return Err(invalid());
    }
    Ok(Reply {
        id: word(bytes, 0)?,
        question,
        rcode: bytes[3] & 15,
        truncated: bytes[2] & 2 != 0,
        addresses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dns_requires_complete_framing_and_question_identity() {
        let query = query("ok.test.", 7).unwrap();
        let mut response = query.clone();
        response[2] |= 0x80;
        response[7] = 1;
        response.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 1, 0, 4, 192, 0, 2, 7]);
        assert_eq!(
            parse_reply(&response, &query).unwrap().addresses,
            vec![[192, 0, 2, 7]]
        );
        for length in [0, 4, 11, response.len() - 1] {
            assert!(parse_reply(&response[..length], &query).is_err());
        }
        for (at, value) in [(1, 8), (2, 1), (5, 2), (9, 1), (11, 1), (13, b'x')] {
            let mut invalid = response.clone();
            invalid[at] = value;
            assert!(parse_reply(&invalid, &query).is_err());
        }
        let mut invalid = response.clone();
        invalid[query.len() + 1] = query.len() as u8;
        assert!(parse_reply(&invalid, &query).is_err());
        response.push(0);
        assert!(parse_reply(&response, &query).is_err());
    }
    #[test]
    fn tc_is_not_a_complete_resolution_and_same_id_is_not_question_identity() {
        let request = query("cut.test.", 7).unwrap();
        let mut response = request.clone();
        response[2] |= 0x82;
        let reply = parse_reply(&response, &request).unwrap();
        assert!(reply.truncated);
        assert!(reply.addresses.is_empty());
        assert!(parse_reply(&response, &query("other.test.", 7).unwrap()).is_err());
        assert!(query(&format!("{}.test", "a".repeat(64)), 7).is_err());
    }
}
