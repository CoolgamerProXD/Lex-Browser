use lex_css::{parse, Color};
use lex_dom::Document;
use lex_style::{
    BorderStyle, Display, Edges, FontWeight, InvalidationReason, LengthPercentage,
    LengthPercentageAuto, LineHeight, Property, Size, SpecifiedValue, StyleEngine, TextAlign,
    Visibility, BLACK, LINK_BLUE, TRANSPARENT,
};

fn styled(html: &str, css: &str) -> (Document, StyleEngine) {
    let document = Document::from_html_bytes(html.as_bytes()).unwrap();
    let mut engine = StyleEngine::new(vec![parse(css)]);
    engine.recompute(&document);
    (document, engine)
}

fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color {
        red,
        green,
        blue,
        alpha: 255,
    }
}

#[test]
fn inherited_properties_flow_through_nested_elements_and_can_be_overridden() {
    let (document, engine) = styled(
        "<div id='parent'><section><span id='child'>text</span><em id='override'>x</em></section></div>",
        "#parent { color: #123456; font-family: 'Lex Sans'; font-size: 20px; font-weight: 600; line-height: 1.5; text-align: center; visibility: hidden } #override { color: red; visibility: visible }",
    );
    let child = engine
        .computed_style(document.element_by_id("child").unwrap())
        .unwrap();
    assert_eq!(child.color, rgb(0x12, 0x34, 0x56));
    assert_eq!(child.font_family, ["Lex Sans"]);
    assert_eq!(child.font_size, 20.0);
    assert_eq!(child.font_weight, FontWeight::Number(600));
    assert_eq!(child.line_height, LineHeight::Number(1.5));
    assert_eq!(child.text_align, TextAlign::Center);
    assert_eq!(child.visibility, Visibility::Hidden);

    let overridden = engine
        .computed_style(document.element_by_id("override").unwrap())
        .unwrap();
    assert_eq!(overridden.color, rgb(255, 0, 0));
    assert_eq!(overridden.visibility, Visibility::Visible);
    assert_eq!(overridden.font_size, 20.0);
}

#[test]
fn non_inherited_properties_use_initial_values_on_children() {
    let (document, engine) = styled(
        "<div id='parent'><span id='child'>text</span></div>",
        "#parent { background-color: #abcdef; width: 90px; margin: 12px; padding: 8px; opacity: .4; border-width: 2px; border-style: solid }",
    );
    let child = engine
        .computed_style(document.element_by_id("child").unwrap())
        .unwrap();
    assert_eq!(child.background_color, TRANSPARENT);
    assert_eq!(child.width, Size::Auto);
    assert_eq!(
        child.margin,
        Edges::all(LengthPercentageAuto::Length(LengthPercentage::Px(0.0)))
    );
    assert_eq!(child.padding, Edges::all(LengthPercentage::Px(0.0)));
    assert_eq!(child.opacity, 1.0);
    assert_eq!(child.border_width, Edges::all(0.0));
    assert_eq!(child.border_style, Edges::all(BorderStyle::None));
}

#[test]
fn inherit_initial_and_unset_have_property_appropriate_behavior() {
    let (document, engine) = styled(
        "<div id='parent'><span id='inherit'></span><span id='initial'></span><span id='unset'></span></div>",
        "#parent { color: red; background-color: #112233; width: 80px; font-size: 20px } #inherit { color: inherit; background-color: inherit; width: inherit } #initial { color: initial; font-size: initial } #unset { color: unset; width: unset; background-color: unset }",
    );
    let inherited = engine
        .computed_style(document.element_by_id("inherit").unwrap())
        .unwrap();
    assert_eq!(inherited.color, rgb(255, 0, 0));
    assert_eq!(inherited.background_color, rgb(0x11, 0x22, 0x33));
    assert_eq!(
        inherited.width,
        Size::Length(LengthPercentage::Px(80.0))
    );

    let initial = engine
        .computed_style(document.element_by_id("initial").unwrap())
        .unwrap();
    assert_eq!(initial.color, BLACK);
    assert_eq!(initial.font_size, 16.0);

    let unset = engine
        .computed_style(document.element_by_id("unset").unwrap())
        .unwrap();
    assert_eq!(unset.color, rgb(255, 0, 0));
    assert_eq!(unset.width, Size::Auto);
    assert_eq!(unset.background_color, TRANSPARENT);
}

