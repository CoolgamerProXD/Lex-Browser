//! Lex's mutable Document Object Model.
//!
//! Nodes live in a document-owned arena and refer to one another by stable
//! [`NodeId`] values. This avoids reference cycles and gives future style,
//! event, script, and `DevTools` systems durable handles.

use std::collections::HashSet;

use lex_html::{Document as ParsedDocument, Node as ParsedNode, SourceSpan};
use thiserror::Error;

/// Stable handle into a [`Document`]'s node arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(pub usize);

/// DOM attribute with optional parser provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub source_span: Option<SourceSpan>,
}

/// HTML element data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementData {
    pub tag_name: String,
    pub attributes: Vec<Attribute>,
}

/// DOM node payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Document,
    DocumentFragment,
    Element(ElementData),
    Text(String),
    Comment(String),
    Doctype(String),
}

/// Arena entry containing a payload and tree relationships.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeRecord {
    pub node: Node,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub source_span: Option<SourceSpan>,
}

/// Mutation details suitable for future style/layout invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationRecord {
    ChildInserted {
        parent: NodeId,
        node: NodeId,
        index: usize,
    },
    ChildRemoved {
        parent: NodeId,
        node: NodeId,
        index: usize,
    },
    AttributeChanged {
        element: NodeId,
        name: String,
        old_value: Option<String>,
        new_value: Option<String>,
    },
    CharacterDataChanged {
        node: NodeId,
        old_value: String,
        new_value: String,
    },
}

/// A deterministic collection of records produced by one operation.
pub type MutationBatch = Vec<MutationRecord>;

/// Safe DOM operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DomError {
    #[error("node {0:?} does not exist in this document")]
    MissingNode(NodeId),
    #[error("node {0:?} cannot have children")]
    ParentCannotHaveChildren(NodeId),
    #[error("inserting node {node:?} below {parent:?} would create a cycle")]
    HierarchyCycle { parent: NodeId, node: NodeId },
    #[error("reference node {reference:?} is not a child of {parent:?}")]
    ReferenceNotChild { parent: NodeId, reference: NodeId },
    #[error("node {0:?} has no parent")]
    HasNoParent(NodeId),
    #[error("operation requires an element node, got {0:?}")]
    NotAnElement(NodeId),
    #[error("operation requires text or comment character data, got {0:?}")]
    NotCharacterData(NodeId),
    #[error("a document or doctype node cannot be inserted here")]
    InvalidChildType,
    #[error("parsed document references a missing node")]
    InvalidParsedDocument,
}

/// Invariant violation reported without panicking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvariantError {
    pub node: NodeId,
    pub message: String,
}

