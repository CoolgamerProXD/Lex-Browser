use std::ops::Range;

/// Half-open UTF-8 byte offsets in the decoded input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpan(pub Range<usize>);

/// One normalized HTML attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub span: SourceSpan,
}

/// Lex HTML token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    Doctype { name: String, span: SourceSpan },
    StartTag {
        name: String,
        attributes: Vec<Attribute>,
        self_closing: bool,
        span: SourceSpan,
    },
    EndTag { name: String, span: SourceSpan },
    Text { data: String, span: SourceSpan },
    Comment { data: String, span: SourceSpan },
    CharacterReference {
        source: String,
        value: String,
        span: SourceSpan,
    },
}

impl Token {
    /// Source range occupied by this token.
    #[must_use]
    pub fn span(&self) -> &SourceSpan {
        match self {
            Self::Doctype { span, .. }
            | Self::StartTag { span, .. }
            | Self::EndTag { span, .. }
            | Self::Text { span, .. }
            | Self::Comment { span, .. }
            | Self::CharacterReference { span, .. } => span,
        }
    }
}

/// Recoverable tokenizer diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenizeError {
    pub offset: usize,
    pub message: String,
}

/// Complete tokenizer output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tokenization {
    pub tokens: Vec<Token>,
    pub errors: Vec<TokenizeError>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextMode {
    Data,
    RcData,
    RawText,
}

/// Stateful Lex-owned HTML tokenizer.
pub struct Tokenizer<'a> {
    source: &'a str,
    position: usize,
    output: Tokenization,
    text_mode: TextMode,
    raw_element: Option<String>,
}

