// SPDX-License-Identifier: Apache-2.0

//! The text renderer: display-width measurement, aligned columns, and colour.
//!
//! ```
//! use macportsbrew::render::{Style, Table};
//!
//! let mut table = Table::new(["Package", "Provider"]);
//! table.push(["wget", "homebrew"]);
//! table.push(["日本語のパッケージ", "macports"]);
//!
//! // Both rows put the provider in the same column, because the widths are
//! // measured in terminal columns rather than characters.
//! assert_eq!(table.column_offsets(None), vec![0, 20]);
//! assert!(!Style::plain().error("x").contains('\u{1b}'));
//! ```

pub mod style;
pub mod table;
pub mod width;

pub use style::Style;
pub use table::Table;
pub use width::{display_width, pad_to, truncate_to};