/// Mutable DOM document and node arena.
#[derive(Clone, Debug)]
pub struct Document {
    nodes: Vec<NodeRecord>,
    root: NodeId,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Creates an empty document with a stable root node.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![NodeRecord {
                node: Node::Document,
                parent: None,
                children: Vec::new(),
                source_span: None,
            }],
            root: NodeId(0),
        }
    }

    /// Converts the M4 parser's intermediate tree into the authoritative DOM.
    ///
    /// # Errors
    /// Returns an error if the intermediate arena contains invalid handles.
    pub fn from_parsed(parsed: &ParsedDocument) -> Result<Self, DomError> {
        let mut document = Self::new();
        let parsed_root = parsed
            .node(parsed.root())
            .ok_or(DomError::InvalidParsedDocument)?;
        let mut work: Vec<(lex_html::NodeId, NodeId)> = parsed_root
            .children
            .iter()
            .rev()
            .map(|child| (*child, document.root))
            .collect();

        while let Some((parsed_id, parent)) = work.pop() {
            let parsed_record = parsed
                .node(parsed_id)
                .ok_or(DomError::InvalidParsedDocument)?;
            let node = match &parsed_record.node {
                ParsedNode::Document => continue,
                ParsedNode::Doctype(name) => Node::Doctype(name.clone()),
                ParsedNode::Element(element) => Node::Element(ElementData {
                    tag_name: element.name.clone(),
                    attributes: element
                        .attributes
                        .iter()
                        .map(|attribute| Attribute {
                            name: attribute.name.clone(),
                            value: attribute.value.clone(),
                            source_span: Some(attribute.span.clone()),
                        })
                        .collect(),
                }),
                ParsedNode::Text(data) => Node::Text(data.clone()),
                ParsedNode::Comment(data) => Node::Comment(data.clone()),
            };
            let id = document.allocate(node, parsed_record.span.clone());
            document.attach_unchecked(parent, id);
            for child in parsed_record.children.iter().rev() {
                work.push((*child, id));
            }
        }
        Ok(document)
    }

    /// Parses HTML bytes through `lex-html` and constructs a mutable DOM.
    ///
    /// # Errors
    /// Returns an error only if the parser's intermediate tree is internally
    /// inconsistent.
    pub fn from_html_bytes(bytes: &[u8]) -> Result<Self, DomError> {
        Self::from_parsed(&lex_html::parse_bytes(bytes))
    }

    /// Document root handle.
    #[must_use]
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Looks up a node.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&NodeRecord> {
        self.nodes.get(id.0)
    }

    /// Number of allocated nodes, including detached nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Creates a detached HTML element.
    pub fn create_element(&mut self, tag_name: impl Into<String>) -> NodeId {
        self.allocate(
            Node::Element(ElementData {
                tag_name: tag_name.into().to_ascii_lowercase(),
                attributes: Vec::new(),
            }),
            None,
        )
    }

    /// Creates a detached text node.
    pub fn create_text(&mut self, data: impl Into<String>) -> NodeId {
        self.allocate(Node::Text(data.into()), None)
    }

    /// Creates a detached comment.
    pub fn create_comment(&mut self, data: impl Into<String>) -> NodeId {
        self.allocate(Node::Comment(data.into()), None)
    }

    /// Creates a detached document fragment.
    pub fn create_document_fragment(&mut self) -> NodeId {
        self.allocate(Node::DocumentFragment, None)
    }

    /// Parent handle, or `None` for detached/root nodes.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).and_then(|record| record.parent)
    }

    /// First child in tree order.
    #[must_use]
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.children.first().copied()
    }

    /// Last child in tree order.
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.children.last().copied()
    }

    /// Previous sibling in tree order.
    #[must_use]
    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let siblings = &self.node(parent)?.children;
        let index = siblings.iter().position(|candidate| *candidate == id)?;
        index.checked_sub(1).map(|previous| siblings[previous])
    }

    /// Next sibling in tree order.
    #[must_use]
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let siblings = &self.node(parent)?.children;
        let index = siblings.iter().position(|candidate| *candidate == id)?;
        siblings.get(index + 1).copied()
    }

    /// Appends a node, reparenting it when necessary. Appending a fragment
    /// transfers its children and leaves the fragment empty.
    ///
    /// # Errors
    /// Returns a hierarchy, missing-node, or invalid-parent error.
    pub fn append_child(
        &mut self,
        parent: NodeId,
        child: NodeId,
    ) -> Result<MutationBatch, DomError> {
        let index = self.record(parent)?.children.len();
        self.insert_at(parent, child, index)
    }

    /// Inserts before an existing child. A fragment transfers all children at
    /// the reference position.
    ///
    /// # Errors
    /// Returns an error when handles or hierarchy relationships are invalid.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        reference: NodeId,
    ) -> Result<MutationBatch, DomError> {
        let index = self
            .record(parent)?
            .children
            .iter()
            .position(|candidate| *candidate == reference)
            .ok_or(DomError::ReferenceNotChild { parent, reference })?;
        self.insert_at(parent, child, index)
    }

    /// Detaches a direct child while retaining its stable handle.
    ///
    /// # Errors
    /// Returns an error if either handle is invalid or the relationship does
    /// not exist.
    pub fn remove_child(
        &mut self,
        parent: NodeId,
        child: NodeId,
    ) -> Result<MutationBatch, DomError> {
        self.record(child)?;
        let index = self
            .record(parent)?
            .children
            .iter()
            .position(|candidate| *candidate == child)
            .ok_or(DomError::ReferenceNotChild {
                parent,
                reference: child,
            })?;
        self.nodes[parent.0].children.remove(index);
        self.nodes[child.0].parent = None;
        Ok(vec![MutationRecord::ChildRemoved {
            parent,
            node: child,
            index,
        }])
    }

    /// Detaches a node from its current parent.
    ///
    /// # Errors
    /// Returns an error for a missing or already detached node.
    pub fn detach(&mut self, node: NodeId) -> Result<MutationBatch, DomError> {
        let parent = self.parent(node).ok_or(DomError::HasNoParent(node))?;
        self.remove_child(parent, node)
    }

    /// Gets an element attribute using ASCII case-insensitive HTML names.
    ///
    /// # Errors
    /// Returns an error if the handle is missing or is not an element.
    pub fn attribute(&self, element: NodeId, name: &str) -> Result<Option<&str>, DomError> {
        let data = self.element(element)?;
        Ok(data
            .attributes
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| attribute.value.as_str()))
    }

    /// Inserts or replaces an element attribute.
    ///
    /// # Errors
    /// Returns an error if the handle is missing or is not an element.
    pub fn set_attribute(
        &mut self,
        element: NodeId,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<MutationBatch, DomError> {
        let name = name.into().to_ascii_lowercase();
        let value = value.into();
        let data = self.element_mut(element)?;
        let old_value = if let Some(attribute) = data
            .attributes
            .iter_mut()
            .find(|attribute| attribute.name == name)
        {
            Some(std::mem::replace(&mut attribute.value, value.clone()))
        } else {
            data.attributes.push(Attribute {
                name: name.clone(),
                value: value.clone(),
                source_span: None,
            });
            None
        };
        Ok(vec![MutationRecord::AttributeChanged {
            element,
            name,
            old_value,
            new_value: Some(value),
        }])
    }

    /// Removes an element attribute, returning no mutation when absent.
    ///
    /// # Errors
    /// Returns an error if the handle is missing or is not an element.
    pub fn remove_attribute(
        &mut self,
        element: NodeId,
        name: &str,
    ) -> Result<MutationBatch, DomError> {
        let data = self.element_mut(element)?;
        let Some(index) = data
            .attributes
            .iter()
            .position(|attribute| attribute.name.eq_ignore_ascii_case(name))
        else {
            return Ok(Vec::new());
        };
        let removed = data.attributes.remove(index);
        Ok(vec![MutationRecord::AttributeChanged {
            element,
            name: removed.name,
            old_value: Some(removed.value),
            new_value: None,
        }])
    }

    /// Text or comment character data.
    ///
    /// # Errors
    /// Returns an error for missing nodes or incompatible node types.
    pub fn character_data(&self, node: NodeId) -> Result<&str, DomError> {
        match &self.record(node)?.node {
            Node::Text(data) | Node::Comment(data) => Ok(data),
            _ => Err(DomError::NotCharacterData(node)),
        }
    }

    /// Replaces text or comment character data.
    ///
    /// # Errors
    /// Returns an error for missing nodes or incompatible node types.
    pub fn set_character_data(
        &mut self,
        node: NodeId,
        value: impl Into<String>,
    ) -> Result<MutationBatch, DomError> {
        let value = value.into();
        let record = self.record_mut(node)?;
        let (Node::Text(target) | Node::Comment(target)) = &mut record.node else {
            return Err(DomError::NotCharacterData(node));
        };
        let old_value = std::mem::replace(target, value.clone());
        Ok(vec![MutationRecord::CharacterDataChanged {
            node,
            old_value,
            new_value: value,
        }])
    }

    /// Concatenated text-node descendants in deterministic tree order.
    ///
    /// # Errors
    /// Returns an error when the root handle is missing.
    pub fn text_content(&self, root: NodeId) -> Result<String, DomError> {
        self.record(root)?;
        let mut text = String::new();
        for id in self.preorder(root)? {
            if let Node::Text(data) = &self.nodes[id.0].node {
                text.push_str(data);
            }
        }
        Ok(text)
    }

    /// Replaces all children with one text node when `value` is non-empty.
    /// Existing children become detached but retain stable handles.
    ///
    /// # Errors
    /// Returns an error for missing nodes or node types that cannot have
    /// children.
    pub fn set_text_content(
        &mut self,
        node: NodeId,
        value: impl Into<String>,
    ) -> Result<MutationBatch, DomError> {
        self.ensure_parent_type(node)?;
        let value = value.into();
        let existing = self.nodes[node.0].children.clone();
        let mut mutations = Vec::new();
        for child in existing {
            mutations.extend(self.remove_child(node, child)?);
        }
        if !value.is_empty() {
            let text = self.create_text(value);
            mutations.extend(self.append_child(node, text)?);
        }
        Ok(mutations)
    }

    /// Preorder traversal including `root`.
    ///
    /// # Errors
    /// Returns an error when `root` is missing.
    pub fn preorder(&self, root: NodeId) -> Result<Vec<NodeId>, DomError> {
        self.record(root)?;
        let mut result = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            result.push(id);
            stack.extend(self.nodes[id.0].children.iter().rev());
        }
        Ok(result)
    }

    /// Elements matching a normalized tag name in document preorder.
    #[must_use]
    pub fn elements_by_tag_name(&self, tag_name: &str) -> Vec<NodeId> {
        let normalized = tag_name.to_ascii_lowercase();
        self.query_elements(|element| normalized == "*" || element.tag_name == normalized)
    }

    /// First element whose `id` attribute exactly matches.
    #[must_use]
    pub fn element_by_id(&self, id: &str) -> Option<NodeId> {
        self.query_elements(|element| {
            element
                .attributes
                .iter()
                .any(|attribute| attribute.name == "id" && attribute.value == id)
        })
        .into_iter()
        .next()
    }

    /// Elements containing the requested whitespace-separated class token.
    #[must_use]
    pub fn elements_by_class_name(&self, class_name: &str) -> Vec<NodeId> {
        self.query_elements(|element| {
            element.attributes.iter().any(|attribute| {
                attribute.name == "class"
                    && attribute
                        .value
                        .split_ascii_whitespace()
                        .any(|token| token == class_name)
            })
        })
    }

    /// Checks bidirectional relationships, duplicate children, cycles, and
    /// child-capable node types.
    #[must_use]
    pub fn validate_invariants(&self) -> Vec<InvariantError> {
        let mut errors = Vec::new();
        for (index, record) in self.nodes.iter().enumerate() {
            let id = NodeId(index);
            if !can_have_children(&record.node) && !record.children.is_empty() {
                errors.push(InvariantError {
                    node: id,
                    message: "node type cannot contain children".into(),
                });
            }
            let mut unique = HashSet::new();
            for child in &record.children {
                if !unique.insert(*child) {
                    errors.push(InvariantError {
                        node: id,
                        message: format!("duplicate child {child:?}"),
                    });
                }
                match self.node(*child) {
                    Some(child_record) if child_record.parent == Some(id) => {}
                    Some(_) => errors.push(InvariantError {
                        node: *child,
                        message: format!("parent does not point back to {id:?}"),
                    }),
                    None => errors.push(InvariantError {
                        node: id,
                        message: format!("missing child {child:?}"),
                    }),
                }
            }
            if let Some(parent) = record.parent {
                if self
                    .node(parent)
                    .is_none_or(|parent_record| !parent_record.children.contains(&id))
                {
                    errors.push(InvariantError {
                        node: id,
                        message: "parent relationship is not bidirectional".into(),
                    });
                }
            }
            let mut ancestors = HashSet::new();
            let mut cursor = Some(id);
            while let Some(current) = cursor {
                if !ancestors.insert(current) {
                    errors.push(InvariantError {
                        node: id,
                        message: "parent cycle detected".into(),
                    });
                    break;
                }
                cursor = self.parent(current);
            }
        }
        errors
    }

    fn allocate(&mut self, node: Node, source_span: Option<SourceSpan>) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(NodeRecord {
            node,
            parent: None,
            children: Vec::new(),
            source_span,
        });
        id
    }

    fn record(&self, id: NodeId) -> Result<&NodeRecord, DomError> {
        self.node(id).ok_or(DomError::MissingNode(id))
    }

    fn record_mut(&mut self, id: NodeId) -> Result<&mut NodeRecord, DomError> {
        self.nodes.get_mut(id.0).ok_or(DomError::MissingNode(id))
    }

    fn element(&self, id: NodeId) -> Result<&ElementData, DomError> {
        match &self.record(id)?.node {
            Node::Element(element) => Ok(element),
            _ => Err(DomError::NotAnElement(id)),
        }
    }

    fn element_mut(&mut self, id: NodeId) -> Result<&mut ElementData, DomError> {
        match &mut self.record_mut(id)?.node {
            Node::Element(element) => Ok(element),
            _ => Err(DomError::NotAnElement(id)),
        }
    }

    fn ensure_parent_type(&self, id: NodeId) -> Result<(), DomError> {
        if can_have_children(&self.record(id)?.node) {
            Ok(())
        } else {
            Err(DomError::ParentCannotHaveChildren(id))
        }
    }

    fn insert_at(
        &mut self,
        parent: NodeId,
        child: NodeId,
        mut index: usize,
    ) -> Result<MutationBatch, DomError> {
        self.ensure_parent_type(parent)?;
        let child_record = self.record(child)?;
        if matches!(child_record.node, Node::Document | Node::Doctype(_)) {
            return Err(DomError::InvalidChildType);
        }
        if child == parent || self.is_ancestor(child, parent)? {
            return Err(DomError::HierarchyCycle {
                parent,
                node: child,
            });
        }
        if matches!(child_record.node, Node::DocumentFragment) {
            let fragment_children = child_record.children.clone();
            let mut records = Vec::new();
            for fragment_child in fragment_children {
                records.extend(self.insert_at(parent, fragment_child, index)?);
                index += 1;
            }
            return Ok(records);
        }

        let mut records = Vec::new();
        if let Some(old_parent) = self.nodes[child.0].parent {
            let old_index = self.nodes[old_parent.0]
                .children
                .iter()
                .position(|candidate| *candidate == child)
                .expect("DOM invariant guarantees child appears in parent");
            self.nodes[old_parent.0].children.remove(old_index);
            self.nodes[child.0].parent = None;
            records.push(MutationRecord::ChildRemoved {
                parent: old_parent,
                node: child,
                index: old_index,
            });
            if old_parent == parent && old_index < index {
                index -= 1;
            }
        }
        index = index.min(self.nodes[parent.0].children.len());
        self.nodes[parent.0].children.insert(index, child);
        self.nodes[child.0].parent = Some(parent);
        records.push(MutationRecord::ChildInserted {
            parent,
            node: child,
            index,
        });
        Ok(records)
    }

    fn is_ancestor(&self, possible_ancestor: NodeId, node: NodeId) -> Result<bool, DomError> {
        self.record(possible_ancestor)?;
        self.record(node)?;
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if current == possible_ancestor {
                return Ok(true);
            }
            cursor = self.parent(current);
        }
        Ok(false)
    }

    fn attach_unchecked(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent.0].children.push(child);
        self.nodes[child.0].parent = Some(parent);
    }

    fn query_elements(&self, predicate: impl Fn(&ElementData) -> bool) -> Vec<NodeId> {
        self.preorder(self.root)
            .unwrap_or_default()
            .into_iter()
            .filter(
                |id| matches!(&self.nodes[id.0].node, Node::Element(element) if predicate(element)),
            )
            .collect()
    }
}

