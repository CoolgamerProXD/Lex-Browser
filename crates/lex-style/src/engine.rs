use std::collections::BTreeMap;

use lex_css::{
    matching_declarations, parse_declaration_list, Declaration, MatchedDeclaration, Specificity,
    Stylesheet,
};
use lex_dom::{Document, MutationRecord, Node, NodeId};

use crate::ua::{apply_box_defaults, apply_primary_defaults};
use crate::values::{
    apply_value, copy_property, valid_specified, ComputeContext, ComputedStyle, Property,
    SpecifiedStyle, SpecifiedValue,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationReason {
    AttributeChanged,
    ClassChanged,
    IdChanged,
    NodeInserted,
    NodeRemoved,
    StylesheetChanged,
}

/// A conservative reason/scope record. M7 deliberately recomputes the full
/// connected document for every style-relevant invalidation; retaining the
/// affected node establishes a stable path to subtree invalidation later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleInvalidation {
    pub reason: InvalidationReason,
    pub node: Option<NodeId>,
}

/// Owns parsed author stylesheets and the complete style snapshot for one DOM.
/// Selector matching, cascade, inline declarations, inheritance, UA defaults,
/// and value computation all stop at this boundary; future layout only needs
/// [`computed_style`](Self::computed_style).
#[derive(Clone, Debug)]
pub struct StyleEngine {
    stylesheets: Vec<Stylesheet>,
    specified: BTreeMap<NodeId, SpecifiedStyle>,
    computed: BTreeMap<NodeId, ComputedStyle>,
    dirty: bool,
    generation: u64,
}

impl Default for StyleEngine {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl StyleEngine {
    #[must_use]
    pub fn new(stylesheets: Vec<Stylesheet>) -> Self {
        Self {
            stylesheets,
            specified: BTreeMap::new(),
            computed: BTreeMap::new(),
            dirty: true,
            generation: 0,
        }
    }

    #[must_use]
    pub fn stylesheets(&self) -> &[Stylesheet] {
        &self.stylesheets
    }

    /// Replaces author stylesheets and marks the document dirty.
    #[must_use]
    pub fn replace_stylesheets(&mut self, stylesheets: Vec<Stylesheet>) -> StyleInvalidation {
        self.stylesheets = stylesheets;
        self.dirty = true;
        StyleInvalidation {
            reason: InvalidationReason::StylesheetChanged,
            node: None,
        }
    }

    /// Appends an author stylesheet and marks the document dirty.
    #[must_use]
    pub fn add_stylesheet(&mut self, stylesheet: Stylesheet) -> StyleInvalidation {
        self.stylesheets.push(stylesheet);
        self.dirty = true;
        StyleInvalidation {
            reason: InvalidationReason::StylesheetChanged,
            node: None,
        }
    }

