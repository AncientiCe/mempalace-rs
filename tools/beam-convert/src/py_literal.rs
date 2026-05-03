//! Minimal parser for the subset of Python literals that Hugging Face's
//! BEAM parquet stores in the `probing_questions` column.
//!
//! Supports: `dict`, `list`, `tuple` (treated as list), single/double-quoted
//! strings (with the common `\\`, `\'`, `\"`, `\n`, `\t`, `\r`, `\xNN`,
//! `\uNNNN` escapes), integers, floats, `None`, `True`, and `False`.
//!
//! Anything else triggers a parse error rather than panicking.

use anyhow::{anyhow, bail, Result};
use serde_json::{Map, Number, Value};

pub fn parse(input: &str) -> Result<Value> {
    let mut parser = Parser::new(input);
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        bail!(
            "trailing input at byte offset {} (next: {:?})",
            parser.pos,
            parser.peek_char()
        );
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_whitespace();
        match self.peek_byte() {
            Some(b'{') => self.parse_dict(),
            Some(b'[') => self.parse_seq(b']'),
            Some(b'(') => self.parse_seq(b')'),
            Some(b'\'') | Some(b'"') => self.parse_string().map(Value::String),
            Some(b'-') | Some(b'+') | Some(b'0'..=b'9') => self.parse_number(),
            Some(b'N') => self.parse_keyword("None", Value::Null),
            Some(b'T') => self.parse_keyword("True", Value::Bool(true)),
            Some(b'F') => self.parse_keyword("False", Value::Bool(false)),
            Some(b) => Err(anyhow!(
                "unexpected byte {:?} at offset {}",
                b as char,
                self.pos
            )),
            None => Err(anyhow!("unexpected end of input")),
        }
    }

    fn parse_dict(&mut self) -> Result<Value> {
        self.expect(b'{')?;
        let mut map = Map::new();
        self.skip_whitespace();
        if self.consume_if(b'}') {
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_whitespace();
            let key = match self.peek_byte() {
                Some(b'\'') | Some(b'"') => self.parse_string()?,
                Some(b'0'..=b'9') | Some(b'-') | Some(b'+') => {
                    let number = self.parse_number()?;
                    number_to_string(&number)
                }
                _ => bail!("expected dict key at offset {}", self.pos),
            };
            self.skip_whitespace();
            self.expect(b':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_whitespace();
            match self.peek_byte() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_whitespace();
                    if self.consume_if(b'}') {
                        break;
                    }
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                Some(b) => bail!(
                    "expected ',' or '}}' at offset {} (saw {:?})",
                    self.pos,
                    b as char
                ),
                None => bail!("unterminated dict starting"),
            }
        }
        Ok(Value::Object(map))
    }

    fn parse_seq(&mut self, close: u8) -> Result<Value> {
        let open = if close == b']' { b'[' } else { b'(' };
        self.expect(open)?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.consume_if(close) {
            return Ok(Value::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek_byte() {
                Some(b) if b == b',' => {
                    self.pos += 1;
                    self.skip_whitespace();
                    if self.consume_if(close) {
                        break;
                    }
                }
                Some(b) if b == close => {
                    self.pos += 1;
                    break;
                }
                Some(b) => bail!(
                    "expected ',' or {:?} at offset {} (saw {:?})",
                    close as char,
                    self.pos,
                    b as char
                ),
                None => bail!("unterminated sequence"),
            }
        }
        Ok(Value::Array(items))
    }

    fn parse_string(&mut self) -> Result<String> {
        let quote = self
            .peek_byte()
            .ok_or_else(|| anyhow!("expected string at offset {}", self.pos))?;
        if quote != b'\'' && quote != b'"' {
            bail!("expected string quote at offset {}", self.pos);
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let byte = match self.peek_byte() {
                Some(b) => b,
                None => bail!("unterminated string"),
            };
            if byte == quote {
                self.pos += 1;
                return Ok(out);
            }
            if byte == b'\\' {
                self.pos += 1;
                let escape = self
                    .peek_byte()
                    .ok_or_else(|| anyhow!("dangling escape at offset {}", self.pos))?;
                self.pos += 1;
                match escape {
                    b'\\' => out.push('\\'),
                    b'\'' => out.push('\''),
                    b'"' => out.push('"'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'b' => out.push('\u{08}'),
                    b'f' => out.push('\u{0C}'),
                    b'0' => out.push('\0'),
                    b'a' => out.push('\u{07}'),
                    b'v' => out.push('\u{0B}'),
                    b'/' => out.push('/'),
                    b'x' => {
                        let value = self.parse_hex(2)?;
                        let ch = char::from_u32(value)
                            .ok_or_else(|| anyhow!("invalid \\x escape"))?;
                        out.push(ch);
                    }
                    b'u' => {
                        let value = self.parse_hex(4)?;
                        let ch = char::from_u32(value)
                            .ok_or_else(|| anyhow!("invalid \\u escape"))?;
                        out.push(ch);
                    }
                    b'U' => {
                        let value = self.parse_hex(8)?;
                        let ch = char::from_u32(value)
                            .ok_or_else(|| anyhow!("invalid \\U escape"))?;
                        out.push(ch);
                    }
                    b'\n' => {}
                    other => bail!("unsupported escape \\{}", other as char),
                }
            } else {
                let ch_start = self.pos;
                let ch = self.peek_char().ok_or_else(|| anyhow!("invalid utf-8"))?;
                self.pos = ch_start + ch.len_utf8();
                out.push(ch);
            }
        }
    }

    fn parse_hex(&mut self, len: usize) -> Result<u32> {
        if self.pos + len > self.bytes.len() {
            bail!("hex escape ran off end at offset {}", self.pos);
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        let text = std::str::from_utf8(slice)
            .map_err(|_| anyhow!("non-utf8 hex escape at offset {}", self.pos))?;
        let value =
            u32::from_str_radix(text, 16).map_err(|_| anyhow!("invalid hex escape {text:?}"))?;
        self.pos += len;
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value> {
        let start = self.pos;
        if matches!(self.peek_byte(), Some(b'+') | Some(b'-')) {
            self.pos += 1;
        }
        let mut saw_digit = false;
        while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
            self.pos += 1;
            saw_digit = true;
        }
        let mut is_float = false;
        if let Some(b'.') = self.peek_byte() {
            is_float = true;
            self.pos += 1;
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.pos += 1;
                saw_digit = true;
            }
        }
        if matches!(self.peek_byte(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek_byte(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if !saw_digit {
            bail!("expected number at offset {}", start);
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| anyhow!("non-utf8 number"))?;
        if is_float {
            let value: f64 = text
                .parse()
                .map_err(|_| anyhow!("invalid float {text:?}"))?;
            let number = Number::from_f64(value)
                .ok_or_else(|| anyhow!("non-finite float {text:?}"))?;
            Ok(Value::Number(number))
        } else {
            let value: i64 = text
                .parse()
                .map_err(|_| anyhow!("invalid integer {text:?}"))?;
            Ok(Value::Number(Number::from(value)))
        }
    }

    fn parse_keyword(&mut self, keyword: &str, value: Value) -> Result<Value> {
        let bytes = keyword.as_bytes();
        if self.pos + bytes.len() > self.bytes.len()
            || &self.bytes[self.pos..self.pos + bytes.len()] != bytes
        {
            bail!("expected keyword {keyword} at offset {}", self.pos);
        }
        self.pos += bytes.len();
        Ok(value)
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        match self.peek_byte() {
            Some(b) if b == byte => {
                self.pos += 1;
                Ok(())
            }
            Some(b) => Err(anyhow!(
                "expected {:?} at offset {} (saw {:?})",
                byte as char,
                self.pos,
                b as char
            )),
            None => Err(anyhow!("expected {:?}, found end of input", byte as char)),
        }
    }

    fn consume_if(&mut self, byte: u8) -> bool {
        if self.peek_byte() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek_byte() {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_char(&self) -> Option<char> {
        std::str::from_utf8(&self.bytes[self.pos..])
            .ok()
            .and_then(|s| s.chars().next())
    }
}

fn number_to_string(value: &Value) -> String {
    match value {
        Value::Number(number) => number.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_json::json;

    #[test]
    fn parses_scalars() {
        assert_eq!(parse("None").unwrap(), serde_json::Value::Null);
        assert_eq!(parse("True").unwrap(), json!(true));
        assert_eq!(parse("False").unwrap(), json!(false));
        assert_eq!(parse("42").unwrap(), json!(42));
        assert_eq!(parse("-7").unwrap(), json!(-7));
        assert_eq!(parse("'hi'").unwrap(), json!("hi"));
    }

    #[test]
    fn parses_nested_dict_with_python_quirks() {
        let input = "{'a': [1, 2], 'b': {'c': None, 'd': True}, 'e': 'it\\'s'}";
        assert_eq!(
            parse(input).unwrap(),
            json!({
                "a": [1, 2],
                "b": {"c": null, "d": true},
                "e": "it's",
            })
        );
    }

    #[test]
    fn parses_beam_like_blob() {
        let input = "{'abstention': [{'question': 'q?', 'difficulty': 'medium'}], \
            'event_ordering': [{'question': 'q2', 'source_chat_ids': [4, 60, 116]}]}";
        let value = parse(input).unwrap();
        assert_eq!(value["abstention"][0]["question"], "q?");
        assert_eq!(value["event_ordering"][0]["source_chat_ids"], json!([4, 60, 116]));
    }

    #[test]
    fn rejects_trailing_garbage() {
        assert!(parse("[1, 2] junk").is_err());
    }
}
