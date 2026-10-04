//! Lex display commands and renderer contract.
//!
//! Engine code emits [`DisplayList`] values and has no knowledge of Direct2D
//! or any other native graphics API.

use std::{error::Error, fmt};

/// An RGBA colour with non-premultiplied channels in the range `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const LEX_INK: Self = Self::rgb(0.10, 0.14, 0.19);

    #[must_use]
    pub const fn rgb(red: f32, green: f32, blue: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

/// A rectangle in device-independent pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// A font request attached to one text drawing operation.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: String,
    pub size: f32,
    pub color: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: "Segoe UI".into(),
            size: 16.0,
            color: Color::LEX_INK,
        }
    }
}

/// A backend-independent painting operation.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayCommand {
    Clear(Color),
    FillRect {
        rect: Rect,
        color: Color,
    },
    DrawText {
        text: String,
        bounds: Rect,
        style: TextStyle,
    },
    PushClip(Rect),
    PopClip,
}

/// An ordered sequence of painting operations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    commands: Vec<DisplayCommand>,
}

impl DisplayList {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: DisplayCommand) {
        self.commands.push(command);
    }

    #[must_use]
    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
    }

    #[must_use]
    pub fn clip_depth_is_balanced(&self) -> bool {
        let mut depth = 0_u32;
        for command in &self.commands {
            match command {
                DisplayCommand::PushClip(_) => depth = depth.saturating_add(1),
                DisplayCommand::PopClip if depth == 0 => return false,
                DisplayCommand::PopClip => depth -= 1,
                _ => {}
            }
        }
        depth == 0
    }
}

/// Rendering failure that can be shown by the application without panicking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError(pub String);

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RenderError {}

/// Consumes Lex display commands. Implementations own platform resources.
pub trait Renderer {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError>;
    fn render(&mut self, list: &DisplayList) -> Result<(), RenderError>;
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Direct2DRenderer;

#[cfg(test)]
mod tests {
    use super::{Color, DisplayCommand, DisplayList, Rect, TextStyle};

    #[test]
    fn preserves_paint_order_and_payload() {
        let mut list = DisplayList::new();
        list.push(DisplayCommand::Clear(Color::WHITE));
        list.push(DisplayCommand::FillRect {
            rect: Rect::new(1.0, 2.0, 30.0, 40.0),
            color: Color::LEX_INK,
        });
        list.push(DisplayCommand::DrawText {
            text: "Lex".into(),
            bounds: Rect::new(8.0, 8.0, 100.0, 30.0),
            style: TextStyle::default(),
        });
        assert_eq!(list.commands().len(), 3);
        assert!(matches!(list.commands()[0], DisplayCommand::Clear(_)));
    }

    #[test]
    fn detects_unbalanced_clips() {
        let mut list = DisplayList::new();
        list.push(DisplayCommand::PushClip(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(!list.clip_depth_is_balanced());
        list.push(DisplayCommand::PopClip);
        assert!(list.clip_depth_is_balanced());
        list.push(DisplayCommand::PopClip);
        assert!(!list.clip_depth_is_balanced());
    }
}
