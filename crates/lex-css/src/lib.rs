//! Lex-owned CSS tokenization, parsing, selector matching, and cascade foundations.
//!
//! This intentionally implements a small, deterministic CSS subset. Unknown
//! properties and values are retained so later engine milestones can extend it.

use lex_dom::{Document, Node, NodeId};
use std::collections::BTreeMap;

/// Half-open byte range in the original UTF-8 source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

/// CSS lexical token.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
}

/// Token payloads needed by the M6 grammar.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Hash(String),
    String(String),
    Number(f32),
    Percentage(f32),
    Dimension(f32, String),
    Whitespace,
    Colon,
    Semicolon,
    Comma,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    Delim(char),
}

/// Tokenizes CSS without discarding source positions.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let c = bytes[i] as char;
        if c.is_ascii_whitespace() {
            i += 1;
            while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
                i += 1;
            }
            push(&mut out, TokenKind::Whitespace, start, i);
            continue;
        }
        if source[start..].starts_with("/*") {
            i += 2;
            while i + 1 < bytes.len() && &source[i..i + 2] != "*/" {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        let simple = match c {
            ':' => Some(TokenKind::Colon),
            ';' => Some(TokenKind::Semicolon),
            ',' => Some(TokenKind::Comma),
            '{' => Some(TokenKind::OpenBrace),
            '}' => Some(TokenKind::CloseBrace),
            '[' => Some(TokenKind::OpenBracket),
            ']' => Some(TokenKind::CloseBracket),
            '(' => Some(TokenKind::OpenParen),
            ')' => Some(TokenKind::CloseParen),
            _ => None,
        };
        if let Some(kind) = simple {
            i += 1;
            push(&mut out, kind, start, i);
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut value = String::new();
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == quote {
                    i += 1;
                    break;
                }
                if ch == '\\' && i + 1 < bytes.len() {
                    i += 1;
                }
                value.push(bytes[i] as char);
                i += 1;
            }
            push(&mut out, TokenKind::String(value), start, i);
            continue;
        }
        if c == '#' && i + 1 < bytes.len() && is_name(bytes[i + 1] as char) {
            i += 1;
            let s = i;
            while i < bytes.len() && is_name(bytes[i] as char) {
                i += 1;
            }
            push(
                &mut out,
                TokenKind::Hash(source[s..i].to_string()),
                start,
                i,
            );
            continue;
        }
        if c.is_ascii_digit()
            || (c == '.' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
        {
            i += 1;
            while i < bytes.len() && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let n = source[start..i].parse().unwrap_or(0.0);
            if i < bytes.len() && bytes[i] == b'%' {
                i += 1;
                push(&mut out, TokenKind::Percentage(n), start, i);
            } else if i < bytes.len() && is_name_start(bytes[i] as char) {
                let u = i;
                i += 1;
                while i < bytes.len() && is_name(bytes[i] as char) {
                    i += 1;
                }
                push(
                    &mut out,
                    TokenKind::Dimension(n, source[u..i].to_ascii_lowercase()),
                    start,
                    i,
                );
            } else {
                push(&mut out, TokenKind::Number(n), start, i);
            }
            continue;
        }
        if is_name_start(c)
            || (c == '-' && i + 1 < bytes.len() && is_name_start(bytes[i + 1] as char))
        {
            i += 1;
            while i < bytes.len() && is_name(bytes[i] as char) {
                i += 1;
            }
            push(
                &mut out,
                TokenKind::Ident(source[start..i].to_ascii_lowercase()),
                start,
                i,
            );
            continue;
        }
        i += 1;
        push(&mut out, TokenKind::Delim(c), start, i);
    }
    out
}

fn push(out: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize) {
    out.push(Token {
        kind,
        span: SourceSpan { start, end },
    });
}
fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}
fn is_name(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-'
}

/// Parsed stylesheet with recoverable diagnostics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<StyleRule>,
    pub errors: Vec<ParseError>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, PartialEq)]
pub struct StyleRule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
    pub span: SourceSpan,
    pub source_order: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: CssValue,
    pub important: bool,
    pub span: SourceSpan,
}