#[test]
fn cascade_respects_specificity_source_order_importance_and_combinators() {
    let (document, engine) = styled(
        "<main><div class='card'><p id='target' class='lead'>x</p></div></main>",
        "p { color: black; width: 1px } main .lead { color: red } main > p { height: 99px } .card > p { height: 12px } #target { width: 2px } .lead { width: 3px !important; color: #111111 } .lead { color: #222222 }",
    );
    let target = engine
        .computed_style(document.element_by_id("target").unwrap())
        .unwrap();
    assert_eq!(target.color, rgb(255, 0, 0));
    assert_eq!(target.width, Size::Length(LengthPercentage::Px(3.0)));
    assert_eq!(target.height, Size::Length(LengthPercentage::Px(12.0)));
}

#[test]
fn inline_styles_join_the_author_cascade_without_bypassing_important() {
    let (document, engine) = styled(
        "<p id='normal' style='color: blue'></p><p id='important' style='color: blue !important'></p>",
        "#normal, #important { color: red !important }",
    );
    assert_eq!(
        engine
            .computed_style(document.element_by_id("normal").unwrap())
            .unwrap()
            .color,
        rgb(255, 0, 0)
    );
    assert_eq!(
        engine
            .computed_style(document.element_by_id("important").unwrap())
            .unwrap()
            .color,
        rgb(0, 0, 255)
    );
}

#[test]
fn html_defaults_cover_blocks_headings_lists_and_inline_elements() {
    let (document, engine) = styled(
        "<html><body id='body'><p id='p'></p><h1 id='h1'></h1><h6 id='h6'></h6><ul id='ul'><li id='li'></li></ul><span id='span'></span><strong id='strong'></strong><em id='em'></em><a id='a'></a><div id='div'></div></body></html>",
        "",
    );
    let style = |id| {
        engine
            .computed_style(document.element_by_id(id).unwrap())
            .unwrap()
    };
    assert_eq!(style("body").display, Display::Block);
    assert_eq!(style("body").margin.top, px_auto(8.0));
    assert_eq!(style("p").display, Display::Block);
    assert_eq!(style("p").margin.top, px_auto(16.0));
    assert_eq!(style("h1").font_size, 32.0);
    assert_eq!(style("h1").font_weight, FontWeight::Bold);
    assert_eq!(style("h6").font_size, 10.72);
    assert_eq!(style("ul").padding.left, LengthPercentage::Px(40.0));
    assert_eq!(style("li").display, Display::ListItem);
    assert_eq!(style("span").display, Display::Inline);
    assert_eq!(style("strong").font_weight, FontWeight::Bold);
    assert_eq!(style("em").display, Display::Inline);
    assert_eq!(style("a").color, LINK_BLUE);
    assert_eq!(style("div").display, Display::Block);
}

#[test]
fn typed_values_preserve_layout_dependent_percentages_and_compute_relative_fonts() {
    let (document, engine) = styled(
        "<div id='parent'><div id='box'></div></div>",
        "#parent { font-size: 20px } #box { font-size: 150%; width: 50%; height: 25vh; margin: 1px 2% 3px auto; padding: 1em 4px; border-width: thin 2px thick 0; border-style: solid dashed; border-color: red #00ff00; opacity: 150%; line-height: 120%; font-family: 'Lex Sans', serif }",
    );
    let style = engine
        .computed_style(document.element_by_id("box").unwrap())
        .unwrap();
    assert_eq!(style.font_size, 30.0);
    assert_eq!(style.width, Size::Length(LengthPercentage::Percentage(50.0)));
    assert_eq!(
        style.height,
        Size::Length(LengthPercentage::ViewportHeight(25.0))
    );
    assert_eq!(style.margin.top, px_auto(1.0));
    assert_eq!(
        style.margin.right,
        LengthPercentageAuto::Length(LengthPercentage::Percentage(2.0))
    );
    assert_eq!(style.margin.left, LengthPercentageAuto::Auto);
    assert_eq!(style.padding.top, LengthPercentage::Px(30.0));
    assert_eq!(style.border_width.top, 1.0);
    assert_eq!(style.border_width.bottom, 5.0);
    assert_eq!(style.border_style.right, BorderStyle::Dashed);
    assert_eq!(style.border_color.top, rgb(255, 0, 0));
    assert_eq!(style.border_color.right, rgb(0, 255, 0));
    assert_eq!(style.opacity, 1.0);
    assert_eq!(style.line_height, LineHeight::Px(36.0));
    assert_eq!(style.font_family, ["Lex Sans", "serif"]);
}

#[test]
fn malformed_and_unsupported_declarations_are_ignored_before_winner_selection() {
    let (document, engine) = styled(
        "<div id='target'></div>",
        "div { width: 25px; color: red } #target { width: nonsense; color: #zzzzzz; made-up-property: 10px; padding: auto }",
    );
    let target = document.element_by_id("target").unwrap();
    let style = engine.computed_style(target).unwrap();
    assert_eq!(style.width, Size::Length(LengthPercentage::Px(25.0)));
    assert_eq!(style.color, rgb(255, 0, 0));
    assert_eq!(style.padding, Edges::all(LengthPercentage::Px(0.0)));
    assert_eq!(
        engine.specified_style(target).unwrap().get(Property::Width),
        Some(&SpecifiedValue::Value(lex_css::CssValue::Length(
            25.0,
            lex_css::LengthUnit::Px
        )))
    );
}

