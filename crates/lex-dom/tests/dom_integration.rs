use lex_dom::{Document, DomError, MutationRecord, Node, NodeId};

const FIXTURE: &str = include_str!("fixtures/document.html");

#[test]
fn converts_m4_document_and_preserves_spans_and_queries() {
    let parsed = lex_html::parse(FIXTURE);
    let document = Document::from_parsed(&parsed).unwrap();
    let html = document.elements_by_tag_name("html");
    assert_eq!(html.len(), 1);
    let target = document.element_by_id("target").unwrap();
    assert_eq!(document.text_content(target).unwrap(), "world");
    assert_eq!(document.elements_by_class_name("readable").len(), 2);
    assert!(document.node(target).unwrap().source_span.is_some());
    assert!(document.validate_invariants().is_empty());
}

#[test]
fn attribute_and_character_data_mutations_are_explicit() {
    let mut document = Document::new();
    let element = document.create_element("div");
    let text = document.create_text("old");
    document.append_child(element, text).unwrap();

    let inserted = document.set_attribute(element, "ID", "first").unwrap();
    assert!(
        matches!(&inserted[0], MutationRecord::AttributeChanged { old_value: None, new_value: Some(value), .. } if value == "first")
    );
    let replaced = document.set_attribute(element, "id", "second").unwrap();
    assert!(
        matches!(&replaced[0], MutationRecord::AttributeChanged { old_value: Some(value), .. } if value == "first")
    );
    assert_eq!(document.attribute(element, "Id").unwrap(), Some("second"));
    assert_eq!(document.remove_attribute(element, "ID").unwrap().len(), 1);
    assert_eq!(document.attribute(element, "id").unwrap(), None);

    let changed = document.set_character_data(text, "new").unwrap();
    assert!(
        matches!(&changed[0], MutationRecord::CharacterDataChanged { old_value, new_value, .. } if old_value == "old" && new_value == "new")
    );
    assert_eq!(document.character_data(text).unwrap(), "new");
}

#[test]
fn set_text_content_detaches_old_nodes_without_invalidating_handles() {
    let mut document = Document::new();
    let element = document.create_element("p");
    let old = document.create_text("old");
    document.append_child(element, old).unwrap();
    document.set_text_content(element, "replacement").unwrap();
    assert_eq!(document.parent(old), None);
    assert!(matches!(&document.node(old).unwrap().node, Node::Text(value) if value == "old"));
    assert_eq!(document.text_content(element).unwrap(), "replacement");
}

#[test]
fn nested_preorder_is_deterministic() {
    let document =
        Document::from_html_bytes(b"<div><p>one</p><p>two<span>three</span></p></div>").unwrap();
    let tags: Vec<_> = document
        .preorder(document.root())
        .unwrap()
        .into_iter()
        .filter_map(|id| match &document.node(id).unwrap().node {
            Node::Element(element) => Some(element.tag_name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(tags, ["div", "p", "p", "span"]);
}

#[test]
fn large_dom_keeps_handles_and_invariants_stable() {
    let mut document = Document::new();
    let root = document.create_element("main");
    document.append_child(document.root(), root).unwrap();
    let mut handles = Vec::new();
    for index in 0..20_000 {
        let element = document.create_element("div");
        document
            .set_attribute(element, "data-index", index.to_string())
            .unwrap();
        document.append_child(root, element).unwrap();
        handles.push(element);
    }
    assert_eq!(handles[0], NodeId(2));
    assert_eq!(handles[19_999], NodeId(20_001));
    assert_eq!(document.node_count(), 20_002);
    assert!(document.validate_invariants().is_empty());
}

#[test]
fn normal_invalid_calls_return_errors() {
    let mut document = Document::new();
    let text = document.create_text("x");
    let fragment = document.create_document_fragment();
    assert!(matches!(
        document.append_child(text, fragment),
        Err(DomError::ParentCannotHaveChildren(_))
    ));
    assert!(matches!(
        document.detach(fragment),
        Err(DomError::HasNoParent(_))
    ));
    assert!(matches!(
        document.character_data(fragment),
        Err(DomError::NotCharacterData(_))
    ));
    assert!(matches!(
        document.preorder(NodeId(usize::MAX)),
        Err(DomError::MissingNode(_))
    ));
}