/// Common value forms; `Raw` preserves valid future syntax.
#[derive(Clone, Debug, PartialEq)]
pub enum CssValue {
    Keyword(String),
    Number(f32),
    Percentage(f32),
    Length(f32, LengthUnit),
    Color(Color),
    String(String),
    Raw(String),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Vw,
    Vh,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selector {
    pub parts: Vec<SelectorPart>,
    pub specificity: Specificity,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorPart {
    pub compound: CompoundSelector,
    pub combinator_to_left: Option<Combinator>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompoundSelector {
    pub type_selector: Option<String>,
    pub universal: bool,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttributeSelector>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributeSelector {
    pub name: String,
    pub value: Option<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Combinator {
    Descendant,
    Child,
}
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Specificity(pub u16, pub u16, pub u16);

/// Parses a stylesheet and recovers at the next rule/declaration boundary.
#[must_use]
pub fn parse(source: &str) -> Stylesheet {
    let tokens = tokenize(source);
    let mut p = Parser {
        source,
        tokens,
        pos: 0,
        errors: Vec::new(),
    };
    let mut rules = Vec::new();
    while p.skip_ws() < p.tokens.len() {
        let start = p.here();
        let prelude = p.take_until_brace();
        if prelude.is_empty() || !p.eat_open_brace() {
            p.error("expected selector and '{'", start);
            p.recover_rule();
            continue;
        }
        let selectors = parse_selector_list(&prelude, source);
        if selectors.is_empty() {
            p.error("invalid selector", start);
        }
        let declarations = p.declarations();
        let end = p.here();
        if !selectors.is_empty() {
            rules.push(StyleRule {
                selectors,
                declarations,
                span: SourceSpan { start, end },
                source_order: rules.len(),
            });
        }
    }
    Stylesheet {
        rules,
        errors: p.errors,
    }
}
struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}
impl Parser<'_> {
    fn skip_ws(&mut self) -> usize {
        while matches!(
            self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::Whitespace)
        ) {
            self.pos += 1;
        }
        self.pos
    }
    fn here(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map_or(self.source.len(), |t| t.span.start)
    }
    fn take_until_brace(&mut self) -> Vec<Token> {
        let s = self.pos;
        while self.pos < self.tokens.len()
            && !matches!(
                self.tokens[self.pos].kind,
                TokenKind::OpenBrace | TokenKind::CloseBrace
            )
        {
            self.pos += 1;
        }
        self.tokens[s..self.pos].to_vec()
    }
    fn eat_open_brace(&mut self) -> bool {
        if matches!(
            self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::OpenBrace)
        ) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn error(&mut self, msg: &str, at: usize) {
        self.errors.push(ParseError {
            message: msg.into(),
            span: SourceSpan { start: at, end: at },
        });
    }
    fn recover_rule(&mut self) {
        while self.pos < self.tokens.len()
            && !matches!(self.tokens[self.pos].kind, TokenKind::CloseBrace)
        {
            self.pos += 1;
        }
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }
    fn declarations(&mut self) -> Vec<Declaration> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.tokens.len() {
                self.error("unclosed declaration block", self.here());
                break;
            }
            if matches!(self.tokens[self.pos].kind, TokenKind::CloseBrace) {
                self.pos += 1;
                break;
            }
            let start = self.here();
            let Some(TokenKind::Ident(name)) =
                self.tokens.get(self.pos).map(|token| token.kind.clone())
            else {
                self.error("expected property name", start);
                self.recover_decl();
                continue;
            };
            self.pos += 1;
            self.skip_ws();
            if !matches!(
                self.tokens.get(self.pos).map(|t| &t.kind),
                Some(TokenKind::Colon)
            ) {
                self.error("expected ':'", start);
                self.recover_decl();
                continue;
            }
            self.pos += 1;
            let value_start = self.pos;
            while self.pos < self.tokens.len()
                && !matches!(
                    self.tokens[self.pos].kind,
                    TokenKind::Semicolon | TokenKind::CloseBrace
                )
            {
                self.pos += 1;
            }
            let mut values = self.tokens[value_start..self.pos].to_vec();
            trim_ws(&mut values);
            let important = remove_important(&mut values);
            if values.is_empty() {
                self.error("empty property value", start);
            } else {
                let end = values.last().map_or(start, |t| t.span.end);
                out.push(Declaration {
                    name,
                    value: parse_value(&values, self.source),
                    important,
                    span: SourceSpan { start, end },
                });
            }
            if matches!(
                self.tokens.get(self.pos).map(|t| &t.kind),
                Some(TokenKind::Semicolon)
            ) {
                self.pos += 1;
            }
        }
        out
    }
    fn recover_decl(&mut self) {
        while self.pos < self.tokens.len()
            && !matches!(
                self.tokens[self.pos].kind,
                TokenKind::Semicolon | TokenKind::CloseBrace
            )
        {
            self.pos += 1;
        }
        if matches!(
            self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::Semicolon)
        ) {
            self.pos += 1;
        }
    }
}
fn trim_ws(v: &mut Vec<Token>) {
    while matches!(v.first().map(|t| &t.kind), Some(TokenKind::Whitespace)) {
        v.remove(0);
    }
    while matches!(v.last().map(|t| &t.kind), Some(TokenKind::Whitespace)) {
        v.pop();
    }
}
fn remove_important(v: &mut Vec<Token>) -> bool {
    trim_ws(v);
    if v.len() >= 2
        && matches!(v[v.len() - 2].kind, TokenKind::Delim('!'))
        && matches!(&v[v.len()-1].kind,TokenKind::Ident(s) if s=="important")
    {
        v.truncate(v.len() - 2);
        trim_ws(v);
        true
    } else {
        false
    }
}
fn parse_value(v: &[Token], source: &str) -> CssValue {
    if v.len() == 1 {
        match &v[0].kind {
            TokenKind::Ident(s) => match s.as_str() {
                "red" => CssValue::Color(Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                }),
                "black" => CssValue::Color(Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                }),
                "white" => CssValue::Color(Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                }),
                _ => CssValue::Keyword(s.clone()),
            },
            TokenKind::Hash(h) => {
                parse_hex(h).map_or_else(|| CssValue::Raw(format!("#{h}")), CssValue::Color)
            }
            TokenKind::Number(n) => CssValue::Number(*n),
            TokenKind::Percentage(n) => CssValue::Percentage(*n),
            TokenKind::Dimension(n, u) => unit(u).map_or_else(
                || CssValue::Raw(format!("{n}{u}")),
                |x| CssValue::Length(*n, x),
            ),
            TokenKind::String(s) => CssValue::String(s.clone()),
            _ => raw(v, source),
        }
    } else {
        raw(v, source)
    }
}
fn raw(v: &[Token], source: &str) -> CssValue {
    CssValue::Raw(
        source[v[0].span.start..v[v.len() - 1].span.end]
            .trim()
            .into(),
    )
}
fn unit(s: &str) -> Option<LengthUnit> {
    match s {
        "px" => Some(LengthUnit::Px),
        "em" => Some(LengthUnit::Em),
        "rem" => Some(LengthUnit::Rem),
        "vw" => Some(LengthUnit::Vw),
        "vh" => Some(LengthUnit::Vh),
        _ => None,
    }
}
fn parse_hex(s: &str) -> Option<Color> {
    let x = u32::from_str_radix(s, 16).ok()?;
    match s.len() {
        3 => Some(Color {
            red: u8::try_from((x >> 8) & 15).ok()? * 17,
            green: u8::try_from((x >> 4) & 15).ok()? * 17,
            blue: u8::try_from(x & 15).ok()? * 17,
            alpha: 255,
        }),
        6 => Some(Color {
            red: u8::try_from((x >> 16) & 255).ok()?,
            green: u8::try_from((x >> 8) & 255).ok()?,
            blue: u8::try_from(x & 255).ok()?,
            alpha: 255,
        }),
        _ => None,
    }
}
fn parse_selector_list(tokens: &[Token], source: &str) -> Vec<Selector> {
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..=tokens.len() {
        if i == tokens.len() || matches!(tokens[i].kind, TokenKind::Comma) {
            if let Some(s) = parse_selector(&tokens[start..i], source) {
                out.push(s);
            }
            start = i + 1;
        }
    }
    out
}
#[allow(clippy::too_many_lines)]
fn parse_selector(input: &[Token], _source: &str) -> Option<Selector> {
    let mut t = input.to_vec();
    trim_ws(&mut t);
    if t.is_empty() {
        return None;
    }
    let span = SourceSpan {
        start: t[0].span.start,
        end: t.last()?.span.end,
    };
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    let mut i = 0;
    let mut current = CompoundSelector::default();
    let mut has = false;
    while i < t.len() {
        match &t[i].kind {
            TokenKind::Whitespace => {
                while i < t.len() && matches!(t[i].kind, TokenKind::Whitespace) {
                    i += 1;
                }
                if has && i < t.len() && !matches!(t[i].kind, TokenKind::Delim('>')) {
                    compounds.push(current);
                    current = CompoundSelector::default();
                    has = false;
                    combinators.push(Combinator::Descendant);
                }
                continue;
            }
            TokenKind::Delim('>') => {
                if !has {
                    return None;
                }
                compounds.push(current);
                current = CompoundSelector::default();
                has = false;
                combinators.push(Combinator::Child);
                i += 1;
                while i < t.len() && matches!(t[i].kind, TokenKind::Whitespace) {
                    i += 1;
                }
                continue;
            }
            TokenKind::Delim('*') if !has => {
                current.universal = true;
                has = true
            }
            TokenKind::Ident(s) if !has => {
                current.type_selector = Some(s.clone());
                has = true
            }
            TokenKind::Hash(s) => {
                current.id = Some(s.clone());
                has = true
            }
            TokenKind::Delim('.') => {
                i += 1;
                if let Some(Token {
                    kind: TokenKind::Ident(s),
                    ..
                }) = t.get(i)
                {
                    current.classes.push(s.clone());
                    has = true
                } else {
                    return None;
                }
            }
            TokenKind::OpenBracket => {
                i += 1;
                while matches!(t.get(i).map(|x| &x.kind), Some(TokenKind::Whitespace)) {
                    i += 1;
                }
                let name = match t.get(i).map(|x| &x.kind) {
                    Some(TokenKind::Ident(s)) => s.clone(),
                    _ => return None,
                };
                i += 1;
                while matches!(t.get(i).map(|x| &x.kind), Some(TokenKind::Whitespace)) {
                    i += 1;
                }
                let value = if matches!(t.get(i).map(|x| &x.kind), Some(TokenKind::Delim('='))) {
                    i += 1;
                    while matches!(t.get(i).map(|x| &x.kind), Some(TokenKind::Whitespace)) {
                        i += 1;
                    }
                    let v = match t.get(i).map(|x| &x.kind) {
                        Some(TokenKind::Ident(s) | TokenKind::String(s)) => s.clone(),
                        _ => return None,
                    };
                    i += 1;
                    Some(v)
                } else {
                    None
                };
                while matches!(t.get(i).map(|x| &x.kind), Some(TokenKind::Whitespace)) {
                    i += 1;
                }
                if !matches!(t.get(i).map(|x| &x.kind), Some(TokenKind::CloseBracket)) {
                    return None;
                }
                current.attributes.push(AttributeSelector { name, value });
                has = true
            }
            _ => return None,
        }
        i += 1;
    }
    if !has {
        return None;
    }
    compounds.push(current);
    if compounds.len() != combinators.len() + 1 {
        return None;
    }
    let mut specificity = Specificity::default();
    for c in &compounds {
        specificity.0 += u16::from(c.id.is_some());
        specificity.1 += u16::try_from(c.classes.len() + c.attributes.len()).unwrap_or(u16::MAX);
        specificity.2 += u16::from(c.type_selector.is_some());
    }
    let parts = compounds
        .into_iter()
        .enumerate()
        .map(|(n, compound)| SelectorPart {
            compound,
            combinator_to_left: if n == 0 {
                None
            } else {
                Some(combinators[n - 1])
            },
        })
        .collect();
    Some(Selector {
        parts,
        specificity,
        span,
    })
}

