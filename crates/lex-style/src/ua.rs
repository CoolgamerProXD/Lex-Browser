use crate::{
    ComputedStyle, Display, Edges, FontWeight, LengthPercentage, LengthPercentageAuto, LINK_BLUE,
};

/// Applies Lex's intentionally small HTML user-agent defaults that affect
/// display and inherited/text properties. Author declarations are applied
/// afterward and always win.
pub(crate) fn apply_primary_defaults(style: &mut ComputedStyle, tag_name: &str) {
    match tag_name {
        "html" | "body" | "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "ul" | "ol" => style.display = Display::Block,
        "li" => style.display = Display::ListItem,
        "span" | "strong" | "em" | "a" => style.display = Display::Inline,
        _ => {}
    }

    match tag_name {
        "h1" => {
            style.font_size *= 2.0;
            style.font_weight = FontWeight::Bold;
        }
        "h2" => {
            style.font_size *= 1.5;
            style.font_weight = FontWeight::Bold;
        }
        "h3" => {
            style.font_size *= 1.17;
            style.font_weight = FontWeight::Bold;
        }
        "h4" => style.font_weight = FontWeight::Bold,
        "h5" => {
            style.font_size *= 0.83;
            style.font_weight = FontWeight::Bold;
        }
        "h6" => {
            style.font_size *= 0.67;
            style.font_weight = FontWeight::Bold;
        }
        "strong" => style.font_weight = FontWeight::Bold,
        "a" => style.color = LINK_BLUE,
        _ => {}
    }
}

/// Applies UA box defaults after the final author font size is known, allowing
/// the documented `em`-like paragraph/list/heading margins to scale without
/// resolving any containing-block percentages.
pub(crate) fn apply_box_defaults(style: &mut ComputedStyle, tag_name: &str) {
    let px = |value| LengthPercentageAuto::Length(LengthPercentage::Px(value));
    match tag_name {
        "body" => style.margin = Edges::all(px(8.0)),
        "p" => {
            style.margin.top = px(style.font_size);
            style.margin.bottom = px(style.font_size);
        }
        "h1" => vertical_margin(style, 0.67),
        "h2" => vertical_margin(style, 0.83),
        "h3" => vertical_margin(style, 1.0),
        "h4" => vertical_margin(style, 1.33),
        "h5" => vertical_margin(style, 1.67),
        "h6" => vertical_margin(style, 2.33),
        "ul" | "ol" => {
            style.margin.top = px(style.font_size);
            style.margin.bottom = px(style.font_size);
            style.padding.left = LengthPercentage::Px(40.0);
        }
        _ => {}
    }
}

fn vertical_margin(style: &mut ComputedStyle, factor: f32) {
    let value = LengthPercentageAuto::Length(LengthPercentage::Px(style.font_size * factor));
    style.margin.top = value;
    style.margin.bottom = value;
}
