use serde::{Deserialize, Serialize};

pub const MAX_SOURCE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Fn,
    Ident,
    String,
    None,
    LeftParen,
    RightParen,
    Dot,
    Colon,
    Comma,
    Equal,
    Whitespace,
    Comment,
    Unknown,
    Eof,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Span,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SyntaxTree {
    source: String,
    pub tokens: Vec<Token>,
    pub function: Option<Function>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Function {
    pub name: String,
    pub body: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expression {
    Call {
        callee: String,
        arguments: Vec<NamedArgument>,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    None {
        span: Span,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NamedArgument {
    pub name: String,
    pub value: Expression,
    pub span: Span,
}

impl SyntaxTree {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn token_text(&self, token: &Token) -> &str {
        &self.source[token.span.start..token.span.end]
    }

    pub fn reconstruct(&self) -> String {
        self.tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Eof)
            .map(|token| self.token_text(token))
            .collect()
    }
}

pub fn parse(source: &str) -> SyntaxTree {
    if source.len() > MAX_SOURCE_BYTES {
        return SyntaxTree {
            source: source.to_string(),
            tokens: vec![Token {
                kind: TokenKind::Eof,
                span: Span { start: 0, end: 0 },
            }],
            function: None,
            diagnostics: vec![Diagnostic {
                code: "LSE0001".to_string(),
                message: format!("source exceeds {MAX_SOURCE_BYTES} bytes"),
                span: Span {
                    start: 0,
                    end: source.len(),
                },
            }],
        };
    }

    let (tokens, mut diagnostics) = lex(source);
    let mut parser = Parser::new(source, &tokens);
    let function = parser.parse_function();
    diagnostics.extend(parser.diagnostics);
    SyntaxTree {
        source: source.to_string(),
        tokens,
        function,
        diagnostics,
    }
}

fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let start = cursor;
        let kind = match bytes[cursor] {
            byte if byte.is_ascii_whitespace() => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                TokenKind::Whitespace
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'/') => {
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
                TokenKind::Comment
            }
            b'"' => {
                cursor += 1;
                let mut terminated = false;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'\\' if cursor + 1 < bytes.len() => cursor += 2,
                        b'"' => {
                            cursor += 1;
                            terminated = true;
                            break;
                        }
                        _ => cursor += char_len_at(source, cursor),
                    }
                }
                if !terminated {
                    diagnostics.push(Diagnostic {
                        code: "LSE0003".to_string(),
                        message: "unterminated string literal".to_string(),
                        span: Span { start, end: cursor },
                    });
                }
                TokenKind::String
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                match &source[start..cursor] {
                    "fn" => TokenKind::Fn,
                    "none" => TokenKind::None,
                    _ => TokenKind::Ident,
                }
            }
            b'(' => single(&mut cursor, TokenKind::LeftParen),
            b')' => single(&mut cursor, TokenKind::RightParen),
            b'.' => single(&mut cursor, TokenKind::Dot),
            b':' => single(&mut cursor, TokenKind::Colon),
            b',' => single(&mut cursor, TokenKind::Comma),
            b'=' => single(&mut cursor, TokenKind::Equal),
            _ => {
                cursor += char_len_at(source, cursor);
                diagnostics.push(Diagnostic {
                    code: "LSE0002".to_string(),
                    message: "unknown token".to_string(),
                    span: Span { start, end: cursor },
                });
                TokenKind::Unknown
            }
        };
        tokens.push(Token {
            kind,
            span: Span { start, end: cursor },
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span {
            start: source.len(),
            end: source.len(),
        },
    });
    (tokens, diagnostics)
}

fn single(cursor: &mut usize, kind: TokenKind) -> TokenKind {
    *cursor += 1;
    kind
}

