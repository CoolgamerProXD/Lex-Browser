use lex_css::{compute_style, parse, tokenize, CssValue, Specificity};
use lex_dom::Document;

const HTML: &[u8] = include_bytes!("fixtures/article.html");
const CSS: &str = include_str!("fixtures/article.css");

#[test]
fn fixture_flows_from_html_through_dom_and_style() {
    let document = Document::from_html_bytes(HTML).unwrap();
    let stylesheet = parse(CSS);
    assert_eq!(stylesheet.errors, []);
    let title = document.element_by_id("title").unwrap();
    let style = compute_style(&document, title, &[stylesheet]);
    assert_eq!(
        style["font-size"],
        CssValue::Length(24.0, lex_css::LengthUnit::Px)
    );
    assert_eq!(style["display"], CssValue::Keyword("block".into()));
}

#[test]
fn source_order_specificity_and_importance_are_deterministic() {
    let document = Document::from_html_bytes(HTML).unwrap();
    let title = document.element_by_id("title").unwrap();
    let sheet = parse("#title { color: red } .title { color: black !important } .title { display:block } .title { display:inline }");
    assert_eq!(
        sheet.rules[0].selectors[0].specificity,
        Specificity(1, 0, 0)
    );
    let style = compute_style(&document, title, &[sheet]);
    assert_eq!(
        style["color"],
        CssValue::Color(lex_css::Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255
        })
    );
    assert_eq!(style["display"], CssValue::Keyword("inline".into()));
}

#[test]
fn malformed_fixture_recovers_without_panicking() {
    let sheet = parse("h1 { color red; width: 10px; } [ { bad:yes } p { display:block }");
    assert_ne!(sheet.errors, []);
    assert!(sheet
        .rules
        .iter()
        .any(|rule| rule.declarations.iter().any(|d| d.name == "display")));
    assert!(!tokenize("/* unfinished").iter().any(|_| false));
}
