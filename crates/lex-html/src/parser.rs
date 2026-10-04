use crate::{Attribute, SourceSpan, Token, Tokenizer};

/// Stable index into a parsed document's node arena.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(pub usize);

/// Parsed element data, independent of the future live DOM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementData {
    pub name: String,
    pub attributes: Vec<Attribute>,
}

/// Intermediate HTML tree node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Document,
    Doctype(String),
    Element(ElementData),
    Text(String),
    Comment(String),
}

/// One arena entry with relationships and source provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeRecord {
    pub node: Node,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub span: Option<SourceSpan>,
}

/// Recoverable tree-construction diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

/// Arena-backed intermediate document produced by M4.
#[derive(Clone, Debug)]
pub struct Document {
    nodes: Vec<NodeRecord>,
    root: NodeId,
    errors: Vec<ParseError>,
}

impl Document {
    /// Document root node.
    #[must_use]
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Looks up an arena node.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&NodeRecord> {
        self.nodes.get(id.0)
    }

    /// All nodes in stable insertion order.
    #[must_use]
    pub fn nodes(&self) -> &[NodeRecord] {
        &self.nodes
    }

    /// Tokenization and tree-recovery diagnostics.
    #[must_use]
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Returns the first element with the normalized tag name.
    #[must_use]
    pub fn first_element(&self, name: &str) -> Option<NodeId> {
        self.nodes.iter().enumerate().find_map(|(index, record)| {
            matches!(&record.node, Node::Element(element) if element.name == name)
                .then_some(NodeId(index))
        })
    }

    /// Concatenates text descendants in tree order.
    #[must_use]
    pub fn text_content(&self, id: NodeId) -> String {
        let mut result = String::new();
        self.append_text(id, &mut result);
        result
    }

    fn append_text(&self, id: NodeId, result: &mut String) {
        let Some(record) = self.node(id) else {
            return;
        };
        if let Node::Text(text) = &record.node {
            result.push_str(text);
        }
        for child in &record.children {
            self.append_text(*child, result);
        }
    }
}

/// HTML tree constructor with bounded, deterministic error recovery.
#[derive(Debug, Default)]
pub struct Parser;

impl Parser {
    /// Creates a parser.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Tokenizes and constructs an intermediate document.
    #[must_use]
    pub fn parse(&self, source: &str) -> Document {
        let tokenization = Tokenizer::new(source).tokenize();
        let mut document = Document {
            nodes: vec![NodeRecord {
                node: Node::Document,
                parent: None,
                children: Vec::new(),
                span: Some(SourceSpan(0..source.len())),
            }],
            root: NodeId(0),
            errors: tokenization
                .errors
                .into_iter()
                .map(|error| ParseError {
                    offset: error.offset,
                    message: error.message,
                })
                .collect(),
        };
        let mut open = vec![document.root];

        for token in tokenization.tokens {
            match token {
                Token::Doctype { name, span } => {
                    let root = document.root;
                    append(&mut document, root, Node::Doctype(name), Some(span));
                }
                Token::Comment { data, span } => {
                    let parent = *open.last().unwrap_or(&document.root);
                    append(&mut document, parent, Node::Comment(data), Some(span));
                }
                Token::Text { data, span } => {
                    append_text(&mut document, *open.last().unwrap_or(&document.root), data, span);
                }
                Token::CharacterReference { value, span, .. } => {
                    append_text(&mut document, *open.last().unwrap_or(&document.root), value, span);
                }
                Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                    span,
                } => {
                    recover_before_start(&mut document, &mut open, &name, span.0.start);
                    let parent = *open.last().unwrap_or(&document.root);
                    let id = append(
                        &mut document,
                        parent,
                        Node::Element(ElementData {
                            name: name.clone(),
                            attributes,
                        }),
                        Some(span),
                    );
                    if !self_closing && !is_void_element(&name) {
                        open.push(id);
                    }
                }
                Token::EndTag { name, span } => {
                    close_element(&mut document, &mut open, &name, span.0.start);
                }
            }
        }

        if open.len() > 1 {
            for id in open.iter().skip(1).rev() {
                let name = element_name(&document, *id).unwrap_or("unknown");
                document.errors.push(ParseError {
                    offset: source.len(),
                    message: format!("unclosed element `<{name}>` at end of input"),
                });
            }
        }
        document
    }
}

