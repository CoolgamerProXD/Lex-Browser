use std::mem::size_of;
use lex_ui::ChromeState;
use windows::{core::{w, Error, Result}, Win32::{Foundation::{HWND, LPARAM, LRESULT, WPARAM}, Graphics::Gdi::{BeginPaint, EndPaint, SetBkMode, SetTextColor, TextOutW, HBRUSH, PAINTSTRUCT, RGB, TRANSPARENT}, System::LibraryLoader::GetModuleHandleW, UI::{HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2}, WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadCursorW, PostQuitMessage, RegisterClassExW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, COLOR_WINDOW, CW_USEDEFAULT, IDC_ARROW, MSG, SW_SHOW, WINDOW_EX_STYLE, WM_DESTROY, WM_KEYDOWN, WM_MOUSEMOVE, WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW, WS_VISIBLE}}}};
use crate::ApplicationState;

pub(super) fn run() -> Result<()> {
    // Available on supported Windows versions; failure only means system DPI fallback.
    unsafe { let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }
    let instance = unsafe { GetModuleHandleW(None)? };
    let class_name = w!("LexBrowserWindow");
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        lpszClassName: class_name,
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&class) } == 0 { return Err(Error::from_win32()); }
    let state = Box::new(ApplicationState::default());
    let hwnd = unsafe { CreateWindowExW(WINDOW_EX_STYLE::default(), class_name, w!("Lex"), WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT, 1100, 760, None, None, instance, Some(Box::into_raw(state).cast()))? };
    unsafe { ShowWindow(hwnd, SW_SHOW); }
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        unsafe { let _ = TranslateMessage(&message); DispatchMessageW(&message); }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{CREATESTRUCTW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW, WM_NCCREATE, WM_NCDESTROY};
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize); }
        return LRESULT(1);
    }
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ApplicationState };
    match message {
        WM_SIZE if !state.is_null() => { unsafe { (*state).resized((lparam.0 & 0xffff) as u32, ((lparam.0 >> 16) & 0xffff) as u32); } LRESULT(0) }
        WM_MOUSEMOVE if !state.is_null() => { unsafe { (*state).pointer_moved((lparam.0 as i16) as i32, ((lparam.0 >> 16) as i16) as i32); } LRESULT(0) }
        WM_KEYDOWN if !state.is_null() => { unsafe { (*state).key_pressed(wparam.0 as u16); } LRESULT(0) }
        WM_PAINT => { unsafe { paint(hwnd); } LRESULT(0) }
        WM_DESTROY => { unsafe { PostQuitMessage(0); } LRESULT(0) }
        WM_NCDESTROY => { if !state.is_null() { unsafe { drop(Box::from_raw(state)); SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0); } } unsafe { DefWindowProcW(hwnd, message, wparam, lparam) } }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    unsafe { SetBkMode(dc, TRANSPARENT); SetTextColor(dc, RGB(25, 35, 48)); }
    let chrome = ChromeState::default();
    let title: Vec<u16> = chrome.title().encode_utf16().collect();
    let status: Vec<u16> = chrome.status().encode_utf16().collect();
    unsafe { let _ = TextOutW(dc, 48, 48, &title); let _ = TextOutW(dc, 48, 82, &status); EndPaint(hwnd, &paint); }
}
