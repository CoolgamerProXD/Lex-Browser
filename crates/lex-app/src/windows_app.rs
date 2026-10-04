use std::mem::size_of;

use lex_render::{Color, Direct2DRenderer, DisplayCommand, DisplayList, Rect, Renderer, TextStyle};
use lex_ui::ChromeState;
use windows::{
    core::{w, Error, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{BeginPaint, EndPaint, COLOR_WINDOW, HBRUSH, PAINTSTRUCT},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadCursorW,
                PostQuitMessage, RegisterClassExW, ShowWindow, TranslateMessage, CREATESTRUCTW,
                CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOW,
                WINDOW_EX_STYLE, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_MOUSEMOVE, WM_NCCREATE,
                WM_NCDESTROY, WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            },
        },
    },
};

use crate::ApplicationState;

struct WindowData {
    application: ApplicationState,
    renderer: Option<Direct2DRenderer>,
}

pub(super) fn run() -> Result<()> {
    // Failure means Windows will use its system DPI fallback.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let instance = unsafe { GetModuleHandleW(None)? };
    let class_name = w!("LexBrowserWindow");
    let class_size = u32::try_from(size_of::<WNDCLASSEXW>())
        .expect("WNDCLASSEXW size must fit the Win32 cbSize field");
    let class = WNDCLASSEXW {
        cbSize: class_size,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut core::ffi::c_void),
        lpszClassName: class_name,
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&class) } == 0 {
        return Err(Error::from_win32());
    }

    let data = Box::new(WindowData {
        application: ApplicationState::default(),
        renderer: None,
    });
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("Lex"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1100,
            760,
            None,
            None,
            instance,
            Some(Box::into_raw(data).cast()),
        )?
    };
    unsafe {
        ShowWindow(window, SW_SHOW);
    }

    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                window,
                GWLP_USERDATA,
                create.lpCreateParams as isize,
            );
        }
        return LRESULT(1);
    }

    let data = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(window, GWLP_USERDATA)
            as *mut WindowData
    };
    match message {
        WM_CREATE if !data.is_null() => {
            match Direct2DRenderer::new(window, 1100, 760) {
                Ok(renderer) => unsafe { (*data).renderer = Some(renderer) },
                Err(error) => eprintln!("LEX render initialization failed: {error}"),
            }
            LRESULT(0)
        }
        WM_SIZE if !data.is_null() => {
            let (low_word, high_word) = lparam_words(lparam);
            let width = u32::from(low_word);
            let height = u32::from(high_word);
            unsafe {
                (*data).application.resized(width, height);
                if let Some(renderer) = &mut (*data).renderer {
                    if let Err(error) = renderer.resize(width, height) {
                        eprintln!("LEX render resize failed: {error}");
                    }
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE if !data.is_null() => {
            let (low_word, high_word) = lparam_words(lparam);
            let x = i32::from(i16::from_ne_bytes(low_word.to_ne_bytes()));
            let y = i32::from(i16::from_ne_bytes(high_word.to_ne_bytes()));
            unsafe { (*data).application.pointer_moved(x, y) };
            LRESULT(0)
        }
        WM_KEYDOWN if !data.is_null() => {
            let bytes = wparam.0.to_ne_bytes();
            let virtual_key = u16::from_ne_bytes([bytes[0], bytes[1]]);
            unsafe { (*data).application.key_pressed(virtual_key) };
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe { paint(window, data) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_NCDESTROY => {
            if !data.is_null() {
                unsafe {
                    drop(Box::from_raw(data));
                    windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                        window,
                        GWLP_USERDATA,
                        0,
                    );
                }
            }
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn lparam_words(lparam: LPARAM) -> (u16, u16) {
    let bytes = lparam.0.to_ne_bytes();
    (
        u16::from_ne_bytes([bytes[0], bytes[1]]),
        u16::from_ne_bytes([bytes[2], bytes[3]]),
    )
}

unsafe fn paint(window: HWND, data: *mut WindowData) {
    let mut paint = PAINTSTRUCT::default();
    unsafe { BeginPaint(window, &mut paint) };
    if !data.is_null() {
        if let Some(renderer) = unsafe { &mut (*data).renderer } {
            if let Err(error) = renderer.render(&bootstrap_display_list()) {
                eprintln!("LEX render failed: {error}");
            }
        }
    }
    unsafe { EndPaint(window, &paint) };
}

fn bootstrap_display_list() -> DisplayList {
    let chrome = ChromeState::default();
    let mut list = DisplayList::new();
    list.push(DisplayCommand::Clear(Color::WHITE));
    list.push(DisplayCommand::PushClip(Rect::new(
        32.0, 32.0, 700.0, 120.0,
    )));
    list.push(DisplayCommand::FillRect {
        rect: Rect::new(32.0, 38.0, 4.0, 76.0),
        color: Color::rgb(0.20, 0.42, 0.92),
    });
    list.push(DisplayCommand::DrawText {
        text: chrome.title().into(),
        bounds: Rect::new(48.0, 40.0, 640.0, 36.0),
        style: TextStyle {
            size: 26.0,
            ..TextStyle::default()
        },
    });
    list.push(DisplayCommand::DrawText {
        text: chrome.status().into(),
        bounds: Rect::new(48.0, 80.0, 640.0, 30.0),
        style: TextStyle::default(),
    });
    list.push(DisplayCommand::PopClip);
    list
}
