use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U},
                D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
                D2D1_RENDER_TARGET_USAGE_NONE,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED,
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_MEASURING_MODE_NATURAL,
            },
            Dxgi::Common::DXGI_FORMAT_UNKNOWN,
        },
    },
};

use crate::{Color, DisplayCommand, DisplayList, Rect, RenderError, Renderer, TextStyle};

/// Direct2D/DirectWrite implementation of Lex's platform-neutral renderer.
pub struct Direct2DRenderer {
    target: ID2D1HwndRenderTarget,
    write_factory: IDWriteFactory,
}

impl Direct2DRenderer {
    /// Creates a hardware-accelerated render target associated with `window`.
    ///
    /// # Errors
    /// Returns an error when Direct2D, DirectWrite, or the window render target
    /// cannot be initialized.
    pub fn new(window: HWND, width: u32, height: u32) -> Result<Self, RenderError> {
        let factory: ID2D1Factory = unsafe {
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).map_err(render_error)?
        };
        let target_properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: windows::Win32::Graphics::Direct2D::Common::D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_UNKNOWN,
                alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
            },
            dpiX: 0.0,
            dpiY: 0.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: Default::default(),
        };
        let window_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: window,
            pixelSize: D2D_SIZE_U { width, height },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let target = unsafe {
            factory
                .CreateHwndRenderTarget(&target_properties, &window_properties)
                .map_err(render_error)?
        };
        let write_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(render_error)? };
        Ok(Self {
            target,
            write_factory,
        })
    }

    fn fill_rect(&self, rect: Rect, color: Color) -> windows::core::Result<()> {
        let target: ID2D1RenderTarget = self.target.cast()?;
        let brush = unsafe { target.CreateSolidColorBrush(&to_color(color), None)? };
        unsafe { target.FillRectangle(&to_rect(rect), &brush) };
        Ok(())
    }

    fn draw_text(&self, text: &str, bounds: Rect, style: &TextStyle) -> windows::core::Result<()> {
        let family = wide(&style.family);
        let locale = wide("en-us");
        let format = unsafe {
            self.write_factory.CreateTextFormat(
                PCWSTR(family.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.size,
                PCWSTR(locale.as_ptr()),
            )?
        };
        let target: ID2D1RenderTarget = self.target.cast()?;
        let brush = unsafe { target.CreateSolidColorBrush(&to_color(style.color), None)? };
        let text = wide(text);
        unsafe {
            self.target.DrawText(
                &text[..text.len().saturating_sub(1)],
                &format,
                &to_rect(bounds),
                &brush,
                Default::default(),
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }
}

impl Renderer for Direct2DRenderer {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        unsafe { self.target.Resize(&D2D_SIZE_U { width, height }) }.map_err(render_error)
    }

    fn render(&mut self, list: &DisplayList) -> Result<(), RenderError> {
        if !list.clip_depth_is_balanced() {
            return Err(RenderError("display list has unbalanced clips".into()));
        }

        unsafe { self.target.BeginDraw() };
        for command in list.commands() {
            match command {
                DisplayCommand::Clear(color) => unsafe {
                    self.target.Clear(Some(&to_color(*color)));
                },
                DisplayCommand::FillRect { rect, color } => {
                    self.fill_rect(*rect, *color).map_err(render_error)?;
                }
                DisplayCommand::DrawText {
                    text,
                    bounds,
                    style,
                } => self.draw_text(text, *bounds, style).map_err(render_error)?,
                DisplayCommand::PushClip(rect) => unsafe {
                    self.target
                        .PushAxisAlignedClip(&to_rect(*rect), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                },
                DisplayCommand::PopClip => unsafe { self.target.PopAxisAlignedClip() },
            }
        }
        unsafe { self.target.EndDraw(None, None) }.map_err(render_error)
    }
}

fn to_color(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.red,
        g: color.green,
        b: color.blue,
        a: color.alpha,
    }
}

fn to_rect(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn render_error(error: windows::core::Error) -> RenderError {
    RenderError(format!("Direct2D/DirectWrite error: {error}"))
}
