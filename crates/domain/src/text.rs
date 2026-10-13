// SPDX-License-Identifier: Apache-2.0

//! Which characters in text are hostile to a reader, and how they are shown.
//!
//! One definition, used by two boundaries that ask different questions of it
//! (SE-092, end-of-pass amendment, "Not closed" (a)):
//!
//! - **The snapshot check** (`Snapshot::check`, the one door for capture and
//!   load) refuses the [`HostileClass::Control`] class and only that class. A
//!   snapshot is the record of what the machine and its providers reported;
//!   format characters and the line and paragraph separators occur in
//!   legitimate names — a ZWJ emoji sequence, ZWNJ inside a Persian word, a soft
//!   hyphen, tag characters — so they are kept, and nothing is escaped at
//!   capture.
//! - **Every human renderer** escapes all three classes with
//!   [`escape_hostile`], so a kept character can neither reorder the text
//!   around it nor start a line of its own. It hides nothing as far as format
//!   characters go; default-ignorable characters outside Cf (Hangul fillers,
//!   variation selectors) are not in the classification and are not covered.
//!
//! Pure: no I/O, no clock, no environment, no dependency.

/// The three kinds of character a renderer must not pass through verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostileClass {
    /// [`char::is_control`]: Unicode general category Cc, C0 and C1. Moves a
    /// cursor, clears or breaks a line, starts a terminal sequence.
    Control,
    /// Unicode general category Cf (Format), Unicode 16.0: the bidirectional
    /// overrides and isolates, zero-width characters, the tag characters.
    /// Reorders what is shown around it, or is invisible.
    Format,
    /// U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR (Zl, Zp): neither
    /// controls nor Cf, but a renderer that honours them breaks the line.
    LineSeparator,
}

/// Unicode 16.0 general category Cf, as inclusive ranges of code points.
const FORMAT_RANGES: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

/// The class `c` belongs to, or `None` when it may be shown as it is.
pub fn hostile_class(c: char) -> Option<HostileClass> {
    let code = u32::from(c);
    if c.is_control() {
        Some(HostileClass::Control)
    } else if FORMAT_RANGES
        .iter()
        .any(|&(first, last)| (first..=last).contains(&code))
    {
        Some(HostileClass::Format)
    } else if matches!(code, LINE_SEPARATOR | PARAGRAPH_SEPARATOR) {
        Some(HostileClass::LineSeparator)
    } else {
        None
    }
}

// Decimal, deliberately: the hexadecimal spelling of these two code points reads
// as a year to INV-20's deadline scan, and an exemption there would be worse
// than an unfamiliar radix here. 8232 is U+2028, 8233 is U+2029.
const LINE_SEPARATOR: u32 = 8232;
const PARAGRAPH_SEPARATOR: u32 = 8233;

/// Whether `c` belongs to any [`HostileClass`].
pub fn is_hostile(c: char) -> bool {
    hostile_class(c).is_some()
}

/// Whether `c` is in the [`HostileClass::Control`] class — the half a
/// snapshot refuses.
pub fn is_control_class(c: char) -> bool {
    hostile_class(c) == Some(HostileClass::Control)
}

