use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_CALL_DEPTH: usize = 16;

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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

    pub fn token_text(&self, token: &Token) -> Option<&str> {
        self.source.get(token.span.start..token.span.end)
    }

    pub fn reconstruct(&self) -> Option<String> {
        self.tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Eof)
            .map(|token| self.token_text(token))
            .collect::<Option<String>>()
    }
}

#[derive(Deserialize)]
struct SyntaxTreeWire {
    source: String,
    tokens: Vec<Token>,
    function: Option<Function>,
    diagnostics: Vec<Diagnostic>,
}

impl<'de> Deserialize<'de> for SyntaxTree {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SyntaxTreeWire::deserialize(deserializer)?;
        let tree = Self {
            source: wire.source,
            tokens: wire.tokens,
            function: wire.function,
            diagnostics: wire.diagnostics,
        };
        tree.validate_serialized_shape().map_err(D::Error::custom)?;
        Ok(tree)
    }
}

impl SyntaxTree {
    fn validate_serialized_shape(&self) -> Result<(), &'static str> {
        if self.source.len() > MAX_SOURCE_BYTES {
            let rejected_shape = self.function.is_none()
                && self.tokens.as_slice()
                    == [Token {
                        kind: TokenKind::Eof,
                        span: Span { start: 0, end: 0 },
                    }]
                && self.diagnostics.len() == 1
                && self.diagnostics[0].code == "LSE0001"
                && self.diagnostics[0].span
                    == (Span {
                        start: 0,
                        end: self.source.len(),
                    });
            return rejected_shape
                .then_some(())
                .ok_or("invalid oversized syntax tree rejection shape");
        }
        if self.tokens.is_empty() {
            return Err("invalid syntax tree token count");
        }
        let mut cursor = 0;
        for (index, token) in self.tokens.iter().enumerate() {
            validate_span(&self.source, token.span)?;
            if token.kind == TokenKind::Eof {
                if index + 1 != self.tokens.len()
                    || token.span.start != self.source.len()
                    || token.span.end != self.source.len()
                {
                    return Err("invalid syntax tree EOF token");
                }
            } else {
                if token.span.start != cursor || token.span.end <= token.span.start {
                    return Err("syntax tree tokens do not losslessly cover source");
                }
                cursor = token.span.end;
            }
        }
        if cursor != self.source.len()
            || self
                .tokens
                .last()
                .is_none_or(|token| token.kind != TokenKind::Eof)
        {
            return Err("syntax tree token stream is incomplete");
        }
        for diagnostic in &self.diagnostics {
            validate_span(&self.source, diagnostic.span)?;
        }
        if let Some(function) = &self.function {
            validate_span(&self.source, function.span)?;
            validate_expression_spans(&self.source, &function.body, 0)?;
        }
        Ok(())
    }
}

fn validate_span(source: &str, span: Span) -> Result<(), &'static str> {
    if span.start > span.end
        || span.end > source.len()
        || !source.is_char_boundary(span.start)
        || !source.is_char_boundary(span.end)
    {
        return Err("syntax tree span is outside UTF-8 source boundaries");
    }
    Ok(())
}

fn validate_expression_spans(
    source: &str,
    expression: &Expression,
    depth: usize,
) -> Result<(), &'static str> {
    validate_span(source, expression_span(expression))?;
    if let Expression::Call { arguments, .. } = expression {
        if depth >= MAX_CALL_DEPTH {
            return Err("syntax tree expression depth exceeds limit");
        }
        for argument in arguments {
            validate_span(source, argument.span)?;
            validate_expression_spans(source, &argument.value, depth + 1)?;
        }
    }
    Ok(())
}

pub fn format(tree: &SyntaxTree) -> Result<String, Vec<Diagnostic>> {
    if !tree.diagnostics.is_empty() {
        return Err(tree.diagnostics.clone());
    }
    let Some(function) = &tree.function else {
        return Err(vec![Diagnostic {
            code: "LSE2001".to_string(),
            message: "cannot format a syntax tree without a function".to_string(),
            span: Span { start: 0, end: 0 },
        }]);
    };
    let mut output = format!("fn {}() = ", function.name);
    format_expression(&function.body, 0, &mut output);
    output.push('\n');
    if output.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic {
            code: "LSE2002".to_string(),
            message: format!("formatted source exceeds {MAX_SOURCE_BYTES} bytes"),
            span: function.span,
        }]);
    }
    Ok(output)
}

