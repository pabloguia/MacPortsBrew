// SPDX-License-Identifier: Apache-2.0

//! Layout fixtures for the writing systems that break naïve column arithmetic,
//! and the ADR-0002 verification of the candidate TUI crate.

use macportsbrew::render::{display_width, truncate_to, Style, Table};

/// Splits a line at a display-width offset, so an assertion can be written in
/// terminal columns rather than bytes — which is the whole point.
fn split_at_column(line: &str, column: usize) -> (&str, &str) {
    let mut used = 0;
    for (index, c) in line.char_indices() {
        if used == column {
            return line.split_at(index);
        }
        let mut buffer = [0u8; 4];
        used += display_width(c.encode_utf8(&mut buffer));
    }
    (line, "")
}

// --- Width fixtures ---------------------------------------------------------

#[test]
fn east_asian_text_is_measured_in_columns_not_characters() {
    assert_eq!(display_width("漢字"), 4);
    assert_eq!("漢字".chars().count(), 2);

    assert_eq!(display_width("日本語のパッケージ"), 18);
    assert_eq!("日本語のパッケージ".chars().count(), 9);

    // Simplified Chinese and full-width punctuation are double-width too.
    assert_eq!(display_width("软件包"), 6);
    assert_eq!(display_width("："), 2);
}

#[test]
fn greek_and_cyrillic_are_single_width() {
    assert_eq!(display_width("Ελληνικά"), 8);
    assert_eq!("Ελληνικά".chars().count(), 8);
    assert_eq!(display_width("Пакет"), 5);
}

#[test]
fn a_combining_mark_adds_a_character_and_no_column() {
    // "e" followed by U+0301 COMBINING ACUTE ACCENT.
    let decomposed = "e\u{301}";
    assert_eq!(decomposed.chars().count(), 2);
    assert_eq!(display_width(decomposed), 1);
}

// --- Table alignment --------------------------------------------------------

#[test]
fn a_cjk_cell_and_an_ascii_cell_put_the_next_column_in_the_same_place() {
    let mut table = Table::new(["Package", "Provider", "Version"]);
    table.push(["wget", "homebrew", "1.25.0"]);
    table.push(["日本語のパッケージ", "macports", "2.0"]);
    table.push(["Ελληνικά", "homebrew", "3.1"]);

    // The widest first cell is eighteen columns; the gutter is two.
    let offsets = table.column_offsets(None);
    assert_eq!(offsets, vec![0, 20, 30]);

    let rendered = table.render(None);
    assert_eq!(rendered.lines().count(), 4);

    for line in rendered.lines() {
        for offset in &offsets[1..] {
            let (before, _) = split_at_column(line, *offset);
            assert_eq!(
                display_width(before),
                *offset,
                "column at {offset} is misaligned in {line:?}"
            );
        }
    }

    // And the columns really do start with the values, not mid-character.
    for (line, expected) in rendered.lines().skip(1).zip([
        ("homebrew", "1.25.0"),
        ("macports", "2.0"),
        ("homebrew", "3.1"),
    ]) {
        let (_, rest) = split_at_column(line, offsets[1]);
        assert!(rest.starts_with(expected.0), "{line:?}");
        let (_, rest) = split_at_column(line, offsets[2]);
        assert!(rest.starts_with(expected.1), "{line:?}");
    }
}

#[test]
fn a_squeezed_cjk_cell_is_cut_on_a_character_boundary() {
    let mut table = Table::new(["Package"]);
    table.push(["日本語のパッケージ"]);
    let rendered = table.render(Some(9));
    let cell = rendered.lines().nth(1).unwrap();
    assert!(display_width(cell) <= 9, "{cell:?}");
    assert!(cell.ends_with("..."), "{cell:?}");
    // Never half a character.
    assert!(cell.is_char_boundary(cell.len()));
    assert_eq!(truncate_to("日本語のパッケージ", 9), cell);
}

#[test]
fn plain_output_is_free_of_escape_sequences() {
    let mut table = Table::new(["Package", "Support"]);
    table.push(["日本語のパッケージ", &Style::plain().warning("Degraded")]);
    let rendered = table.render(None);
    assert!(!rendered.contains('\u{1b}'));
    // Stable: the same input renders the same bytes.
    assert_eq!(rendered, table.render(None));
}

// --- ADR-0002 ---------------------------------------------------------------

/// ADR-0002 leaves one question open for the foundation pass: whether the
/// candidate TUI crate measures East Asian text the way this renderer does.
/// If it did not, the interactive view and the plain view would disagree about
/// where a column starts, and `docs/spec/06-cli-and-ux.md`'s promise that
/// anything the TUI can do the flags can do too would be false at the layout
/// level.
///
/// It does: ratatui lays "漢字" out over four buffer cells, giving each kanji
/// its own cell followed by a blank continuation cell, so the character after
/// a double-width one lands at the column this crate's [`display_width`]
/// predicts. The crate is a dev-dependency only — no TUI ships in this pass.
#[test]
fn adr_0002_tui_crate_handles_east_asian_double_width() {
    let buffer = render_to_buffer("漢字");

    // The first kanji starts at column 0 and owns column 1 as a continuation:
    // ratatui blanks the cell rather than putting half a character in it.
    assert_eq!(buffer[0], "漢");
    assert_eq!(buffer[1], " ", "x=1 should be a continuation cell");
    // The second kanji therefore starts at column 2, not column 1 — which is
    // where a renderer counting characters instead of columns would put it.
    assert_eq!(buffer[2], "字");
    assert_eq!(buffer[3], " ");

    // And the width ratatui computed is the width this crate computes: the
    // marker after the text lands exactly display_width() columns along.
    assert_eq!(marker_column("漢字"), display_width("漢字"));
    assert_eq!(marker_column("漢字"), 4);
}

/// The same agreement across the writing systems in the target locales, so the
/// check is about the width algorithm rather than one lucky character.
#[test]
fn the_tui_crate_and_this_renderer_agree_on_mixed_text() {
    for text in [
        "日本語のパッケージ",
        "软件包",
        "Ελληνικά",
        "wget",
        "a漢b字c",
        "",
    ] {
        assert_eq!(
            marker_column(text),
            display_width(text),
            "{text:?}: ratatui and this crate disagree on the width"
        );
    }
}

/// The symbols ratatui puts in each cell of a one-row buffer.
fn render_to_buffer(text: &str) -> Vec<String> {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::text::Line;
    use ratatui::widgets::{Paragraph, Widget};

    let area = Rect::new(0, 0, 40, 1);
    let mut buffer = Buffer::empty(area);
    Paragraph::new(Line::from(text)).render(area, &mut buffer);
    (0..area.width)
        .map(|x| buffer[(x, 0)].symbol().to_string())
        .collect()
}

/// The column ratatui places a marker at when it follows `text` — that is,
/// the width ratatui believes `text` to have.
fn marker_column(text: &str) -> usize {
    let marker = "|";
    let buffer = render_to_buffer(&format!("{text}{marker}"));
    buffer
        .iter()
        .position(|symbol| symbol == marker)
        .expect("the marker must be somewhere in the buffer")
}
