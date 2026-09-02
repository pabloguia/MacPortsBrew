// SPDX-License-Identifier: Apache-2.0

//! Display width: the only correct measure for laying text out in a terminal.
//!
//! `str::len` counts bytes and `str::chars().count()` counts scalar values;
//! neither is the number of terminal columns a string occupies. Japanese and
//! Chinese are among the target locales (`docs/spec/06-cli-and-ux.md`) and are
//! written in characters two columns wide, so a table whose columns were sized
//! by either count is ragged for two of the five locales this project promises
//! to support. Combining marks are the mirror image: they add characters and
//! no columns.
//!
//! Everything here measures with `unicode-width`, which implements Unicode
//! Standard Annex #11. East Asian Ambiguous characters are treated as narrow —
//! that is the crate's `width()` behaviour, and it is why the truncation marker
//! below is ASCII rather than U+2026, whose width depends on the terminal's
//! locale.
//!
//! Measure before styling, never after: an ANSI escape sequence occupies zero
//! columns but several bytes and characters, so a coloured string measured here
//! would be reported far too wide. [`crate::render::style`] is applied to text
//! that has already been laid out.

use unicode_width::UnicodeWidthStr;

/// The marker appended to text that had to be cut short.
///
/// ASCII, and therefore exactly three columns wide in every terminal and every
/// locale. A single-character ellipsis would be one column narrower but is
/// East Asian Ambiguous, so its width — and with it the whole table's
/// alignment — would depend on the reader's terminal settings.
pub const ELLIPSIS: &str = "...";

/// The display width of [`ELLIPSIS`].
pub const ELLIPSIS_WIDTH: usize = 3;

/// The number of terminal columns a string occupies.
pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Pads on the right with spaces until the string is `width` columns.
///
/// A string already at or beyond `width` is returned unchanged: padding is for
/// alignment, and silently cutting text off would be [`truncate_to`]'s job.
pub fn pad_to(text: &str, width: usize) -> String {
    let current = display_width(text);
    if current >= width {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + (width - current));
    out.push_str(text);
    for _ in 0..(width - current) {
        out.push(' ');
    }
    out
}

/// Shortens a string to at most `width` columns, marking the cut.
///
/// The result is never wider than `width` and never splits a character. A
/// double-width character that would straddle the boundary is dropped rather
/// than half-printed, so the result can come back one column narrower than the
/// budget; that is the correct answer, because there is no such thing as half
/// of 漢.
///
/// Below [`ELLIPSIS_WIDTH`] columns there is no room to say that text was cut,
/// so the text is cut without a marker.
pub fn truncate_to(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width < ELLIPSIS_WIDTH {
        return take_columns(text, width);
    }
    let mut out = take_columns(text, width - ELLIPSIS_WIDTH);
    out.push_str(ELLIPSIS);
    out
}

/// The longest prefix of `text` that fits in `budget` columns.
fn take_columns(text: &str, budget: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for c in text.chars() {
        let mut buffer = [0u8; 4];
        let w = display_width(c.encode_utf8(&mut buffer));
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_one_column_per_character() {
        assert_eq!(display_width("wget"), 4);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn east_asian_text_is_two_columns_per_character() {
        assert_eq!(display_width("漢字"), 4);
        assert_eq!(display_width("日本語のパッケージ"), 18);
        assert_eq!(display_width("软件包"), 6);
    }

    #[test]
    fn greek_and_cyrillic_are_narrow() {
        assert_eq!(display_width("Ελληνικά"), 8);
        assert_eq!(display_width("Пакет"), 5);
    }

    #[test]
    fn accented_latin_is_narrow() {
        // The pseudolocale must not accidentally become a double-width test.
        assert_eq!(display_width("ápp"), 3);
        assert_eq!(display_width("Ø"), 1);
    }

    #[test]
    fn padding_reaches_the_requested_column_count() {
        assert_eq!(display_width(&pad_to("漢字", 8)), 8);
        assert_eq!(pad_to("ab", 5), "ab   ");
        assert_eq!(pad_to("abcdef", 3), "abcdef");
        assert_eq!(pad_to("", 2), "  ");
    }

    #[test]
    fn truncation_never_exceeds_the_budget() {
        for width in 0..24 {
            for text in ["wget", "日本語のパッケージ", "Ελληνικά", "a漢b字c"] {
                let cut = truncate_to(text, width);
                assert!(
                    display_width(&cut) <= width,
                    "{text:?} cut to {width} gave {cut:?}"
                );
            }
        }
    }

    #[test]
    fn short_enough_text_is_returned_untouched() {
        assert_eq!(truncate_to("wget", 4), "wget");
        assert_eq!(truncate_to("漢字", 4), "漢字");
    }

    #[test]
    fn truncation_marks_the_cut() {
        assert_eq!(truncate_to("abcdefgh", 5), "ab...");
        assert!(truncate_to("日本語のパッケージ", 9).ends_with(ELLIPSIS));
    }

    #[test]
    fn truncation_does_not_split_a_double_width_character() {
        // Budget 6 leaves 3 columns before the marker; one and a half kanji do
        // not fit, so one is dropped and the result is a column short.
        let cut = truncate_to("漢字漢字", 6);
        assert_eq!(cut, "漢...");
        assert_eq!(display_width(&cut), 5);
    }

    #[test]
    fn too_narrow_for_a_marker_cuts_without_one() {
        assert_eq!(truncate_to("abcdef", 2), "ab");
        assert_eq!(truncate_to("漢字", 2), "漢");
        assert_eq!(truncate_to("漢字", 1), "");
        assert_eq!(truncate_to("abcdef", 0), "");
    }
}