fn append(
    document: &mut Document,
    parent: NodeId,
    node: Node,
    span: Option<SourceSpan>,
) -> NodeId {
    let id = NodeId(document.nodes.len());
    document.nodes.push(NodeRecord {
        node,
        parent: Some(parent),
        children: Vec::new(),
        span,
    });
    if let Some(parent) = document.nodes.get_mut(parent.0) {
        parent.children.push(id);
    }
    id
}

fn append_text(document: &mut Document, parent: NodeId, data: String, span: SourceSpan) {
    if data.is_empty() {
        return;
    }
    if let Some(last) = document.nodes[parent.0].children.last().copied() {
        if let Node::Text(existing) = &mut document.nodes[last.0].node {
            existing.push_str(&data);
            if let Some(existing_span) = &mut document.nodes[last.0].span {
                existing_span.0.end = span.0.end;
            }
            return;
        }
    }
    append(document, parent, Node::Text(data), Some(span));
}

fn recover_before_start(
    document: &mut Document,
    open: &mut Vec<NodeId>,
    incoming: &str,
    offset: usize,
) {
    if incoming == "li" {
        close_if_open(document, open, "li", offset, "implicitly closed `<li>`");
    }
    if incoming == "p" || is_block_element(incoming) {
        close_if_open(document, open, "p", offset, "implicitly closed `<p>`");
    }
    if matches!(incoming, "tr") {
        close_if_open(document, open, "tr", offset, "implicitly closed `<tr>`");
    }
    if matches!(incoming, "td" | "th") {
        close_if_open(document, open, "td", offset, "implicitly closed table cell");
        close_if_open(document, open, "th", offset, "implicitly closed table cell");
    }
}

fn close_if_open(
    document: &mut Document,
    open: &mut Vec<NodeId>,
    name: &str,
    offset: usize,
    message: &str,
) {
    if let Some(index) = open.iter().rposition(|id| element_name(document, *id) == Some(name)) {
        open.truncate(index);
        document.errors.push(ParseError {
            offset,
            message: message.into(),
        });
    }
}

fn close_element(document: &mut Document, open: &mut Vec<NodeId>, name: &str, offset: usize) {
    let Some(index) = open.iter().rposition(|id| element_name(document, *id) == Some(name)) else {
        document.errors.push(ParseError {
            offset,
            message: format!("ignored unmatched closing tag `</{name}>`"),
        });
        return;
    };
    if index + 1 != open.len() {
        document.errors.push(ParseError {
            offset,
            message: format!("closing `</{name}>` implicitly closed nested elements"),
        });
    }
    open.truncate(index);
}

fn element_name(document: &Document, id: NodeId) -> Option<&str> {
    match &document.nodes.get(id.0)?.node {
        Node::Element(element) => Some(&element.name),
        _ => None,
    }
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_block_element(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "div"
            | "dl"
            | "fieldset"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "ul"
    )
}

#[cfg(test)]
mod tests {
    use crate::{parse, Node};

    #[test]
    fn builds_nested_elements_and_merges_text_boundaries() {
        let document = parse("<div>Hello &amp; <span>Lex</span></div>");
        let div = document.first_element("div").unwrap();
        assert_eq!(document.text_content(div), "Hello & Lex");
        let children = &document.node(div).unwrap().children;
        assert!(matches!(document.node(children[0]).unwrap().node, Node::Text(_)));
        assert!(document.first_element("span").is_some());
    }

    #[test]
    fn recovers_from_crossed_and_unmatched_tags() {
        let document = parse("<div><span>x</div></missing>");
        assert_eq!(document.text_content(document.root()), "x");
        assert!(document.errors().len() >= 2);
    }

    #[test]
    fn implicitly_closes_paragraphs_and_list_items() {
        let document = parse("<p>one<div>two</div><ul><li>a<li>b</ul>");
        assert!(document.errors().iter().any(|error| error.message.contains("<p>")));
        assert_eq!(document.nodes().iter().filter(|node| matches!(&node.node, Node::Element(element) if element.name == "li")).count(), 2);
    }

    #[test]
    fn handles_common_document_structures() {
        let source = "<!doctype html><html><head><title>T</title><meta charset=utf-8><style>x{}</style></head><body><h1>H</h1><p>P</p><a href=/x>A</a><img src=x><ul><li>L</li></ul><table><tr><td>C</td></tr></table><form><input></form><script>x<y</script></body></html>";
        let document = parse(source);
        for name in ["html", "head", "title", "meta", "style", "body", "h1", "p", "a", "img", "ul", "li", "table", "tr", "td", "form", "input", "script"] {
            assert!(document.first_element(name).is_some(), "missing {name}");
        }
    }
}