/// Returns whether an element matches a parsed selector.
#[must_use]
pub fn matches_selector(document: &Document, element: NodeId, selector: &Selector) -> bool {
    fn rec(d: &Document, node: NodeId, s: &Selector, index: usize) -> bool {
        if !matches_compound(d, node, &s.parts[index].compound) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match s.parts[index].combinator_to_left {
            Some(Combinator::Child) => d.parent(node).is_some_and(|p| rec(d, p, s, index - 1)),
            Some(Combinator::Descendant) => {
                let mut p = d.parent(node);
                while let Some(id) = p {
                    if rec(d, id, s, index - 1) {
                        return true;
                    }
                    p = d.parent(id);
                }
                false
            }
            None => false,
        }
    }
    !selector.parts.is_empty() && rec(document, element, selector, selector.parts.len() - 1)
}
fn matches_compound(d: &Document, id: NodeId, c: &CompoundSelector) -> bool {
    let Some(record) = d.node(id) else {
        return false;
    };
    let Node::Element(data) = &record.node else {
        return false;
    };
    if let Some(tag) = &c.type_selector {
        if &data.tag_name != tag {
            return false;
        }
    }
    if let Some(expected) = &c.id {
        if d.attribute(id, "id").ok().flatten() != Some(expected.as_str()) {
            return false;
        }
    }
    let classes = d.attribute(id, "class").ok().flatten().unwrap_or("");
    if c.classes
        .iter()
        .any(|x| !classes.split_ascii_whitespace().any(|actual| actual == x))
    {
        return false;
    }
    for a in &c.attributes {
        let actual = d.attribute(id, &a.name).ok().flatten();
        if actual.is_none() || a.value.as_deref().is_some_and(|v| actual != Some(v)) {
            return false;
        }
    }
    true
}

