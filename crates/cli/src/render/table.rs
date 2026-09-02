// SPDX-License-Identifier: Apache-2.0

//! Plain aligned columns.
//!
//! The output contract (`docs/spec/06-cli-and-ux.md`) says non-TTY output is
//! plain, unstyled and stable, so this renderer draws no boxes, emits no
//! escape sequences and produces the same bytes for the same input on every
//! run. Columns are separated by two spaces and nothing else; trailing spaces
//! are trimmed, so a line's bytes do not depend on the width of a column to
//! its right that happened to be empty.
//!
//! Every measurement goes through [`super::width`], which is what makes a
//! Japanese package name line up with an ASCII one.

use super::width::{display_width, pad_to, truncate_to};

/// The gap between two columns.
pub const GUTTER: &str = "  ";

/// The narrowest a column is allowed to become when a table is squeezed: wide
/// enough for the truncation marker, so a squeezed cell still says it was cut.
pub const MIN_COLUMN: usize = super::width::ELLIPSIS_WIDTH;

/// A table of text, laid out in display width.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    /// The column headings. Their number fixes the number of columns.
    pub headers: Vec<String>,
    /// The rows. A row with too few cells is padded with empty ones and a row
    /// with too many has the extras dropped, so a malformed row cannot shift
    /// the columns of the rows around it.
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// A table with these headings and no rows.
    pub fn new<I, S>(headers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
        }
    }

    /// Appends a row.
    pub fn push<I, S>(&mut self, row: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rows.push(row.into_iter().map(Into::into).collect());
    }

    /// The number of columns.
    pub fn columns(&self) -> usize {
        self.headers.len()
    }

    /// The width in columns of each column.
    ///
    /// Without `max_width` this is the widest cell in each column. With one,
    /// the widest column is shaved a column at a time until the whole table
    /// fits, which spends the squeeze on the column that has most to give.
    /// No column goes below [`MIN_COLUMN`]: past that point the text is gone
    /// anyway, so a table that cannot fit overflows visibly rather than
    /// degrading into a column of markers.
    pub fn column_widths(&self, max_width: Option<usize>) -> Vec<usize> {
        let mut widths: Vec<usize> = self.headers.iter().map(|h| display_width(h)).collect();
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate().take(widths.len()) {
                widths[index] = widths[index].max(display_width(cell));
            }
        }
        if let Some(limit) = max_width {
            shrink(&mut widths, limit);
        }
        widths
    }

    /// The column each column starts at, measured in display width from the
    /// start of the line. This is what a test asserts against when it checks
    /// that a Japanese cell and an ASCII cell line up.
    pub fn column_offsets(&self, max_width: Option<usize>) -> Vec<usize> {
        let gutter = display_width(GUTTER);
        let mut offset = 0;
        let mut offsets = Vec::with_capacity(self.headers.len());
        for width in self.column_widths(max_width) {
            offsets.push(offset);
            offset += width + gutter;
        }
        offsets
    }

    /// Renders the table: the heading line, then one line per row, joined by
    /// newlines and with no trailing newline.
    pub fn render(&self, max_width: Option<usize>) -> String {
        if self.headers.is_empty() {
            return String::new();
        }
        let widths = self.column_widths(max_width);
        let mut out = render_row(&self.headers, &widths);
        for row in &self.rows {
            out.push('\n');
            out.push_str(&render_row(row, &widths));
        }
        out
    }
}

/// Lays one row out against the column widths.
fn render_row(cells: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            line.push_str(GUTTER);
        }
        let cell = cells.get(index).map_or("", String::as_str);
        line.push_str(&pad_to(&truncate_to(cell, *width), *width));
    }
    // Trailing spaces carry no information and make output diffs noisy.
    while line.ends_with(' ') {
        line.pop();
    }
    line
}