impl<'a> Tokenizer<'a> {
    /// Creates a tokenizer over valid decoded UTF-8.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            output: Tokenization::default(),
            text_mode: TextMode::Data,
            raw_element: None,
        }
    }

    /// Tokenizes the complete input with recoverable diagnostics.
    #[must_use]
    pub fn tokenize(mut self) -> Tokenization {
        while self.position < self.source.len() {
            if self.text_mode != TextMode::Data {
                self.consume_special_text();
            } else if self.remaining().starts_with("<!--") {
                self.consume_comment();
            } else if starts_ascii_case_insensitive(self.remaining(), "<!doctype") {
                self.consume_doctype();
            } else if self.remaining().starts_with("</") && self.end_tag_looks_valid() {
                self.consume_end_tag();
            } else if self.remaining().starts_with('<') && self.start_tag_looks_valid() {
                self.consume_start_tag();
            } else if self.remaining().starts_with('&') {
                self.consume_character_reference_token();
            } else {
                self.consume_text();
            }
        }
        self.output
    }

    fn remaining(&self) -> &str {
        &self.source[self.position..]
    }

    fn error(&mut self, offset: usize, message: impl Into<String>) {
        self.output.errors.push(TokenizeError {
            offset,
            message: message.into(),
        });
    }

    fn start_tag_looks_valid(&self) -> bool {
        self.remaining()[1..]
            .chars()
            .next()
            .is_some_and(is_tag_name_char)
    }

    fn end_tag_looks_valid(&self) -> bool {
        self.remaining()[2..]
            .chars()
            .next()
            .is_some_and(is_tag_name_char)
    }

    fn consume_text(&mut self) {
        let start = self.position;
        while self.position < self.source.len() {
            let remaining = self.remaining();
            if remaining.starts_with('<') || remaining.starts_with('&') {
                break;
            }
            self.advance_char();
        }
        if self.position == start {
            self.advance_char();
        }
        self.output.tokens.push(Token::Text {
            data: self.source[start..self.position].into(),
            span: SourceSpan(start..self.position),
        });
    }

    fn consume_comment(&mut self) {
        let start = self.position;
        self.position += 4;
        if let Some(length) = self.remaining().find("-->") {
            let end = self.position + length;
            let data = self.source[self.position..end].into();
            self.position = end + 3;
            self.output.tokens.push(Token::Comment {
                data,
                span: SourceSpan(start..self.position),
            });
        } else {
            let data = self.remaining().into();
            self.position = self.source.len();
            self.error(start, "comment reached end of input before `-->`");
            self.output.tokens.push(Token::Comment {
                data,
                span: SourceSpan(start..self.position),
            });
        }
    }

    fn consume_doctype(&mut self) {
        let start = self.position;
        self.position += "<!doctype".len();
        self.skip_whitespace();
        let name_start = self.position;
        while self.position < self.source.len()
            && !self.peek_char().is_some_and(|character| character.is_whitespace() || character == '>')
        {
            self.advance_char();
        }
        let name = self.source[name_start..self.position].to_ascii_lowercase();
        if let Some(end) = self.remaining().find('>') {
            self.position += end + 1;
        } else {
            self.position = self.source.len();
            self.error(start, "doctype reached end of input before `>`");
        }
        self.output.tokens.push(Token::Doctype {
            name,
            span: SourceSpan(start..self.position),
        });
    }

    fn consume_start_tag(&mut self) {
        let start = self.position;
        self.position += 1;
        let name = self.consume_name();
        let mut attributes = Vec::new();
        let mut self_closing = false;
        loop {
            self.skip_whitespace();
            if self.position >= self.source.len() {
                self.error(start, "start tag reached end of input before `>`");
                break;
            }
            if self.remaining().starts_with("/>") {
                self.position += 2;
                self_closing = true;
                break;
            }
            if self.remaining().starts_with('>') {
                self.position += 1;
                break;
            }
            let attribute_start = self.position;
            let attribute_name = self.consume_attribute_name();
            if attribute_name.is_empty() {
                self.error(self.position, "invalid character in start tag");
                self.advance_char();
                continue;
            }
            self.skip_whitespace();
            let value = if self.remaining().starts_with('=') {
                self.position += 1;
                self.skip_whitespace();
                self.consume_attribute_value()
            } else {
                String::new()
            };
            if attributes.iter().any(|attribute: &Attribute| attribute.name == attribute_name) {
                self.error(attribute_start, format!("duplicate attribute `{attribute_name}`"));
            } else {
                attributes.push(Attribute {
                    name: attribute_name,
                    value,
                    span: SourceSpan(attribute_start..self.position),
                });
            }
        }
        self.output.tokens.push(Token::StartTag {
            name: name.clone(),
            attributes,
            self_closing,
            span: SourceSpan(start..self.position),
        });
        if !self_closing {
            match name.as_str() {
                "script" | "style" => {
                    self.text_mode = TextMode::RawText;
                    self.raw_element = Some(name);
                }
                "title" | "textarea" => {
                    self.text_mode = TextMode::RcData;
                    self.raw_element = Some(name);
                }
                _ => {}
            }
        }
    }

    fn consume_end_tag(&mut self) {
        let start = self.position;
        self.position += 2;
        let name = self.consume_name();
        self.skip_whitespace();
        if self.remaining().starts_with('>') {
            self.position += 1;
        } else if let Some(end) = self.remaining().find('>') {
            self.error(self.position, "unexpected content in end tag");
            self.position += end + 1;
        } else {
            self.error(start, "end tag reached end of input before `>`");
            self.position = self.source.len();
        }
        self.output.tokens.push(Token::EndTag {
            name,
            span: SourceSpan(start..self.position),
        });
    }

    fn consume_special_text(&mut self) {
        let start = self.position;
        let Some(element) = self.raw_element.clone() else {
            self.text_mode = TextMode::Data;
            return;
        };
        let closing_prefix = format!("</{element}");
        let end = find_ascii_case_insensitive(self.remaining(), &closing_prefix)
            .map_or(self.source.len(), |relative| self.position + relative);
        if end > start {
            let raw = &self.source[start..end];
            let data = if self.text_mode == TextMode::RcData {
                decode_references(raw)
            } else {
                raw.into()
            };
            self.output.tokens.push(Token::Text {
                data,
                span: SourceSpan(start..end),
            });
            self.position = end;
        }
        if self.position == self.source.len() {
            self.error(start, format!("`{element}` text reached end of input without a closing tag"));
            self.text_mode = TextMode::Data;
            self.raw_element = None;
        } else {
            self.text_mode = TextMode::Data;
            self.raw_element = None;
        }
    }

    fn consume_character_reference_token(&mut self) {
        let start = self.position;
        let (source, value, consumed, valid) = parse_reference(self.remaining());
        self.position += consumed;
        if !valid {
            self.error(start, format!("unknown or invalid character reference `{source}`"));
        }
        self.output.tokens.push(Token::CharacterReference {
            source,
            value,
            span: SourceSpan(start..self.position),
        });
    }

    fn consume_name(&mut self) -> String {
        let start = self.position;
        while self.peek_char().is_some_and(is_tag_name_char) {
            self.advance_char();
        }
        self.source[start..self.position].to_ascii_lowercase()
    }

    fn consume_attribute_name(&mut self) -> String {
        let start = self.position;
        while self.peek_char().is_some_and(|character| {
            !character.is_whitespace() && !matches!(character, '=' | '>' | '/' | '<' | '\0')
        }) {
            self.advance_char();
        }
        self.source[start..self.position].to_ascii_lowercase()
    }

    fn consume_attribute_value(&mut self) -> String {
        let Some(first) = self.peek_char() else {
            return String::new();
        };
        let (start, end) = if matches!(first, '\'' | '"') {
            self.advance_char();
            let start = self.position;
            while self.peek_char().is_some_and(|character| character != first) {
                self.advance_char();
            }
            let end = self.position;
            if self.peek_char() == Some(first) {
                self.advance_char();
            } else {
                self.error(start, "quoted attribute value reached end of input");
            }
            (start, end)
        } else {
            let start = self.position;
            while self.peek_char().is_some_and(|character| {
                !character.is_whitespace() && !matches!(character, '>' | '<' | '`')
            }) {
                self.advance_char();
            }
            (start, self.position)
        };
        decode_references(&self.source[start..end])
    }

    fn skip_whitespace(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.advance_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(character) = self.peek_char() {
            self.position += character.len_utf8();
        }
    }
}

