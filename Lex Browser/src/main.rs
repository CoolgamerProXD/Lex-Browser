use lex_css::{compute_style, parse};
use lex_dom::Node;

const HTML: &str = "<!doctype html><html><body><main><h1 id='title' class='hero'>Lex Browser</h1><p>HTML, DOM, and CSS are connected.</p></main></body></html>";
const CSS: &str = "body { color: #20242a; } main > .hero { color: #2457d6; font-size: 32px; } p { display: block; }";

fn main() {
    let document = lex_dom::Document::from_html_bytes(HTML.as_bytes())
        .expect("the built-in demo HTML must produce a valid DOM");
    let stylesheet = parse(CSS);
    println!("Lex Browser — M6 pipeline demo");
    println!("HTML → DOM: {} nodes", document.node_count());
    println!(
        "CSS: {} rules, {} recoverable errors",
        stylesheet.rules.len(),
        stylesheet.errors.len()
    );
    println!("Computed styles:");
    for id in document
        .preorder(document.root())
        .expect("document root must exist")
    {
        if let Some(record) = document.node(id) {
            if let Node::Element(element) = &record.node {
                let style = compute_style(&document, id, std::slice::from_ref(&stylesheet));
                if !style.is_empty() {
                    println!("  <{}> {style:?}", element.tag_name);
                }
            }
        }
    }
    println!("Pipeline verified. Layout and page painting begin in the next milestones.");
}