/// Winning declared values after importance, specificity, and source order.
pub type ComputedStyle = BTreeMap<String, CssValue>;
/// Computes the cascade foundation for one element (without inheritance or defaults).
#[must_use]
pub fn compute_style(document: &Document, element: NodeId, sheets: &[Stylesheet]) -> ComputedStyle {
    let mut winners: BTreeMap<String, (bool, Specificity, usize, CssValue)> = BTreeMap::new();
    let mut global_order = 0;
    for sheet in sheets {
        for rule in &sheet.rules {
            let specificity = rule
                .selectors
                .iter()
                .filter(|s| matches_selector(document, element, s))
                .map(|s| s.specificity)
                .max();
            if let Some(spec) = specificity {
                for decl in &rule.declarations {
                    let rank = (decl.important, spec, global_order);
                    let replace = winners
                        .get(&decl.name)
                        .map_or(true, |old| rank >= (old.0, old.1, old.2));
                    if replace {
                        winners.insert(
                            decl.name.clone(),
                            (decl.important, spec, global_order, decl.value.clone()),
                        );
                    }
                }
            }
            global_order += 1;
        }
    }
    winners
        .into_iter()
        .map(|(name, (_, _, _, value))| (name, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokenization_and_spans() {
        let t = tokenize("h1 { width: 10px; color: #abc }");
        assert!(matches!(t[0].kind,TokenKind::Ident(ref x) if x=="h1"));
        assert_eq!(t[0].span, SourceSpan { start: 0, end: 2 });
        assert!(t
            .iter()
            .any(|x| matches!(x.kind,TokenKind::Dimension(10.0,ref u) if u=="px")));
    }
    #[test]
    fn selectors_and_specificity() {
        let s = parse("main > .card[data-x=yes], #hero { color:red }");
        assert_eq!(s.rules[0].selectors.len(), 2);
        assert_eq!(s.rules[0].selectors[0].specificity, Specificity(0, 2, 1));
        assert_eq!(s.rules[0].selectors[1].specificity, Specificity(1, 0, 0));
    }
    #[test]
    fn declarations_and_values() {
        let s = parse("a{width:12px;color:#ff0000; opacity: .5 !important}");
        let d = &s.rules[0].declarations;
        assert_eq!(d[0].value, CssValue::Length(12.0, LengthUnit::Px));
        assert_eq!(
            d[1].value,
            CssValue::Color(Color {
                red: 255,
                green: 0,
                blue: 0,
                alpha: 255
            })
        );
        assert!(d[2].important);
    }
    #[test]
    fn recovery_keeps_later_declarations_and_rules() {
        let s = parse("a { broken; color: red } ??? { x:y } p { width: 2px }");
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.rules[0].declarations.len(), 1);
        assert!(!s.errors.is_empty());
        assert_eq!(s.rules[1].declarations[0].name, "width");
    }
    #[test]
    fn matching_and_cascade() {
        let mut d = Document::new();
        let main = d.create_element("main");
        let p = d.create_element("p");
        d.set_attribute(p, "id", "hero").unwrap();
        d.set_attribute(p, "class", "lead card").unwrap();
        d.append_child(d.root(), main).unwrap();
        d.append_child(main, p).unwrap();
        let s = parse(
            "p {color:black} main > .lead {color:red} #hero {width:10px} [class] {display:block}",
        );
        assert!(matches_selector(&d, p, &s.rules[1].selectors[0]));
        let style = compute_style(&d, p, &[s]);
        assert_eq!(
            style["color"],
            CssValue::Color(Color {
                red: 255,
                green: 0,
                blue: 0,
                alpha: 255
            })
        );
        assert_eq!(style["width"], CssValue::Length(10.0, LengthUnit::Px));
    }
}
