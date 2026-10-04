//! Platform-neutral state for Lex's browser chrome.

/// User-visible state needed by the bootstrap window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeState {
    title: String,
    status: String,
}

impl Default for ChromeState {
    fn default() -> Self {
        Self {
            title: "LEX".into(),
            status: "Browser engine initializing...".into(),
        }
    }
}

impl ChromeState {
    /// The product name rendered in the client area.
    #[must_use]
    pub fn title(&self) -> &str { &self.title }

    /// Current startup status rendered below the product name.
    #[must_use]
    pub fn status(&self) -> &str { &self.status }
}

#[cfg(test)]
mod tests {
    use super::ChromeState;

    #[test]
    fn bootstrap_copy_is_stable() {
        let chrome = ChromeState::default();
        assert_eq!(chrome.title(), "LEX");
        assert_eq!(chrome.status(), "Browser engine initializing...");
    }
}