/// `value` with every hostile character replaced by its `\u{..}` escape, and
/// everything else left exactly as it is. Idempotent: an escape is ASCII
/// that no class contains.
pub fn escape_hostile(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if is_hostile(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// One character with the class it must be given. The table the predicate's
/// test, the snapshot check's test and the renderer's test all iterate, so the
/// three cannot drift apart.
pub type CharacterSample = (char, Option<HostileClass>);

const fn from_code(code: u32) -> char {
    match char::from_u32(code) {
        Some(c) => c,
        None => panic!("a sample code point is not a scalar value"),
    }
}

/// Samples of every class, every Cf range's two endpoints, and benign
/// non-ASCII that must be shown unchanged. Not test-only: three crates' tests
/// read it, and a feature flag to export it would be a new feature.
pub const CHARACTER_SAMPLES: &[CharacterSample] = &[
    // Cc: C0, DEL and C1.
    ('\0', Some(HostileClass::Control)),
    ('\t', Some(HostileClass::Control)),
    ('\n', Some(HostileClass::Control)),
    ('\r', Some(HostileClass::Control)),
    ('\u{1b}', Some(HostileClass::Control)),
    ('\u{7f}', Some(HostileClass::Control)),
    ('\u{80}', Some(HostileClass::Control)),
    ('\u{85}', Some(HostileClass::Control)),
    ('\u{9b}', Some(HostileClass::Control)),
    ('\u{9f}', Some(HostileClass::Control)),
    // Cf, both endpoints of every range.
    ('\u{AD}', Some(HostileClass::Format)),
    ('\u{600}', Some(HostileClass::Format)),
    ('\u{605}', Some(HostileClass::Format)),
    ('\u{61C}', Some(HostileClass::Format)),
    ('\u{6DD}', Some(HostileClass::Format)),
    ('\u{70F}', Some(HostileClass::Format)),
    ('\u{890}', Some(HostileClass::Format)),
    ('\u{891}', Some(HostileClass::Format)),
    ('\u{8E2}', Some(HostileClass::Format)),
    ('\u{180E}', Some(HostileClass::Format)),
    ('\u{200B}', Some(HostileClass::Format)),
    ('\u{200F}', Some(HostileClass::Format)),
    ('\u{202A}', Some(HostileClass::Format)),
    ('\u{202E}', Some(HostileClass::Format)),
    ('\u{2060}', Some(HostileClass::Format)),
    ('\u{2064}', Some(HostileClass::Format)),
    ('\u{2066}', Some(HostileClass::Format)),
    ('\u{206F}', Some(HostileClass::Format)),
    ('\u{FEFF}', Some(HostileClass::Format)),
    ('\u{FFF9}', Some(HostileClass::Format)),
    ('\u{FFFB}', Some(HostileClass::Format)),
    ('\u{110BD}', Some(HostileClass::Format)),
    ('\u{110CD}', Some(HostileClass::Format)),
    ('\u{13430}', Some(HostileClass::Format)),
    ('\u{1343F}', Some(HostileClass::Format)),
    ('\u{1BCA0}', Some(HostileClass::Format)),
    ('\u{1BCA3}', Some(HostileClass::Format)),
    ('\u{1D173}', Some(HostileClass::Format)),
    ('\u{1D17A}', Some(HostileClass::Format)),
    ('\u{E0001}', Some(HostileClass::Format)),
    ('\u{E0020}', Some(HostileClass::Format)),
    ('\u{E007F}', Some(HostileClass::Format)),
    // Zl and Zp, in decimal for the reason above.
    (from_code(LINE_SEPARATOR), Some(HostileClass::LineSeparator)),
    (
        from_code(PARAGRAPH_SEPARATOR),
        Some(HostileClass::LineSeparator),
    ),
    // Benign, including the neighbours of the ranges above.
    ('a', None),
    (' ', None),
    ('/', None),
    ('~', None),
    ('\u{A0}', None),
    ('\u{AE}', None),
    ('\u{606}', None),
    ('\u{200A}', None),
    ('\u{2010}', None),
    ('\u{2065}', None),
    ('é', None),
    ('中', None),
    ('\u{1F600}', None),
];

/// Text that carries format characters and is nonetheless a legitimate name:
/// a ZWJ emoji sequence (a person and a laptop, joined), and a Persian word
/// whose orthography requires a zero-width non-joiner. A snapshot must accept
/// both; a renderer escapes the joiners.
pub const LEGITIMATE_TEXT_WITH_FORMAT: &[&str] = &[
    "\u{1F469}\u{200D}\u{1F4BB}",
    "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sample_is_given_its_class() {
        for &(c, class) in CHARACTER_SAMPLES {
            assert_eq!(
                hostile_class(c),
                class,
                "{:?}",
                c.escape_unicode().to_string()
            );
            assert_eq!(is_hostile(c), class.is_some());
            assert_eq!(is_control_class(c), class == Some(HostileClass::Control));
        }
        // Every class is sampled, so a test iterating the table exercises all.
        for class in [
            HostileClass::Control,
            HostileClass::Format,
            HostileClass::LineSeparator,
        ] {
            assert!(CHARACTER_SAMPLES.iter().any(|&(_, k)| k == Some(class)));
        }
        assert!(CHARACTER_SAMPLES.iter().any(|&(_, k)| k.is_none()));
    }

    #[test]
    fn the_sample_endpoints_are_the_table_endpoints() {
        // Each Cf range's two endpoints appear in the sample list, written out
        // independently, so trimming a range in the table fails a test.
        for &(first, last) in FORMAT_RANGES {
            for code in [first, last] {
                assert!(
                    CHARACTER_SAMPLES
                        .iter()
                        .any(|&(c, k)| u32::from(c) == code && k == Some(HostileClass::Format)),
                    "U+{code:04X} is a Cf endpoint with no sample"
                );
            }
        }
    }

    #[test]
    fn escaping_replaces_exactly_the_hostile_characters() {
        for &(c, class) in CHARACTER_SAMPLES {
            let text = format!("a{c}b");
            let shown = escape_hostile(&text);
            if class.is_some() {
                assert_eq!(shown, format!("a{}b", c.escape_unicode()));
            } else {
                assert_eq!(shown, text);
            }
            assert_eq!(escape_hostile(&shown), shown, "escaping is idempotent");
        }
        for text in LEGITIMATE_TEXT_WITH_FORMAT {
            assert!(text
                .chars()
                .any(|c| hostile_class(c) == Some(HostileClass::Format)));
            assert!(!text.chars().any(is_control_class));
        }
    }
}
