// SPDX-License-Identifier: Apache-2.0

//! Colour, and the absence of it.
//!
//! `--no-color` and a non-TTY stdout both produce plain text
//! (`docs/spec/06-cli-and-ux.md`, "Output contract"), so every method here is
//! the identity function when colour is off. That is the whole design: callers
//! never branch on whether colour is enabled, which is what keeps the plain
//! path from rotting.
//!
//! Styling is applied to text that has already been laid out. An escape
//! sequence occupies no terminal columns but several bytes, so measuring a
//! styled string with [`super::width`] would report it far too wide and pull
//! every column after it out of line.

/// Whether to emit colour, and the eight styles this build uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// True when the output is a terminal the user has not asked to keep plain.
    pub color: bool,
}

/// Ends the current styling.
const RESET: &str = "\u{1b}[0m";

impl Style {
    /// A style that emits colour.
    pub fn colored() -> Self {
        Self { color: true }
    }

    /// A style that emits none: the default, and what non-TTY output uses.
    pub fn plain() -> Self {
        Self { color: false }
    }

    /// Wraps text in an SGR sequence, or returns it unchanged.
    fn wrap(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\u{1b}[{code}m{text}{RESET}")
        } else {
            text.to_string()
        }
    }

    /// A section heading or a table's column headings.
    pub fn heading(self, text: &str) -> String {
        self.wrap("1", text)
    }

    /// Something that is as it should be.
    pub fn ok(self, text: &str) -> String {
        self.wrap("32", text)
    }

    /// Something the user should look at but which is not a failure.
    pub fn warning(self, text: &str) -> String {
        self.wrap("33", text)
    }

    /// A failure.
    pub fn error(self, text: &str) -> String {
        self.wrap("31", text)
    }

    /// Secondary detail: provenance, dates, counts.
    pub fn dim(self, text: &str) -> String {
        self.wrap("2", text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_is_the_default_and_changes_nothing() {
        let style = Style::default();
        assert!(!style.color);
        for text in ["OK", "Warning", "漢字", ""] {
            assert_eq!(style.heading(text), text);
            assert_eq!(style.ok(text), text);
            assert_eq!(style.warning(text), text);
            assert_eq!(style.error(text), text);
            assert_eq!(style.dim(text), text);
        }
    }

    #[test]
    fn plain_output_carries_no_escape_sequence() {
        let style = Style::plain();
        assert!(!style.error("failed").contains('\u{1b}'));
    }

    #[test]
    fn colour_wraps_and_resets() {
        let style = Style::colored();
        assert_eq!(style.error("x"), "\u{1b}[31mx\u{1b}[0m");
        assert_eq!(style.heading("x"), "\u{1b}[1mx\u{1b}[0m");
        assert!(style.ok("x").ends_with(RESET));
    }

    #[test]
    fn the_five_styles_are_distinguishable() {
        let style = Style::colored();
        let rendered = [
            style.heading("x"),
            style.ok("x"),
            style.warning("x"),
            style.error("x"),
            style.dim("x"),
        ];
        let distinct: std::collections::BTreeSet<&String> = rendered.iter().collect();
        assert_eq!(distinct.len(), rendered.len());
    }
}
