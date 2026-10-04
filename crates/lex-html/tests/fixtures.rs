use lex_html::{parse, parse_bytes, Node, Tokenizer};

const BASIC: &str = include_str!("fixtures/basic.html");

#[test]
fn representative_fixture_builds_expected_tree() {
    let document = parse(BASIC);
    assert!(document.first_element("html").is_some());
    assert!(document.first_element("head").is_some());
    assert!(document.first_element("body").is_some());
    let title = document.first_element("title").unwrap();
    assert_eq!(document.text_content(title), "Lex & the Web");
    assert!(document
        .nodes()
        .iter()
        .any(|record| matches!(&record.node, Node::Doctype(name) if name == "html")));
}

#[test]
fn large_input_completes_and_preserves_content() {
    let mut source = String::from("<html><body>");
    for index in 0..10_000 {
        source.push_str(&format!("<p id=p{index}>row {index}</p>"));
    }
    source.push_str("</body></html>");
    let document = parse(&source);
    assert!(document.nodes().len() >= 20_003);
    assert!(document.text_content(document.root()).contains("row 9999"));
}

#[test]
fn arbitrary_truncation_points_do_not_panic() {
    let source = "<!doctype html><!-- comment --><html><body><div a='b'>&amp;<script>x<y</script></div></body></html>";
    for end in source.char_indices().map(|(offset, _)| offset).chain([source.len()]) {
        let _ = parse(&source[..end]);
        let _ = Tokenizer::new(&source[..end]).tokenize();
    }
}

#[test]
fn invalid_utf8_is_replaced_without_panicking() {
    let document = parse_bytes(b"<p>before\xffafter</p>");
    let paragraph = document.first_element("p").unwrap();
    assert_eq!(document.text_content(paragraph), "before\u{fffd}after");
}