fn is_tag_name_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | ':' | '_')
}

fn starts_ascii_case_insensitive(source: &str, prefix: &str) -> bool {
    source
        .get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
}

fn find_ascii_case_insensitive(source: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    source
        .char_indices()
        .map(|(offset, _)| offset)
        .find(|offset| starts_ascii_case_insensitive(&source[*offset..], needle))
}

fn decode_references(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut position = 0;
    while position < source.len() {
        if source[position..].starts_with('&') {
            let (_, value, consumed, _) = parse_reference(&source[position..]);
            result.push_str(&value);
            position += consumed;
        } else if let Some(character) = source[position..].chars().next() {
            result.push(character);
            position += character.len_utf8();
        }
    }
    result
}

fn parse_reference(source: &str) -> (String, String, usize, bool) {
    let end = source
        .find(';')
        .filter(|end| *end <= 32)
        .map_or(1, |end| end + 1);
    if end == 1 {
        return ("&".into(), "&".into(), 1, false);
    }
    let raw = &source[..end];
    let name = &raw[1..raw.len() - 1];
    let decoded = if let Some(number) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        u32::from_str_radix(number, 16).ok().and_then(char::from_u32)
    } else if let Some(number) = name.strip_prefix('#') {
        number.parse::<u32>().ok().and_then(char::from_u32)
    } else {
        match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{00a0}'),
            _ => None,
        }
    };
    decoded.map_or_else(
        || (raw.into(), raw.into(), end, false),
        |character| (raw.into(), character.to_string(), end, true),
    )
}

#[cfg(test)]
mod tests {
    use super::{Token, Tokenizer};

    #[test]
    fn emits_every_basic_token_category() {
        let output = Tokenizer::new("<!doctype HTML><!--x--><DIV>a&amp;b</div>").tokenize();
        assert!(matches!(&output.tokens[0], Token::Doctype { name, .. } if name == "html"));
        assert!(matches!(&output.tokens[1], Token::Comment { data, .. } if data == "x"));
        assert!(matches!(&output.tokens[2], Token::StartTag { name, .. } if name == "div"));
        assert!(matches!(&output.tokens[3], Token::Text { data, .. } if data == "a"));
        assert!(matches!(&output.tokens[4], Token::CharacterReference { value, .. } if value == "&"));
        assert!(matches!(&output.tokens[6], Token::EndTag { name, .. } if name == "div"));
    }

    #[test]
    fn parses_quoted_unquoted_boolean_and_entity_attributes() {
        let output = Tokenizer::new("<A HREF='/x?a=1&amp;b=2' title=hello disabled>").tokenize();
        let Token::StartTag { attributes, .. } = &output.tokens[0] else { panic!("start tag") };
        assert_eq!(attributes[0].name, "href");
        assert_eq!(attributes[0].value, "/x?a=1&b=2");
        assert_eq!(attributes[1].value, "hello");
        assert_eq!(attributes[2].value, "");
    }

    #[test]
    fn decodes_named_numeric_and_unknown_references_safely() {
        let output = Tokenizer::new("&lt;&#65;&#x1f642;&unknown;").tokenize();
        let values: Vec<_> = output.tokens.iter().filter_map(|token| match token {
            Token::CharacterReference { value, .. } => Some(value.as_str()),
            _ => None,
        }).collect();
        assert_eq!(values, ["<", "A", "🙂", "&unknown;"]);
        assert_eq!(output.errors.len(), 1);
    }

    #[test]
    fn preserves_whitespace_and_adjacent_text() {
        let output = Tokenizer::new(" one\n two ").tokenize();
        assert_eq!(output.tokens.len(), 1);
        assert!(matches!(&output.tokens[0], Token::Text { data, .. } if data == " one\n two "));
    }

    #[test]
    fn raw_text_does_not_parse_markup_or_entities() {
        let output = Tokenizer::new("<script>if (a < b) x = '&amp;';</SCRIPT><style>a>b{}</style>").tokenize();
        assert!(matches!(&output.tokens[1], Token::Text { data, .. } if data.contains("< b") && data.contains("&amp;")));
        assert!(matches!(&output.tokens[4], Token::Text { data, .. } if data == "a>b{}"));
    }

    #[test]
    fn eof_in_constructs_is_recoverable() {
        for source in ["<!-- open", "<!doctype", "<div x='open", "<script>open"] {
            let output = Tokenizer::new(source).tokenize();
            assert!(!output.tokens.is_empty());
            assert!(!output.errors.is_empty());
        }
    }
}
