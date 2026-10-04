use lex_css::{parse, Color};
use lex_style::StyleEngine;

const HTML: &str = "<!doctype html><html><body><main id='content'><h1 id='title' class='hero'>Lex Browser</h1><p id='summary'>CSS inheritance now produces layout-ready styles.</p></main></body></html>";
const CSS: &str = "body { color: #20242a; } #content { color: #2457d6; font-family: 'Lex Sans', sans-serif; } main > .hero { color: #111111; font-size: 32px; } #summary { width: 75%; }";

fn main() {
    let document = lex_dom::Document::from_html_bytes(HTML.as_bytes())
        .expect("the built-in demo HTML must produce a valid DOM");
    let stylesheet = parse(CSS);
    let rule_count = stylesheet.rules.len();
    let error_count = stylesheet.errors.len();
    let mut styles = StyleEngine::new(vec![stylesheet]);
    styles.recompute(&document);

    let main = document
        .element_by_id("content")
        .expect("demo main element must exist");
    let heading = document
        .element_by_id("title")
        .expect("demo heading must exist");
    let paragraph = document
        .element_by_id("summary")
        .expect("demo paragraph must exist");
    let main_style = styles
        .computed_style(main)
        .expect("main must have computed style");
    let heading_style = styles
        .computed_style(heading)
        .expect("heading must have computed style");
    let paragraph_style = styles
        .computed_style(paragraph)
        .expect("paragraph must have computed style");

    println!("Lex Browser — M7 style-system demo");
    println!("HTML → DOM: {} nodes", document.node_count());
    println!("CSS: {rule_count} rules, {error_count} recoverable errors");
    println!("Selector matching → cascade → inheritance → computed styles:");
    println!(
        "  <main> color={} font-family={:?}",
        format_color(main_style.color),
        main_style.font_family
    );
    println!(
        "  <h1> color={} font-size={}px display={:?}",
        format_color(heading_style.color),
        heading_style.font_size,
        heading_style.display
    );
    println!(
        "  <p> color={} (inherited from <main>) width={:?}",
        format_color(paragraph_style.color),
        paragraph_style.width
    );
    assert_eq!(paragraph_style.color, main_style.color);
    println!("Pipeline verified. M8 layout can consume these computed styles.");
}

fn format_color(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}