fn format_expression(expression: &Expression, indent: usize, output: &mut String) {
    match expression {
        Expression::Call {
            callee, arguments, ..
        } if arguments.is_empty() => {
            output.push_str(callee);
            output.push_str("()");
        }
        Expression::Call {
            callee, arguments, ..
        } if expression_is_inline(expression) => {
            output.push_str(callee);
            output.push('(');
            let argument = &arguments[0];
            output.push_str(&argument.name);
            output.push_str(": ");
            format_expression(&argument.value, indent, output);
            output.push(')');
        }
        Expression::Call {
            callee, arguments, ..
        } => {
            output.push_str(callee);
            output.push_str("(\n");
            let child_indent = indent + 2;
            for argument in arguments {
                output.push_str(&" ".repeat(child_indent));
                output.push_str(&argument.name);
                output.push_str(": ");
                format_expression(&argument.value, child_indent, output);
                output.push_str(",\n");
            }
            output.push_str(&" ".repeat(indent));
            output.push(')');
        }
        Expression::String { value, .. } => format_string(value, output),
        Expression::None { .. } => output.push_str("none"),
    }
}

fn expression_is_inline(expression: &Expression) -> bool {
    match expression {
        Expression::Call { arguments, .. } => {
            arguments.len() <= 1
                && arguments
                    .first()
                    .is_none_or(|argument| expression_is_inline(&argument.value))
        }
        Expression::String { .. } | Expression::None { .. } => true,
    }
}