fn can_have_children(node: &Node) -> bool {
    matches!(
        node,
        Node::Document | Node::DocumentFragment | Node::Element(_)
    )
}

#[cfg(test)]
mod tests {
    use super::{Document, DomError, MutationRecord, Node, NodeId};

    #[test]
    fn creates_document_and_detached_nodes_with_stable_ids() {
        let mut document = Document::new();
        let element = document.create_element("DIV");
        let text = document.create_text("hello");
        let comment = document.create_comment("note");
        assert_eq!(document.root(), NodeId(0));
        assert_eq!((element, text, comment), (NodeId(1), NodeId(2), NodeId(3)));
        assert!(
            matches!(&document.node(element).unwrap().node, Node::Element(data) if data.tag_name == "div")
        );
    }

    #[test]
    fn appends_inserts_traverses_and_removes() {
        let mut document = Document::new();
        let parent = document.create_element("div");
        let first = document.create_text("first");
        let second = document.create_text("second");
        document.append_child(document.root(), parent).unwrap();
        document.append_child(parent, second).unwrap();
        document.insert_before(parent, first, second).unwrap();
        assert_eq!(document.first_child(parent), Some(first));
        assert_eq!(document.last_child(parent), Some(second));
        assert_eq!(document.next_sibling(first), Some(second));
        assert_eq!(document.previous_sibling(second), Some(first));
        document.remove_child(parent, first).unwrap();
        assert_eq!(document.parent(first), None);
        assert_eq!(document.first_child(parent), Some(second));
    }

