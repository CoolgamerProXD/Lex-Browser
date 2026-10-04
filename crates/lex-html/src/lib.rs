//! Lex's HTML tokenizer and recovery parser.
//!
//! The implementation is intentionally independent of networking, DOM, UI,
//! and rendering. M4 produces an arena-backed intermediate document that M5
//! can convert into Lex's live DOM.

mod parser;
mod tokenizer;

pub use parser::{Document, ElementData, Node, NodeId, NodeRecord, ParseError, Parser};
pub use tokenizer::{Attribute, SourceSpan, Token, Tokenization, TokenizeError, Tokenizer};

/// Tokenizes and parses a decoded HTML string.
#[must_use]
pub fn parse(source: &str) -> Document {
    Parser::new().parse(source)
}

/// Decodes potentially invalid UTF-8 with replacement characters, then parses
/// it without panicking.
#[must_use]
pub fn parse_bytes(bytes: &[u8]) -> Document {
    parse(&String::from_utf8_lossy(bytes))
}