#[test]
fn author_styles_override_html_defaults() {
    let (document, engine) = styled(
        "<body id='body'><p id='p'></p><h1 id='h1'></h1><a id='a'></a></body>",
        "#body { margin: 0 } #p { display: inline; margin: 2px } #h1 { font-size: 18px; font-weight: normal } #a { color: green }",
    );
    let style = |id| {
        engine
            .computed_style(document.element_by_id(id).unwrap())
            .unwrap()
    };
    assert_eq!(style("body").margin, Edges::all(px_auto(0.0)));
    assert_eq!(style("p").display, Display::Inline);
    assert_eq!(style("p").margin, Edges::all(px_auto(2.0)));
    assert_eq!(style("h1").font_size, 18.0);
    assert_eq!(style("h1").font_weight, FontWeight::Normal);
    assert_eq!(style("a").color, rgb(0, 128, 0));
}

#[test]
fn class_id_and_attribute_mutations_trigger_recomputation() {
    let mut document = Document::from_html_bytes(b"<div id='target'></div>").unwrap();
    let target = document.element_by_id("target").unwrap();
    let mut engine = StyleEngine::new(vec![parse(
        ".active { color: red } #renamed { width: 20px } [data-ready=yes] { opacity: .5 }",
    )]);
    engine.recompute(&document);
    let generation = engine.generation();

    let records = document.set_attribute(target, "class", "active").unwrap();
    let invalidations = engine.note_mutations(&records);
    assert_eq!(invalidations[0].reason, InvalidationReason::ClassChanged);
    assert!(engine.recompute_if_needed(&document));
    assert_eq!(engine.generation(), generation + 1);
    assert_eq!(engine.computed_style(target).unwrap().color, rgb(255, 0, 0));

    let records = document.set_attribute(target, "id", "renamed").unwrap();
    assert_eq!(
        engine.note_mutations(&records)[0].reason,
        InvalidationReason::IdChanged
    );
    engine.recompute(&document);
    assert_eq!(
        engine.computed_style(target).unwrap().width,
        Size::Length(LengthPercentage::Px(20.0))
    );

    let records = document
        .set_attribute(target, "data-ready", "yes")
        .unwrap();
    assert_eq!(
        engine.note_mutations(&records)[0].reason,
        InvalidationReason::AttributeChanged
    );
    engine.recompute(&document);
    assert_eq!(engine.computed_style(target).unwrap().opacity, 0.5);
    assert!(!engine.recompute_if_needed(&document));
}

#[test]
fn insertion_removal_and_stylesheet_changes_refresh_the_snapshot() {
    let mut document = Document::new();
    let parent = document.create_element("div");
    document.append_child(document.root(), parent).unwrap();
    let mut engine = StyleEngine::new(vec![parse("span { color: red }")]);
    engine.recompute(&document);

    let child = document.create_element("span");
    let records = document.append_child(parent, child).unwrap();
    assert_eq!(
        engine.note_mutations(&records)[0].reason,
        InvalidationReason::NodeInserted
    );
    engine.recompute(&document);
    assert_eq!(engine.computed_style(child).unwrap().color, rgb(255, 0, 0));

    let records = document.remove_child(parent, child).unwrap();
    assert_eq!(
        engine.note_mutations(&records)[0].reason,
        InvalidationReason::NodeRemoved
    );
    engine.recompute(&document);
    assert!(engine.computed_style(child).is_none());

    let invalidation = engine.replace_stylesheets(vec![parse("div { color: blue }")]);
    assert_eq!(
        invalidation.reason,
        InvalidationReason::StylesheetChanged
    );
    engine.recompute(&document);
    assert_eq!(engine.computed_style(parent).unwrap().color, rgb(0, 0, 255));
}

#[test]
fn character_data_does_not_dirty_current_selector_model() {
    let mut document = Document::new();
    let parent = document.create_element("p");
    let text = document.create_text("before");
    document.append_child(document.root(), parent).unwrap();
    document.append_child(parent, text).unwrap();
    let mut engine = StyleEngine::default();
    engine.recompute(&document);
    let records = document.set_character_data(text, "after").unwrap();
    assert_eq!(engine.note_mutations(&records), []);
    assert!(!engine.is_dirty());
}

fn px_auto(value: f32) -> LengthPercentageAuto {
    LengthPercentageAuto::Length(LengthPercentage::Px(value))
}