fn format_string(value: &str, output: &mut String) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
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
                        b'\\' if cursor + 1 < bytes.len() => {
                            cursor += 1;
                            cursor += char_len_at(source, cursor);
                        }
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
        let body = self.parse_call(0)?;
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

    fn parse_call(&mut self, depth: usize) -> Option<Expression> {
        if depth >= MAX_CALL_DEPTH {
            let span = self.peek().span;
            self.error("LSE1110", "call nesting exceeds parser limit", span);
            return None;
        }
        let first = self.expect_ident("LSE1101", "expected function or effect namespace")?;
        let callee = if self.peek().kind == TokenKind::Dot {
            self.bump();
            let second = self.expect_ident("LSE1103", "expected effect operation")?;
            format!("{}.{}", first.0, second.0)
        } else {
            first.0.clone()
        };
        self.expect(TokenKind::LeftParen, "LSE1104", "expected '('")?;
        let mut arguments = Vec::new();
        while self.peek().kind != TokenKind::RightParen && self.peek().kind != TokenKind::Eof {
            let (name, name_span) = self.expect_ident("LSE1105", "expected argument name")?;
            self.expect(TokenKind::Colon, "LSE1106", "expected ':'")?;
            let value = self.parse_value(depth)?;
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
            callee,
            arguments,
            span: Span {
                start: first.1.start,
                end: close.span.end,
            },
        })
    }

    fn parse_value(&mut self, call_depth: usize) -> Option<Expression> {
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
            TokenKind::Ident => self.parse_call(call_depth + 1),
            _ => {
                self.error(
                    "LSE1109",
                    "expected string, 'none', or nested call",
                    token.span,
                );
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
    let body = &source[1..source.len() - 1];
    if !body.as_bytes().contains(&b'\\') {
        return Ok(body.to_string());
    }
    let mut output = String::with_capacity(body.len());
    let mut chars = body.chars();
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
        assert_eq!(tree.reconstruct().as_deref(), Some(source));
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
    fn parser_retains_declared_all_branch_order() {
        let source = "fn main() = all(inventory: runtime.list(role: \"edge\"), refresh: runtime.refresh(runtime_id: \"runtime-a\"))";
        let tree = parse(source);
        assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
        assert_eq!(tree.reconstruct().as_deref(), Some(source));
        let Expression::Call {
            callee, arguments, ..
        } = tree.function.unwrap().body
        else {
            panic!("expected all call");
        };
        assert_eq!(callee, "all");
        assert_eq!(
            arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>(),
            ["inventory", "refresh"]
        );
        assert!(matches!(
            arguments[0].value,
            Expression::Call { ref callee, .. } if callee == "runtime.list"
        ));
    }

    #[test]
    fn parser_rejects_excessive_nested_calls_before_stack_growth() {
        let mut expression = "runtime.list()".to_string();
        for _ in 0..MAX_CALL_DEPTH {
            expression = format!("all(left: {expression}, right: runtime.list())");
        }
        let tree = parse(&format!("fn main() = {expression}"));
        assert!(tree.diagnostics.iter().any(|item| item.code == "LSE1110"));
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
    fn unicode_after_escape_reports_a_diagnostic_without_breaking_spans() {
        let source = "fn main() = runtime.list(environment: \"\\🙂\")";
        let tree = parse(source);

        assert_eq!(tree.reconstruct().as_deref(), Some(source));
        assert!(tree.function.is_none());
        assert!(tree.diagnostics.iter().any(|item| item.code == "LSE1108"));
        assert!(tree.diagnostics.iter().all(|item| {
            item.span.start <= item.span.end
                && item.span.end <= source.len()
                && source.is_char_boundary(item.span.start)
                && source.is_char_boundary(item.span.end)
        }));
    }

    #[test]
    fn oversized_source_is_rejected_before_lexing() {
        let source = "x".repeat(MAX_SOURCE_BYTES + 1);
        let tree = parse(&source);
        assert_eq!(tree.diagnostics[0].code, "LSE0001");
        assert!(tree.function.is_none());
    }

    #[test]
    fn formatter_erases_trivia_into_one_canonical_program() {
        let compact =
            parse("fn main()=runtime.list(environment:\"production\",cluster:none,role:\"edge\")");
        let commented = parse(
            "// inventory\nfn main ( ) = runtime.list( environment: \"production\", // local\n cluster: none, role: \"edge\", )",
        );
        let expected = "fn main() = runtime.list(\n  environment: \"production\",\n  cluster: none,\n  role: \"edge\",\n)\n";
        assert_eq!(format(&compact).unwrap(), expected);
        assert_eq!(format(&commented).unwrap(), expected);
    }

    #[test]
    fn formatter_preserves_branch_order_and_is_idempotent() {
        let tree = parse(
            "fn main() = all(refresh: runtime.refresh(runtime_id: \"runtime-a\"), inventory: runtime.list())",
        );
        let formatted = format(&tree).unwrap();
        assert_eq!(
            formatted,
            "fn main() = all(\n  refresh: runtime.refresh(runtime_id: \"runtime-a\"),\n  inventory: runtime.list(),\n)\n"
        );
        assert_eq!(format(&parse(&formatted)).unwrap(), formatted);
    }

    #[test]
    fn formatter_uses_only_supported_string_escapes() {
        let formatted = format(&parse(
            "fn main() = runtime.list(role: \"line\\n\\\"quoted\\\"\\\\tab\\t\")",
        ))
        .unwrap();
        assert_eq!(
            formatted,
            "fn main() = runtime.list(role: \"line\\n\\\"quoted\\\"\\\\tab\\t\")\n"
        );
        assert!(parse(&formatted).diagnostics.is_empty());
    }

    #[test]
    fn formatter_refuses_invalid_syntax() {
        let tree = parse("fn main() = runtime.list(role: @)");
        let errors = format(&tree).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "LSE0002"));
    }

    #[test]
    fn formatter_rejects_output_expansion_past_source_limit() {
        let source = format!(
            "fn main() = runtime.list(role: \"{}\")",
            "\t".repeat(MAX_SOURCE_BYTES / 2)
        );
        assert!(source.len() <= MAX_SOURCE_BYTES);
        let errors = format(&parse(&source)).unwrap_err();
        assert_eq!(errors[0].code, "LSE2002");
    }

    #[test]
    fn serialized_tree_rejects_invalid_spans_and_mutation_stays_panic_free() {
        let tree = parse("fn main() = runtime.list()");
        let mut encoded = serde_json::to_value(&tree).unwrap();
        encoded["tokens"][0]["span"]["end"] = serde_json::json!(usize::MAX);
        assert!(serde_json::from_value::<SyntaxTree>(encoded).is_err());

        let mut mutated = tree;
        mutated.tokens[0].span.end = usize::MAX;
        assert_eq!(mutated.token_text(&mutated.tokens[0]), None);
        assert_eq!(mutated.reconstruct(), None);
    }
}
