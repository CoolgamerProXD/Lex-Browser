use std::collections::BTreeMap;

use lex_css::{tokenize, Color, CssValue, LengthUnit, TokenKind};

/// A property implemented by the M7 computed-style model.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Property {
    Display,
    Color,
    BackgroundColor,
    Width,
    Height,
    Margin,
    Padding,
    BorderWidth,
    BorderStyle,
    BorderColor,
    FontSize,
    FontFamily,
    FontWeight,
    LineHeight,
    TextAlign,
    Visibility,
    Opacity,
}

impl Property {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "display" => Some(Self::Display),
            "color" => Some(Self::Color),
            "background-color" => Some(Self::BackgroundColor),
            "width" => Some(Self::Width),
            "height" => Some(Self::Height),
            "margin" => Some(Self::Margin),
            "padding" => Some(Self::Padding),
            "border-width" => Some(Self::BorderWidth),
            "border-style" => Some(Self::BorderStyle),
            "border-color" => Some(Self::BorderColor),
            "font-size" => Some(Self::FontSize),
            "font-family" => Some(Self::FontFamily),
            "font-weight" => Some(Self::FontWeight),
            "line-height" => Some(Self::LineHeight),
            "text-align" => Some(Self::TextAlign),
            "visibility" => Some(Self::Visibility),
            "opacity" => Some(Self::Opacity),
            _ => None,
        }
    }

    /// Whether the property inherits when no declaration wins, and when its
    /// specified value is `unset`.
    #[must_use]
    pub const fn is_inherited(self) -> bool {
        matches!(
            self,
            Self::Color
                | Self::FontSize
                | Self::FontFamily
                | Self::FontWeight
                | Self::LineHeight
                | Self::TextAlign
                | Self::Visibility
        )
    }
}

/// A CSS-wide keyword or a property value retained after the cascade.
#[derive(Clone, Debug, PartialEq)]
pub enum SpecifiedValue {
    Value(CssValue),
    Inherit,
    Initial,
    Unset,
}

impl SpecifiedValue {
    #[must_use]
    pub fn from_css(value: &CssValue) -> Self {
        match value {
            CssValue::Keyword(keyword) if keyword == "inherit" => Self::Inherit,
            CssValue::Keyword(keyword) if keyword == "initial" => Self::Initial,
            CssValue::Keyword(keyword) if keyword == "unset" => Self::Unset,
            _ => Self::Value(value.clone()),
        }
    }
}

/// Winning supported specified values. Unsupported and property-invalid
/// declarations never enter this map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecifiedStyle {
    values: BTreeMap<Property, SpecifiedValue>,
}

impl SpecifiedStyle {
    pub(crate) fn insert(&mut self, property: Property, value: SpecifiedValue) {
        self.values.insert(property, value);
    }

