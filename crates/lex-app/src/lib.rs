//! Lex's native application boundary.
//! Windows messages are translated into this small, testable state model.

/// Last known pointer position in device-independent client coordinates.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PointerPosition { pub x: i32, pub y: i32 }

/// State owned by one top-level Lex window.
#[derive(Debug, Default)]
pub struct ApplicationState {
    client_size: (u32, u32),
    pointer: PointerPosition,
    last_key: Option<u16>,
}

impl ApplicationState {
    pub fn resized(&mut self, width: u32, height: u32) { self.client_size = (width, height); }
    pub fn pointer_moved(&mut self, x: i32, y: i32) { self.pointer = PointerPosition { x, y }; }
    pub fn key_pressed(&mut self, virtual_key: u16) { self.last_key = Some(virtual_key); }
    #[must_use] pub fn client_size(&self) -> (u32, u32) { self.client_size }
    #[must_use] pub fn pointer(&self) -> PointerPosition { self.pointer }
    #[must_use] pub fn last_key(&self) -> Option<u16> { self.last_key }
}

#[cfg(windows)]
mod windows_app;

/// Run the platform application.
#[cfg(windows)]
pub fn run() -> Result<(), String> { windows_app::run().map_err(|error| error.to_string()) }

/// Explains the target requirement when invoked on a development host.
#[cfg(not(windows))]
pub fn run() -> Result<(), String> {
    Err("Lex's native shell requires Windows 10 or newer".into())
}

#[cfg(test)]
mod tests {
    use super::{ApplicationState, PointerPosition};

    #[test]
    fn input_and_resize_are_recorded() {
        let mut state = ApplicationState::default();
        state.resized(1280, 720);
        state.pointer_moved(42, 84);
        state.key_pressed(0x41);
        assert_eq!(state.client_size(), (1280, 720));
        assert_eq!(state.pointer(), PointerPosition { x: 42, y: 84 });
        assert_eq!(state.last_key(), Some(0x41));
    }
}