/// Shaves the widest column until the table fits, or until nothing can give.
fn shrink(widths: &mut [usize], limit: usize) {
    if widths.is_empty() {
        return;
    }
    let gutter = display_width(GUTTER) * (widths.len() - 1);
    loop {
        let total: usize = widths.iter().sum::<usize>() + gutter;
        if total <= limit {
            return;
        }
        // First among equals, so the result does not depend on iteration luck.
        let Some(widest) = (0..widths.len()).max_by_key(|&i| (widths[i], usize::MAX - i)) else {
            return;
        };
        if widths[widest] <= MIN_COLUMN {
            return;
        }
        widths[widest] -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::width::display_width;

    fn sample() -> Table {
        let mut table = Table::new(["Package", "Provider"]);
        table.push(["wget", "homebrew"]);
        table.push(["日本語のパッケージ", "macports"]);
        table
    }

    #[test]
    fn columns_are_sized_by_the_widest_cell_in_display_width() {
        let widths = sample().column_widths(None);
        // "日本語のパッケージ" is eighteen columns, not nine characters.
        assert_eq!(widths, vec![18, 8]);
    }

    #[test]
    fn a_double_width_cell_and_an_ascii_cell_start_at_the_same_column() {
        let table = sample();
        let rendered = table.render(None);
        let offsets = table.column_offsets(None);
        assert_eq!(offsets, vec![0, 20]);
        for line in rendered.lines() {
            let (first, _) = split_at_column(line, offsets[1]);
            assert_eq!(
                display_width(first),
                offsets[1],
                "second column does not start at {} in {line:?}",
                offsets[1]
            );
        }
    }

    /// Splits a line at a display-width offset, so a test can talk about
    /// columns rather than bytes.
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

    #[test]
    fn the_rendered_shape_is_stable() {
        let rendered = sample().render(None);
        // Spelled out from the column widths rather than typed as a literal:
        // a hand-counted run of spaces is exactly the kind of thing this
        // renderer exists to stop anyone having to do.
        let expected = format!(
            "Package{pad7}{gap}Provider\n\
             wget{pad4}{gap}homebrew\n\
             日本語のパッケージ{gap}macports",
            gap = GUTTER,
            pad7 = " ".repeat(18 - display_width("Package")),
            pad4 = " ".repeat(18 - display_width("wget")),
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn every_line_is_the_same_display_width_up_to_trailing_space() {
        let mut table = Table::new(["A", "B"]);
        table.push(["漢字", "x"]);
        table.push(["ab", "yy"]);
        let rendered = table.render(None);
        let widths: Vec<usize> = rendered.lines().map(display_width).collect();
        // Column one is four columns wide; the second column starts at six.
        assert_eq!(table.column_offsets(None), vec![0, 6]);
        assert!(widths.iter().all(|w| *w <= 8), "{widths:?}");
    }

    #[test]
    fn trailing_space_is_trimmed() {
        let mut table = Table::new(["A", "B"]);
        table.push(["long-value", ""]);
        for line in table.render(None).lines() {
            assert!(!line.ends_with(' '), "{line:?}");
        }
    }

    #[test]
    fn a_short_row_is_padded_and_a_long_row_is_clipped() {
        let mut table = Table::new(["A", "B"]);
        table.push(["only-one"]);
        table.push(["a", "b", "c"]);
        let rendered = table.render(None);
        assert!(!rendered.contains('c'), "{rendered}");
        assert_eq!(rendered.lines().count(), 3);
    }

    #[test]
    fn a_width_limit_squeezes_the_widest_column_first() {
        let table = sample();
        let widths = table.column_widths(Some(20));
        assert_eq!(widths.iter().sum::<usize>() + 2, 20);
        // The eight-column provider name is left alone until the wide one has
        // been brought down to it.
        assert_eq!(widths[1], 8);
        assert_eq!(widths[0], 10);
    }

    #[test]
    fn a_squeezed_table_never_renders_wider_than_its_limit() {
        let table = sample();
        for limit in 8..40 {
            for line in table.render(Some(limit)).lines() {
                assert!(
                    display_width(line) <= limit.max(MIN_COLUMN * 2 + 2),
                    "line {line:?} exceeds {limit}"
                );
            }
        }
    }

    #[test]
    fn a_limit_that_cannot_be_met_stops_at_the_minimum() {
        let table = sample();
        let widths = table.column_widths(Some(1));
        assert_eq!(widths, vec![MIN_COLUMN, MIN_COLUMN]);
    }

    #[test]
    fn a_table_with_no_headers_renders_nothing() {
        assert_eq!(Table::default().render(None), "");
    }

    #[test]
    fn a_table_with_no_rows_renders_only_its_headings() {
        assert_eq!(Table::new(["A", "B"]).render(None), "A  B");
    }
}