fn char_len_at(source: &str, cursor: usize) -> usize {
    source[cursor..].chars().next().map_or(1, char::len_utf8)
}

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse_function(&mut self) -> Option<Function> {
        let start = self
            .expect(TokenKind::Fn, "LSE1001", "expected 'fn'")?
            .span
            .start;
        let name = self.expect_ident("LSE1002", "expected function name")?;
        self.expect(TokenKind::LeftParen, "LSE1003", "expected '('")?;
        self.expect(TokenKind::RightParen, "LSE1004", "expected ')'")?;
        self.expect(TokenKind::Equal, "LSE1005", "expected '='")?;
        let body = self.parse_call()?;
        let end = expression_span(&body).end;
        if self.peek().kind != TokenKind::Eof {
            let token = self.peek().clone();
            self.error(
                "LSE1006",
                "unexpected token after function body",
                token.span,
            );
        }
        Some(Function {
            name: name.0,
            body,
            span: Span { start, end },
        })
    }

    fn parse_call(&mut self) -> Option<Expression> {
        let first = self.expect_ident("LSE1101", "expected effect namespace")?;
        self.expect(TokenKind::Dot, "LSE1102", "expected '.'")?;
        let second = self.expect_ident("LSE1103", "expected effect operation")?;
        self.expect(TokenKind::LeftParen, "LSE1104", "expected '('")?;
        let mut arguments = Vec::new();
        while self.peek().kind != TokenKind::RightParen && self.peek().kind != TokenKind::Eof {
            let (name, name_span) = self.expect_ident("LSE1105", "expected argument name")?;
            self.expect(TokenKind::Colon, "LSE1106", "expected ':'")?;
            let value = self.parse_value()?;
            let span = Span {
                start: name_span.start,
                end: expression_span(&value).end,
            };
            arguments.push(NamedArgument { name, value, span });
            if self.peek().kind != TokenKind::Comma {
                break;
            }
            self.bump();
        }
        let close = self.expect(TokenKind::RightParen, "LSE1107", "expected ')'")?;
        Some(Expression::Call {
            callee: format!("{}.{}", first.0, second.0),
            arguments,
            span: Span {
                start: first.1.start,
                end: close.span.end,
            },
        })
    }

    fn parse_value(&mut self) -> Option<Expression> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::String => {
                self.bump();
                match decode_string(&self.source[token.span.start..token.span.end]) {
                    Ok(value) => Some(Expression::String {
                        value,
                        span: token.span,
                    }),
                    Err(message) => {
                        self.error("LSE1108", message, token.span);
                        None
                    }
                }
            }
            TokenKind::None => {
                self.bump();
                Some(Expression::None { span: token.span })
            }
            _ => {
                self.error("LSE1109", "expected string or 'none'", token.span);
                None
            }
        }
    }

    fn expect_ident(&mut self, code: &str, message: &str) -> Option<(String, Span)> {
        let token = self.expect(TokenKind::Ident, code, message)?;
        Some((
            self.source[token.span.start..token.span.end].to_string(),
            token.span,
        ))
    }

    fn expect(&mut self, kind: TokenKind, code: &str, message: &str) -> Option<Token> {
        let token = self.peek().clone();
        if token.kind == kind {
            self.bump();
            Some(token)
        } else {
            self.error(code, message, token.span);
            None
        }
    }

    fn peek(&mut self) -> &Token {
        while matches!(
            self.tokens[self.cursor].kind,
            TokenKind::Whitespace | TokenKind::Comment
        ) {
            self.cursor += 1;
        }
        &self.tokens[self.cursor]
    }

    fn bump(&mut self) {
        self.cursor += 1;
    }

    fn error(&mut self, code: &str, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic {
            code: code.to_string(),
            message: message.into(),
            span,
        });
    }
}

fn expression_span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::String { span, .. }
        | Expression::None { span } => *span,
    }
}

fn decode_string(source: &str) -> Result<String, &'static str> {
    if source.len() < 2 || !source.starts_with('"') || !source.ends_with('"') {
        return Err("unterminated string literal");
    }
    let mut output = String::new();
    let mut chars = source[1..source.len() - 1].chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = chars.next().ok_or("incomplete string escape")?;
        output.push(match escaped {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            _ => return Err("unsupported string escape"),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_retains_trivia_and_reconstructs_source() {
        let source = "// fleet\nfn main() = runtime.list(environment: \"prod\", role: none)\n";
        let tree = parse(source);
        assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
        assert_eq!(tree.reconstruct(), source);
        assert!(
            tree.tokens
                .iter()
                .any(|token| token.kind == TokenKind::Comment)
        );
        assert_eq!(tree.function.unwrap().name, "main");
    }

    #[test]
    fn parser_decodes_strings_and_named_arguments() {
        let tree = parse("fn main() = runtime.list(environment: \"pro\\nd\", cluster: none)");
        let Expression::Call { arguments, .. } = tree.function.unwrap().body else {
            panic!("expected call");
        };
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[0].name, "environment");
        assert!(matches!(
            &arguments[0].value,
            Expression::String { value, .. } if value == "pro\nd"
        ));
    }

    #[test]
    fn malformed_source_reports_stable_spanned_diagnostics() {
        let tree = parse("fn main( = runtime.list(environment: @)");
        assert!(tree.function.is_none());
        assert!(tree.diagnostics.iter().any(|item| item.code == "LSE0002"));
        assert!(tree.diagnostics.iter().any(|item| item.code == "LSE1004"));
        assert!(
            tree.diagnostics
                .iter()
                .all(|item| item.span.end <= tree.source().len())
        );
    }

    #[test]
    fn oversized_source_is_rejected_before_lexing() {
        let source = "x".repeat(MAX_SOURCE_BYTES + 1);
        let tree = parse(&source);
        assert_eq!(tree.diagnostics[0].code, "LSE0001");
        assert!(tree.function.is_none());
    }
}