    #[must_use]
    pub fn get(&self, property: Property) -> Option<&SpecifiedValue> {
        self.values.get(&property)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Property, &SpecifiedValue)> {
        self.values
            .iter()
            .map(|(property, value)| (*property, value))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Display {
    None,
    Block,
    Inline,
    InlineBlock,
    ListItem,
}

/// A computed length which M8 can consume. Percentages and viewport-relative
/// lengths stay unresolved because M7 has no containing block or viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthPercentage {
    Px(f32),
    Percentage(f32),
    ViewportWidth(f32),
    ViewportHeight(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Auto,
    Length(LengthPercentage),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthPercentageAuto {
    Auto,
    Length(LengthPercentage),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Edges<T> {
    #[must_use]
    pub const fn all(value: T) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    fn from_components(values: &[T]) -> Option<Self> {
        match values {
            [all] => Some(Self::all(*all)),
            [vertical, horizontal] => Some(Self {
                top: *vertical,
                right: *horizontal,
                bottom: *vertical,
                left: *horizontal,
            }),
            [top, horizontal, bottom] => Some(Self {
                top: *top,
                right: *horizontal,
                bottom: *bottom,
                left: *horizontal,
            }),
            [top, right, bottom, left] => Some(Self {
                top: *top,
                right: *right,
                bottom: *bottom,
                left: *left,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorderStyle {
    None,
    Hidden,
    Dotted,
    Dashed,
    Solid,
    Double,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontWeight {
    Normal,
    Bold,
    Number(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    Normal,
    Number(f32),
    Px(f32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAlign {
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

/// Complete, typed style data consumed by future layout. Every field has a
/// centrally defined deterministic initial value.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: Color,
    pub background_color: Color,
    pub width: Size,
    pub height: Size,
    pub margin: Edges<LengthPercentageAuto>,
    pub padding: Edges<LengthPercentage>,
    pub border_width: Edges<f32>,
    pub border_style: Edges<BorderStyle>,
    pub border_color: Edges<Color>,
    pub font_size: f32,
    pub font_family: Vec<String>,
    pub font_weight: FontWeight,
    pub line_height: LineHeight,
    pub text_align: TextAlign,
    pub visibility: Visibility,
    pub opacity: f32,
}

pub const BLACK: Color = Color {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 255,
};
pub const TRANSPARENT: Color = Color {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 0,
};
pub const LINK_BLUE: Color = Color {
    red: 0,
    green: 0,
    blue: 238,
    alpha: 255,
};

impl ComputedStyle {
    /// CSS initial values for Lex's supported subset. HTML element defaults are
    /// a separate, lower-priority user-agent layer.
    #[must_use]
    pub fn initial_values() -> Self {
        Self {
            display: Display::Inline,
            color: BLACK,
            background_color: TRANSPARENT,
            width: Size::Auto,
            height: Size::Auto,
            margin: Edges::all(LengthPercentageAuto::Length(LengthPercentage::Px(0.0))),
            padding: Edges::all(LengthPercentage::Px(0.0)),
            border_width: Edges::all(0.0),
            border_style: Edges::all(BorderStyle::None),
            border_color: Edges::all(BLACK),
            font_size: 16.0,
            font_family: vec!["sans-serif".into()],
            font_weight: FontWeight::Normal,
            line_height: LineHeight::Normal,
            text_align: TextAlign::Start,
            visibility: Visibility::Visible,
            opacity: 1.0,
        }
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self::initial_values()
    }
}

pub(crate) struct ComputeContext {
    pub parent_font: f32,
    pub own_font: f32,
    pub root_font: f32,
}

/// Checks property grammar before cascade winner selection. This preserves the
/// CSS rule that an invalid high-specificity declaration is ignored rather
/// than masking an earlier valid declaration.
pub(crate) fn valid_specified(property: Property, specified: &SpecifiedValue) -> bool {
    let SpecifiedValue::Value(value) = specified else {
        return true;
    };
    let context = ComputeContext {
        parent_font: 16.0,
        own_font: 16.0,
        root_font: 16.0,
    };
    match property {
        Property::Display => display(value).is_some(),
        Property::Color | Property::BackgroundColor => color(value).is_some(),
        Property::Width | Property::Height => size(value, &context).is_some(),
        Property::Margin => margin(value, &context).is_some(),
        Property::Padding => padding(value, &context).is_some(),
        Property::BorderWidth => border_width(value, &context).is_some(),
        Property::BorderStyle => border_style(value).is_some(),
        Property::BorderColor => border_color(value).is_some(),
        Property::FontSize => font_size(value, &context).is_some(),
        Property::FontFamily => font_family(value).is_some(),
        Property::FontWeight => font_weight(value).is_some(),
        Property::LineHeight => line_height(value, &context).is_some(),
        Property::TextAlign => text_align(value).is_some(),
        Property::Visibility => visibility(value).is_some(),
        Property::Opacity => opacity(value).is_some(),
    }
}

pub(crate) fn apply_value(
    style: &mut ComputedStyle,
    property: Property,
    value: &CssValue,
    context: &ComputeContext,
) {
    match property {
        Property::Display => style.display = display(value).expect("validated display"),
        Property::Color => style.color = color(value).expect("validated color"),
        Property::BackgroundColor => {
            style.background_color = color(value).expect("validated background color");
        }
        Property::Width => style.width = size(value, context).expect("validated width"),
        Property::Height => style.height = size(value, context).expect("validated height"),
        Property::Margin => style.margin = margin(value, context).expect("validated margin"),
        Property::Padding => style.padding = padding(value, context).expect("validated padding"),
        Property::BorderWidth => {
            style.border_width = border_width(value, context).expect("validated border width");
        }
        Property::BorderStyle => {
            style.border_style = border_style(value).expect("validated border style");
        }
        Property::BorderColor => {
            style.border_color = border_color(value).expect("validated border color");
        }
        Property::FontSize => {
            style.font_size = font_size(value, context).expect("validated font size");
        }
        Property::FontFamily => {
            style.font_family = font_family(value).expect("validated font family");
        }
        Property::FontWeight => {
            style.font_weight = font_weight(value).expect("validated font weight");
        }
        Property::LineHeight => {
            style.line_height = line_height(value, context).expect("validated line height");
        }
        Property::TextAlign => {
            style.text_align = text_align(value).expect("validated text align");
        }
        Property::Visibility => {
            style.visibility = visibility(value).expect("validated visibility");
        }
        Property::Opacity => style.opacity = opacity(value).expect("validated opacity"),
    }
}

pub(crate) fn copy_property(
    target: &mut ComputedStyle,
    source: &ComputedStyle,
    property: Property,
) {
    match property {
        Property::Display => target.display = source.display,
        Property::Color => target.color = source.color,
        Property::BackgroundColor => target.background_color = source.background_color,
        Property::Width => target.width = source.width,
        Property::Height => target.height = source.height,
        Property::Margin => target.margin = source.margin,
        Property::Padding => target.padding = source.padding,
        Property::BorderWidth => target.border_width = source.border_width,
        Property::BorderStyle => target.border_style = source.border_style,
        Property::BorderColor => target.border_color = source.border_color,
        Property::FontSize => target.font_size = source.font_size,
        Property::FontFamily => target.font_family.clone_from(&source.font_family),
        Property::FontWeight => target.font_weight = source.font_weight,
        Property::LineHeight => target.line_height = source.line_height,
        Property::TextAlign => target.text_align = source.text_align,
        Property::Visibility => target.visibility = source.visibility,
        Property::Opacity => target.opacity = source.opacity,
    }
}

fn keyword(value: &CssValue) -> Option<&str> {
    if let CssValue::Keyword(keyword) = value {
        Some(keyword)
    } else {
        None
    }
}

fn display(value: &CssValue) -> Option<Display> {
    match keyword(value)? {
        "none" => Some(Display::None),
        "block" => Some(Display::Block),
        "inline" => Some(Display::Inline),
        "inline-block" => Some(Display::InlineBlock),
        "list-item" => Some(Display::ListItem),
        _ => None,
    }
}

fn color(value: &CssValue) -> Option<Color> {
    match value {
        CssValue::Color(color) => Some(*color),
        CssValue::Keyword(name) => named_color(name),
        _ => None,
    }
}

fn named_color(name: &str) -> Option<Color> {
    let (red, green, blue, alpha) = match name {
        "transparent" => (0, 0, 0, 0),
        "black" => (0, 0, 0, 255),
        "silver" => (192, 192, 192, 255),
        "gray" => (128, 128, 128, 255),
        "white" => (255, 255, 255, 255),
        "maroon" => (128, 0, 0, 255),
        "red" => (255, 0, 0, 255),
        "purple" => (128, 0, 128, 255),
        "fuchsia" => (255, 0, 255, 255),
        "green" => (0, 128, 0, 255),
        "lime" => (0, 255, 0, 255),
        "olive" => (128, 128, 0, 255),
        "yellow" => (255, 255, 0, 255),
        "navy" => (0, 0, 128, 255),
        "blue" => (0, 0, 255, 255),
        "teal" => (0, 128, 128, 255),
        "aqua" => (0, 255, 255, 255),
        _ => return None,
    };
    Some(Color {
        red,
        green,
        blue,
        alpha,
    })
}

fn size(value: &CssValue, context: &ComputeContext) -> Option<Size> {
    if keyword(value) == Some("auto") {
        return Some(Size::Auto);
    }
    let length = computed_length(value, context)?;
    non_negative(length).then_some(Size::Length(length))
}

fn margin(value: &CssValue, context: &ComputeContext) -> Option<Edges<LengthPercentageAuto>> {
    let components = component_values(value)?;
    let values = components
        .iter()
        .map(|value| {
            if keyword(value) == Some("auto") {
                Some(LengthPercentageAuto::Auto)
            } else {
                computed_length(value, context).map(LengthPercentageAuto::Length)
            }
        })
        .collect::<Option<Vec<_>>>()?;
    Edges::from_components(&values)
}

fn padding(value: &CssValue, context: &ComputeContext) -> Option<Edges<LengthPercentage>> {
    let values = component_values(value)?
        .iter()
        .map(|value| computed_length(value, context).filter(|length| non_negative(*length)))
        .collect::<Option<Vec<_>>>()?;
    Edges::from_components(&values)
}

fn border_width(value: &CssValue, context: &ComputeContext) -> Option<Edges<f32>> {
    let values = component_values(value)?
        .iter()
        .map(|value| match keyword(value) {
            Some("thin") => Some(1.0),
            Some("medium") => Some(3.0),
            Some("thick") => Some(5.0),
            _ => match computed_length(value, context)? {
                LengthPercentage::Px(px) if px >= 0.0 => Some(px),
                _ => None,
            },
        })
        .collect::<Option<Vec<_>>>()?;
    Edges::from_components(&values)
}

fn border_style(value: &CssValue) -> Option<Edges<BorderStyle>> {
    let values = component_values(value)?
        .iter()
        .map(|value| match keyword(value)? {
            "none" => Some(BorderStyle::None),
            "hidden" => Some(BorderStyle::Hidden),
            "dotted" => Some(BorderStyle::Dotted),
            "dashed" => Some(BorderStyle::Dashed),
            "solid" => Some(BorderStyle::Solid),
            "double" => Some(BorderStyle::Double),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Edges::from_components(&values)
}

fn border_color(value: &CssValue) -> Option<Edges<Color>> {
    let values = component_values(value)?
        .iter()
        .map(color)
        .collect::<Option<Vec<_>>>()?;
    Edges::from_components(&values)
}

fn font_size(value: &CssValue, context: &ComputeContext) -> Option<f32> {
    let px = match value {
        CssValue::Percentage(percent) => context.parent_font * percent / 100.0,
        CssValue::Length(number, LengthUnit::Px) => *number,
        CssValue::Length(number, LengthUnit::Em) => context.parent_font * number,
        CssValue::Length(number, LengthUnit::Rem) => context.root_font * number,
        CssValue::Keyword(keyword) => match keyword.as_str() {
            "xx-small" => 9.0,
            "x-small" => 10.0,
            "small" => 13.0,
            "medium" => 16.0,
            "large" => 18.0,
            "x-large" => 24.0,
            "xx-large" => 32.0,
            _ => return None,
        },
        CssValue::Number(number) if *number == 0.0 => 0.0,
        _ => return None,
    };
    (px >= 0.0 && px.is_finite()).then_some(px)
}

fn font_family(value: &CssValue) -> Option<Vec<String>> {
    match value {
        CssValue::Keyword(family) | CssValue::String(family) if !family.is_empty() => {
            Some(vec![family.clone()])
        }
        CssValue::Raw(raw) => {
            let mut families = Vec::new();
            for item in raw.split(',') {
                let family = item.trim();
                if family.is_empty() {
                    return None;
                }
                let quoted = (family.starts_with('"') && family.ends_with('"'))
                    || (family.starts_with('\'') && family.ends_with('\''));
                let family = if quoted {
                    if family.len() < 2 {
                        return None;
                    }
                    &family[1..family.len() - 1]
                } else {
                    family
                };
                if family.is_empty() {
                    return None;
                }
                families.push(family.to_string());
            }
            (!families.is_empty()).then_some(families)
        }
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn font_weight(value: &CssValue) -> Option<FontWeight> {
    match value {
        CssValue::Keyword(keyword) if keyword == "normal" => Some(FontWeight::Normal),
        CssValue::Keyword(keyword) if keyword == "bold" => Some(FontWeight::Bold),
        CssValue::Number(number)
            if number.is_finite()
                && *number >= 1.0
                && *number <= 1000.0
                && number.fract() == 0.0 =>
        {
            Some(FontWeight::Number(*number as u16))
        }
        _ => None,
    }
}

fn line_height(value: &CssValue, context: &ComputeContext) -> Option<LineHeight> {
    match value {
        CssValue::Keyword(keyword) if keyword == "normal" => Some(LineHeight::Normal),
        CssValue::Number(number) if *number >= 0.0 && number.is_finite() => {
            Some(LineHeight::Number(*number))
        }
        CssValue::Percentage(percent) if *percent >= 0.0 && percent.is_finite() => {
            Some(LineHeight::Px(context.own_font * percent / 100.0))
        }
        _ => match computed_length(value, context)? {
            LengthPercentage::Px(px) if px >= 0.0 => Some(LineHeight::Px(px)),
            _ => None,
        },
    }
}

fn text_align(value: &CssValue) -> Option<TextAlign> {
    match keyword(value)? {
        "start" => Some(TextAlign::Start),
        "end" => Some(TextAlign::End),
        "left" => Some(TextAlign::Left),
        "right" => Some(TextAlign::Right),
        "center" => Some(TextAlign::Center),
        "justify" => Some(TextAlign::Justify),
        _ => None,
    }
}

fn visibility(value: &CssValue) -> Option<Visibility> {
    match keyword(value)? {
        "visible" => Some(Visibility::Visible),
        "hidden" => Some(Visibility::Hidden),
        "collapse" => Some(Visibility::Collapse),
        _ => None,
    }
}

fn opacity(value: &CssValue) -> Option<f32> {
    let opacity = match value {
        CssValue::Number(number) => *number,
        CssValue::Percentage(percent) => *percent / 100.0,
        _ => return None,
    };
    opacity.is_finite().then_some(opacity.clamp(0.0, 1.0))
}

fn computed_length(value: &CssValue, context: &ComputeContext) -> Option<LengthPercentage> {
    match value {
        CssValue::Length(number, LengthUnit::Px) => Some(LengthPercentage::Px(*number)),
        CssValue::Length(number, LengthUnit::Em) => {
            Some(LengthPercentage::Px(*number * context.own_font))
        }
        CssValue::Length(number, LengthUnit::Rem) => {
            Some(LengthPercentage::Px(*number * context.root_font))
        }
        CssValue::Length(number, LengthUnit::Vw) => Some(LengthPercentage::ViewportWidth(*number)),
        CssValue::Length(number, LengthUnit::Vh) => Some(LengthPercentage::ViewportHeight(*number)),
        CssValue::Percentage(number) => Some(LengthPercentage::Percentage(*number)),
        CssValue::Number(number) if *number == 0.0 => Some(LengthPercentage::Px(0.0)),
        _ => None,
    }
}

fn non_negative(value: LengthPercentage) -> bool {
    match value {
        LengthPercentage::Px(value)
        | LengthPercentage::Percentage(value)
        | LengthPercentage::ViewportWidth(value)
        | LengthPercentage::ViewportHeight(value) => value >= 0.0 && value.is_finite(),
    }
}

fn component_values(value: &CssValue) -> Option<Vec<CssValue>> {
    if !matches!(value, CssValue::Raw(_)) {
        return Some(vec![value.clone()]);
    }
    let CssValue::Raw(raw) = value else {
        unreachable!();
    };
    let tokens = tokenize(raw);
    let mut values = Vec::new();
    for token in tokens {
        let value = match token.kind {
            TokenKind::Whitespace => continue,
            TokenKind::Ident(keyword) => {
                named_color(&keyword).map_or(CssValue::Keyword(keyword), CssValue::Color)
            }
            TokenKind::Hash(value) => CssValue::Color(parse_hex_color(&value)?),
            TokenKind::Number(value) => CssValue::Number(value),
            TokenKind::Percentage(value) => CssValue::Percentage(value),
            TokenKind::Dimension(value, unit_name) => {
                CssValue::Length(value, length_unit(&unit_name)?)
            }
            _ => return None,
        };
        values.push(value);
    }
    (!values.is_empty() && values.len() <= 4).then_some(values)
}

fn length_unit(unit: &str) -> Option<LengthUnit> {
    match unit {
        "px" => Some(LengthUnit::Px),
        "em" => Some(LengthUnit::Em),
        "rem" => Some(LengthUnit::Rem),
        "vw" => Some(LengthUnit::Vw),
        "vh" => Some(LengthUnit::Vh),
        _ => None,
    }
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let number = u32::from_str_radix(value, 16).ok()?;
    match value.len() {
        3 => Some(Color {
            red: u8::try_from((number >> 8) & 15).ok()? * 17,
            green: u8::try_from((number >> 4) & 15).ok()? * 17,
            blue: u8::try_from(number & 15).ok()? * 17,
            alpha: 255,
        }),
        4 => Some(Color {
            red: u8::try_from((number >> 12) & 15).ok()? * 17,
            green: u8::try_from((number >> 8) & 15).ok()? * 17,
            blue: u8::try_from((number >> 4) & 15).ok()? * 17,
            alpha: u8::try_from(number & 15).ok()? * 17,
        }),
        6 => Some(Color {
            red: u8::try_from((number >> 16) & 255).ok()?,
            green: u8::try_from((number >> 8) & 255).ok()?,
            blue: u8::try_from(number & 255).ok()?,
            alpha: 255,
        }),
        8 => Some(Color {
            red: u8::try_from((number >> 24) & 255).ok()?,
            green: u8::try_from((number >> 16) & 255).ok()?,
            blue: u8::try_from((number >> 8) & 255).ok()?,
            alpha: u8::try_from(number & 255).ok()?,
        }),
        _ => None,
    }
}