    /// Converts DOM mutation facts into conservative style invalidations.
    /// Character-data changes are intentionally ignored because M7 supports no
    /// text-sensitive selectors or generated content.
    #[must_use]
    pub fn note_mutations(&mut self, mutations: &[MutationRecord]) -> Vec<StyleInvalidation> {
        let invalidations: Vec<_> = mutations
            .iter()
            .filter_map(|mutation| match mutation {
                MutationRecord::AttributeChanged { element, name, .. } => {
                    let reason = match name.as_str() {
                        "class" => InvalidationReason::ClassChanged,
                        "id" => InvalidationReason::IdChanged,
                        _ => InvalidationReason::AttributeChanged,
                    };
                    Some(StyleInvalidation {
                        reason,
                        node: Some(*element),
                    })
                }
                MutationRecord::ChildInserted { node, .. } => Some(StyleInvalidation {
                    reason: InvalidationReason::NodeInserted,
                    node: Some(*node),
                }),
                MutationRecord::ChildRemoved { node, .. } => Some(StyleInvalidation {
                    reason: InvalidationReason::NodeRemoved,
                    node: Some(*node),
                }),
                MutationRecord::CharacterDataChanged { .. } => None,
            })
            .collect();
        if !invalidations.is_empty() {
            self.dirty = true;
        }
        invalidations
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Computes a fresh snapshot for every connected element in document
    /// preorder. Detached nodes disappear from the snapshot deterministically.
    pub fn recompute(&mut self, document: &Document) {
        self.specified.clear();
        self.computed.clear();

        let nodes = document.preorder(document.root()).unwrap_or_default();
        let mut root_font_size = 16.0;
        let mut found_document_element = false;

        for node_id in nodes {
            let Some(record) = document.node(node_id) else {
                continue;
            };
            let Node::Element(element) = &record.node else {
                continue;
            };

            let parent_style = nearest_parent_style(document, node_id, &self.computed).cloned();
            let specified = cascade_specified(document, node_id, &self.stylesheets);
            let style = compute_style(
                &specified,
                parent_style.as_ref(),
                &element.tag_name,
                root_font_size,
            );

            if !found_document_element {
                root_font_size = style.font_size;
                found_document_element = true;
            }
            self.specified.insert(node_id, specified);
            self.computed.insert(node_id, style);
        }

        self.dirty = false;
        self.generation = self.generation.saturating_add(1);
    }

    /// Recomputes only when construction, mutation, or stylesheet replacement
    /// marked this engine dirty. Returns whether work was performed.
    #[must_use]
    pub fn recompute_if_needed(&mut self, document: &Document) -> bool {
        if !self.dirty {
            return false;
        }
        self.recompute(document);
        true
    }

    #[must_use]
    pub fn computed_style(&self, node: NodeId) -> Option<&ComputedStyle> {
        self.computed.get(&node)
    }

    #[must_use]
    pub fn specified_style(&self, node: NodeId) -> Option<&SpecifiedStyle> {
        self.specified.get(&node)
    }

    #[must_use]
    pub fn styled_element_count(&self) -> usize {
        self.computed.len()
    }

    pub fn computed_styles(&self) -> impl Iterator<Item = (NodeId, &ComputedStyle)> {
        self.computed.iter().map(|(node, style)| (*node, style))
    }
}

fn nearest_parent_style<'a>(
    document: &Document,
    node: NodeId,
    computed: &'a BTreeMap<NodeId, ComputedStyle>,
) -> Option<&'a ComputedStyle> {
    let mut parent = document.parent(node);
    while let Some(node) = parent {
        if let Some(style) = computed.get(&node) {
            return Some(style);
        }
        parent = document.parent(node);
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CascadeRank {
    important: bool,
    inline: bool,
    specificity: Specificity,
    source_order: usize,
    sequence: usize,
}

fn cascade_specified(
    document: &Document,
    element: NodeId,
    stylesheets: &[Stylesheet],
) -> SpecifiedStyle {
    let mut declarations = matching_declarations(document, element, stylesheets)
        .into_iter()
        .map(|matched| (matched, false))
        .collect::<Vec<_>>();

    if let Ok(Some(inline)) = document.attribute(element, "style") {
        let list = parse_declaration_list(inline);
        let source_order = declarations
            .iter()
            .map(|(matched, _)| matched.source_order)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        declarations.extend(list.declarations.into_iter().map(|declaration| {
            (
                MatchedDeclaration {
                    declaration,
                    specificity: Specificity::default(),
                    source_order,
                },
                true,
            )
        }));
    }

    let mut winners: BTreeMap<Property, (CascadeRank, SpecifiedValue)> = BTreeMap::new();
    for (sequence, (matched, inline)) in declarations.into_iter().enumerate() {
        consider_declaration(&mut winners, matched, inline, sequence);
    }

    let mut specified = SpecifiedStyle::default();
    for (property, (_, value)) in winners {
        specified.insert(property, value);
    }
    specified
}

fn consider_declaration(
    winners: &mut BTreeMap<Property, (CascadeRank, SpecifiedValue)>,
    matched: MatchedDeclaration,
    inline: bool,
    sequence: usize,
) {
    let Declaration {
        name,
        value,
        important,
        ..
    } = matched.declaration;
    let Some(property) = Property::from_name(&name) else {
        return;
    };
    let specified = SpecifiedValue::from_css(&value);
    if !valid_specified(property, &specified) {
        return;
    }
    let rank = CascadeRank {
        important,
        inline,
        specificity: matched.specificity,
        source_order: matched.source_order,
        sequence,
    };
    let replace = winners
        .get(&property)
        .map_or(true, |(old_rank, _)| rank >= *old_rank);
    if replace {
        winners.insert(property, (rank, specified));
    }
}

fn compute_style(
    specified: &SpecifiedStyle,
    parent: Option<&ComputedStyle>,
    tag_name: &str,
    root_font_size: f32,
) -> ComputedStyle {
    let initial = ComputedStyle::initial_values();
    let mut style = initial.clone();

    if let Some(parent) = parent {
        for property in INHERITED_PROPERTIES {
            copy_property(&mut style, parent, property);
        }
    }

    apply_primary_defaults(&mut style, tag_name);

    // Font size computes first because em values and line-height depend on it.
    apply_specified_property(
        &mut style,
        specified,
        Property::FontSize,
        parent,
        &initial,
        root_font_size,
    );

    apply_box_defaults(&mut style, tag_name);

    for property in ALL_PROPERTIES {
        if property != Property::FontSize {
            apply_specified_property(
                &mut style,
                specified,
                property,
                parent,
                &initial,
                root_font_size,
            );
        }
    }
    style
}

fn apply_specified_property(
    style: &mut ComputedStyle,
    specified: &SpecifiedStyle,
    property: Property,
    parent: Option<&ComputedStyle>,
    initial: &ComputedStyle,
    root_font_size: f32,
) {
    let Some(value) = specified.get(property) else {
        return;
    };
    match value {
        SpecifiedValue::Inherit => {
            copy_property(style, parent.unwrap_or(initial), property);
        }
        SpecifiedValue::Initial => copy_property(style, initial, property),
        SpecifiedValue::Unset => {
            let source = if property.is_inherited() {
                parent.unwrap_or(initial)
            } else {
                initial
            };
            copy_property(style, source, property);
        }
        SpecifiedValue::Value(value) => {
            let context = ComputeContext {
                parent_font: parent.map_or(initial.font_size, |style| style.font_size),
                own_font: style.font_size,
                root_font: root_font_size,
            };
            apply_value(style, property, value, &context);
        }
    }
}

const INHERITED_PROPERTIES: [Property; 7] = [
    Property::Color,
    Property::FontFamily,
    Property::FontSize,
    Property::FontWeight,
    Property::LineHeight,
    Property::TextAlign,
    Property::Visibility,
];

const ALL_PROPERTIES: [Property; 17] = [
    Property::Display,
    Property::Color,
    Property::BackgroundColor,
    Property::Width,
    Property::Height,
    Property::Margin,
    Property::Padding,
    Property::BorderWidth,
    Property::BorderStyle,
    Property::BorderColor,
    Property::FontSize,
    Property::FontFamily,
    Property::FontWeight,
    Property::LineHeight,
    Property::TextAlign,
    Property::Visibility,
    Property::Opacity,
];