    #[test]
    fn reparents_and_reports_both_sides() {
        let mut document = Document::new();
        let left = document.create_element("div");
        let right = document.create_element("div");
        let child = document.create_element("span");
        document.append_child(left, child).unwrap();
        let records = document.append_child(right, child).unwrap();
        assert!(
            matches!(records[0], MutationRecord::ChildRemoved { parent, .. } if parent == left)
        );
        assert!(
            matches!(records[1], MutationRecord::ChildInserted { parent, .. } if parent == right)
        );
        assert_eq!(document.parent(child), Some(right));
    }

    #[test]
    fn fragment_transfers_children_and_remains_detached() {
        let mut document = Document::new();
        let parent = document.create_element("div");
        let fragment = document.create_document_fragment();
        let one = document.create_text("one");
        let two = document.create_text("two");
        document.append_child(fragment, one).unwrap();
        document.append_child(fragment, two).unwrap();
        document.append_child(parent, fragment).unwrap();
        assert_eq!(
            document.node(fragment).unwrap().children,
            Vec::<NodeId>::new()
        );
        assert_eq!(document.node(parent).unwrap().children, [one, two]);
    }

    #[test]
    fn rejects_invalid_operations_without_panicking() {
        let mut document = Document::new();
        let parent = document.create_element("div");
        let child = document.create_element("span");
        document.append_child(parent, child).unwrap();
        assert!(matches!(
            document.append_child(child, parent),
            Err(DomError::HierarchyCycle { .. })
        ));
        assert!(matches!(
            document.append_child(NodeId(999), child),
            Err(DomError::MissingNode(_))
        ));
        assert!(matches!(
            document.remove_child(parent, document.root()),
            Err(DomError::ReferenceNotChild { .. })
        ));
        assert!(matches!(document.set_attribute(child, "id", "x"), Ok(_)));
        assert!(matches!(
            document.set_attribute(document.root(), "id", "x"),
            Err(DomError::NotAnElement(_))
        ));
    }
}
