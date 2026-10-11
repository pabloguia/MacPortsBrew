// SPDX-License-Identifier: Apache-2.0

//! Parsers for `port`'s line-oriented output.
//!
//! MacPorts has no structured output mode at all (R-3, Finding 1: asked for
//! every occurrence of "json" in `port(1)`, the answer was none), so every
//! read here is text a parser must assert the shape of. Each parser fails
//! with [`ErrorId::ProviderOutputChanged`] rather than guessing
//! (`07-quality-and-testing.md`: parsers assert shape and fail loudly).
//!
//! **MacPorts is not installed on the machine this adapter was written on.**
//! Every shape below is taken from the documentation R-3 cites, never from an
//! observation, and the ones R-3 itself marks "needs observation" carry an
//! `UNVERIFIED (R-3)` comment here. They are the first thing to re-check on a
//! machine that has MacPorts.

use super::names::{bounded_text, is_safe_name, within_output_bound, MAX_ROWS};
use macportsbrew_domain::{DomainError, ErrorId};
use std::collections::{BTreeMap, BTreeSet};

/// One row of `port -q installed`: a name may have several.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledRow {
    /// Port name.
    pub name: String,
    /// Upstream version, without the MacPorts revision.
    pub version: String,
    /// MacPorts revision, when the tuple carried one.
    pub revision: Option<String>,
    /// Variants exactly as printed, sign included (`+universal`, `-x11`).
    pub variants: BTreeSet<String>,
    /// Whether this row is the active image.
    pub active: bool,
}

/// The homepage and licence of one port, read in one invocation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexOrigin {
    /// Upstream homepage, when the record carried a URL.
    pub homepage: Option<String>,
    /// Licence expression exactly as the port index states it.
    pub license: Option<String>,
}

/// The three single-token metadata fields read in one invocation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexVersion {
    /// Version in the port index.
    pub version: String,
    /// Revision in the port index.
    pub revision: String,
    /// The port the tree says replaces this one, when any.
    pub replaced_by: Option<String>,
}

fn shape(context: &str, line: &str) -> DomainError {
    DomainError::new(
        ErrorId::ProviderOutputChanged,
        format!(
            "macports {context}: unexpected line shape `{}`",
            bounded_text(line)
        ),
    )
}

fn bounds(context: &str) -> DomainError {
    DomainError::new(
        ErrorId::ProviderOutputChanged,
        format!("macports {context}: output exceeds the parser's bound"),
    )
}

fn rows<'a>(stream: &'a str, context: &str) -> Result<Vec<&'a str>, DomainError> {
    if !within_output_bound(stream) {
        return Err(bounds(context));
    }
    let out: Vec<&str> = stream
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if out.len() > MAX_ROWS {
        return Err(bounds(context));
    }
    Ok(out)
}

/// Parses the release number out of `port version`.
///
/// UNVERIFIED (R-3): R-3 cites `port(1)` for what the action *does* ("Display
/// the release number of the installed MacPorts infrastructure") and the
/// Guide for a rendered version string, but not for the exact line. Both the
/// labelled form and a bare number are therefore accepted, and the first
/// dotted numeric run wins.
pub fn parse_version(stdout: &str) -> Result<String, DomainError> {
    let line = rows(stdout, "version")?
        .first()
        .copied()
        .ok_or_else(|| shape("version", stdout))?;
    let body = line.split(':').next_back().unwrap_or(line);
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let candidate = body[start..i].trim_end_matches('.');
            if candidate.split('.').filter(|p| !p.is_empty()).count() >= 2 {
                return Ok(candidate.to_string());
            }
        } else {
            i += 1;
        }
    }
    Err(shape("version", line))
}

/// Splits `@version_revision+variants` into its parts.
///
/// The variant tail is a run of `+name` or `-name` items whose names cannot
/// contain `+` or `-`: `port lint` refuses any variant name outside
/// `[A-Za-z0-9_.]+` (`macports-base`, `src/port1.0/portlint_run.tcl`, read
/// 2026-10-11). A name can still end in `_<digits>` (`+perl5_34`), so the
/// revision is the *first* `_<digits>` run that ends at the end of the token
/// or at a `+`/`-` sign and leaves a well-formed variant tail behind it. A
/// later run of the same shape lies inside a variant name, never in front of
/// the tail.
///
/// UNVERIFIED (R-3, "exact bytes of `port -q installed`"): the same lint file
/// does not restrict the characters of a version, so a version containing
/// `_<digits>-` (say `1_2-rc` at revision 0, printed `@1_2-rc_0`) reads as
/// version `1`, revision `2` and a negated variant `-rc_0`. No port in the
/// captured recordings has such a version. A tuple with no revision is split
/// at the first `+` only, because a leading `-` cannot be told from a
/// hyphenated version without one.
fn split_tuple(
    tuple: &str,
    line: &str,
) -> Result<(String, Option<String>, BTreeSet<String>), DomainError> {
    let body = tuple
        .strip_prefix('@')
        .ok_or_else(|| shape("installed", line))?;
    if body.is_empty() {
        return Err(shape("installed", line));
    }
    let bytes = body.as_bytes();
    let mut cut: Option<(usize, usize)> = None;
    for (i, b) in bytes.iter().enumerate() {
        if *b != b'_' {
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        let ends_at_sign = j == bytes.len() || bytes[j] == b'+' || bytes[j] == b'-';
        if j > i + 1 && ends_at_sign && parse_variants(&body[j..], line).is_ok() {
            cut = Some((i, j));
            break;
        }
    }
    let (version, revision, rest) = if let Some((i, j)) = cut {
        (&body[..i], Some(body[i + 1..j].to_string()), &body[j..])
    } else if let Some(i) = body.find('+') {
        (&body[..i], None, &body[i..])
    } else {
        (body, None, "")
    };
    if version.is_empty() {
        return Err(shape("installed", line));
    }
    Ok((version.to_string(), revision, parse_variants(rest, line)?))
}

fn parse_variants(rest: &str, line: &str) -> Result<BTreeSet<String>, DomainError> {
    let mut out = BTreeSet::new();
    let bytes = rest.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if bytes[start] != b'+' && bytes[start] != b'-' {
            return Err(shape("installed", line));
        }
        let mut end = start + 1;
        while end < bytes.len() && bytes[end] != b'+' && bytes[end] != b'-' {
            end += 1;
        }
        if !is_safe_name(&rest[start + 1..end]) {
            return Err(shape("installed", line));
        }
        out.insert(rest[start..end].to_string());
        start = end;
    }
    Ok(out)
}

/// Parses `port -q installed`.
///
/// The documented line is `  name @version_revision+variants (active)` with
/// the header suppressed by `-q` (R-3 E-1). A line with no `@` tuple is a
/// shape change, which includes the header line: if it ever appears despite
/// `-q`, this fails loudly rather than silently skipping rows.
///
/// UNVERIFIED (R-3): whether an inactive row prints nothing or an explicit
/// `(inactive)` marker. Both are accepted; anything that is not `(active)`
/// means inactive.
pub fn parse_installed(stdout: &str) -> Result<Vec<InstalledRow>, DomainError> {
    let mut out = Vec::new();
    for line in rows(stdout, "installed")? {
        let mut tokens = line.split_whitespace();
        let name = tokens.next().ok_or_else(|| shape("installed", line))?;
        if !is_safe_name(name) {
            return Err(shape("installed", line));
        }
        let tuple = tokens.next().ok_or_else(|| shape("installed", line))?;
        let (version, revision, variants) = split_tuple(tuple, line)?;
        let active = tokens.any(|t| t == "(active)");
        out.push(InstalledRow {
            name: name.to_string(),
            version,
            revision,
            variants,
            active,
        });
    }
    Ok(out)
}

/// Parses a `port -q echo <pseudo-portname>` list into a set of names.
///
/// UNVERIFIED (R-3 E-5): the documented example prints one name per line;
/// whitespace-separated names on one line are accepted too, since both
/// reduce to the same set.
pub fn parse_name_list(stdout: &str, context: &str) -> Result<BTreeSet<String>, DomainError> {
    let mut out = BTreeSet::new();
    for line in rows(stdout, context)? {
        for token in line.split_whitespace() {
            if !is_safe_name(token) {
                return Err(shape(context, line));
            }
            out.insert(token.to_string());
        }
    }
    if out.len() > MAX_ROWS {
        return Err(bounds(context));
    }
    Ok(out)
}

/// Splits a `--line` record into its leading name and the rest of the line.
fn split_named(line: &str, context: &str) -> Result<(String, String), DomainError> {
    let mut it = line.splitn(2, char::is_whitespace);
    let name = it.next().unwrap_or_default();
    if !is_safe_name(name) {
        return Err(shape(context, line));
    }
    Ok((
        name.to_string(),
        it.next().unwrap_or_default().trim().to_string(),
    ))
}

/// Parses `-q info --index --line --name --version --revision --replaced_by`.
///
/// Every field here is a single token, so the record is three tokens when the
/// port has no successor and four when it has one. That is why this field set
/// is queried on its own: it is the only grouping in which an *empty*
/// `replaced_by` stays unambiguous (R-3 E-8 documents `--line` only as
/// "Fields are separated by spaces", and the one official `--line` example
/// shows a field that itself contains a space).
pub fn parse_index_version(stdout: &str) -> Result<BTreeMap<String, IndexVersion>, DomainError> {
    let mut out = BTreeMap::new();
    for line in rows(stdout, "info-version")? {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 3 || tokens.len() > 4 || !is_safe_name(tokens[0]) {
            return Err(shape("info-version", line));
        }
        let replaced_by = match tokens.get(3) {
            Some(t) if is_safe_name(t) => Some((*t).to_string()),
            Some(_) => return Err(shape("info-version", line)),
            None => None,
        };
        out.insert(
            tokens[0].to_string(),
            IndexVersion {
                version: tokens[1].to_string(),
                revision: tokens[2].to_string(),
                replaced_by,
            },
        );
    }
    Ok(out)
}

/// Parses `-q info --index --line --name --homepage --license`.
///
/// The homepage is one token; the licence may contain spaces (`GPL-3+ or
/// LGPL-2.1+`), so it takes the remainder of the line. A record whose second
/// token is not a URL is read as having no homepage and a licence that starts
/// there — the only disambiguation available without a separator.
///
/// UNVERIFIED (R-3 E-8): whether an empty field is omitted or rendered as a
/// placeholder token.
pub fn parse_index_homepage_license(
    stdout: &str,
) -> Result<BTreeMap<String, IndexOrigin>, DomainError> {
    let mut out = BTreeMap::new();
    for line in rows(stdout, "info-homepage")? {
        let (name, rest) = split_named(line, "info-homepage")?;
        let (homepage, license) = if rest.is_empty() {
            (None, None)
        } else {
            let mut parts = rest.splitn(2, char::is_whitespace);
            let first = parts.next().unwrap_or_default();
            let tail = parts.next().unwrap_or_default().trim();
            if is_url(first) {
                (
                    Some(bounded_text(first)),
                    (!tail.is_empty()).then(|| bounded_text(tail)),
                )
            } else {
                (None, Some(bounded_text(&rest)))
            }
        };
        out.insert(name, IndexOrigin { homepage, license });
    }
    Ok(out)
}

fn is_url(token: &str) -> bool {
    token.starts_with("https://") || token.starts_with("http://") || token.starts_with("ftp://")
}

/// Parses `-q info --index --line --name --description`. The description is
/// the whole remainder of the line, which is why it is queried alone.
pub fn parse_index_description(stdout: &str) -> Result<BTreeMap<String, String>, DomainError> {
    let mut out = BTreeMap::new();
    for line in rows(stdout, "info-description")? {
        let (name, rest) = split_named(line, "info-description")?;
        if !rest.is_empty() {
            out.insert(name, bounded_text(&rest));
        }
    }
    Ok(out)
}

/// Parses one `-q info --index --line --name --depends_<kind>` invocation.
///
/// Each dependency spec is `port:<name>`, `lib:<spec>:<name>`,
/// `bin:<program>:<name>` or `path:<path>:<name>`; the port name is the last
/// colon-separated component in every form.
///
/// UNVERIFIED (R-3 E-8): whether `--line` joins a multi-valued field with
/// commas, with spaces, or both. Both separators are accepted, which makes
/// the parse independent of that answer. Each `--depends_*` field is queried
/// in its own invocation so that a record has exactly one variable-width
/// field and build-time edges stay distinguishable from runtime ones.
pub fn parse_index_depends(
    stdout: &str,
    context: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, DomainError> {
    let mut out = BTreeMap::new();
    for line in rows(stdout, context)? {
        let (name, rest) = split_named(line, context)?;
        let mut deps = BTreeSet::new();
        for token in rest.split([',', ' ', '\t']).filter(|t| !t.is_empty()) {
            let dep = token.rsplit(':').next().unwrap_or(token);
            if !is_safe_name(dep) {
                return Err(shape(context, line));
            }
            deps.insert(dep.to_string());
        }
        out.insert(name, deps);
    }
    Ok(out)
}

/// Parses `port -q provides <path>`, the authoritative file-to-port answer
/// (R-3: it reads the registry's file map).
///
/// UNVERIFIED (R-3): the exact rendering is not quoted by any source R-3
/// read. The documented human form names the port after `is provided by:`;
/// a bare port name on its own line is accepted as the `-q` form. Anything
/// else yields `None`, never a guess.
pub fn parse_provides(stdout: &str) -> Option<String> {
    if !within_output_bound(stdout) {
        return None;
    }
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some((_, tail)) = line.split_once("is provided by:") {
            let name = tail.trim();
            if is_safe_name(name) {
                return Some(name.to_string());
            }
            return None;
        }
        if is_safe_name(line) {
            return Some(line.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_accepts_the_labelled_and_the_bare_form() {
        assert_eq!(parse_version("Version: 2.12.6\n").unwrap(), "2.12.6");
        assert_eq!(parse_version("2.10.7\n").unwrap(), "2.10.7");
        assert_eq!(
            parse_version("MacPorts base version 2.12.6\n").unwrap(),
            "2.12.6"
        );
        assert!(parse_version("").is_err());
        assert!(parse_version("no numbers here\n").is_err());
    }

    #[test]
    fn installed_rows_follow_the_documented_shape() {
        // The documented sample from R-3 E-1, header removed by `-q`.
        let out = parse_installed(
            "  a52dec @0.7.4_0 (active)\n  apache2 @2.2.27_0+preforkmpm (active)\n",
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "a52dec");
        assert_eq!(out[0].version, "0.7.4");
        assert_eq!(out[0].revision.as_deref(), Some("0"));
        assert!(out[0].variants.is_empty());
        assert!(out[0].active);
        assert!(out[1].variants.contains("+preforkmpm"));
    }

    #[test]
    fn a_negated_variant_keeps_its_sign_and_does_not_eat_the_version() {
        // R-3 E-1: `libsdl @1.2.15_3-x11 (active)`.
        let out = parse_installed("  libsdl @1.2.15_3-x11 (active)\n").unwrap();
        assert_eq!(out[0].version, "1.2.15");
        assert_eq!(out[0].revision.as_deref(), Some("3"));
        assert!(out[0].variants.contains("-x11"));
    }

    #[test]
    fn a_hyphenated_version_is_not_read_as_a_variant() {
        let out = parse_installed("  snapshot @2024-01-01_0+universal\n").unwrap();
        assert_eq!(out[0].version, "2024-01-01");
        assert_eq!(out[0].revision.as_deref(), Some("0"));
        assert!(out[0].variants.contains("+universal"));
        assert!(!out[0].active);
    }

    // Synthetic rows below: shaped after the documented `name @version_revision
    // +variants` line (R-3 E-1) with variant names taken from real ports, but
    // not captured from a machine.

    #[test]
    fn a_variant_name_ending_in_underscore_digits_is_not_the_revision() {
        let out =
            parse_installed("  git @2.42.0_0+credential_osxkeychain+doc+pcre+perl5_34 (active)\n")
                .unwrap();
        assert_eq!(out[0].version, "2.42.0");
        assert_eq!(out[0].revision.as_deref(), Some("0"));
        let expected: BTreeSet<String> = ["+credential_osxkeychain", "+doc", "+pcre", "+perl5_34"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(out[0].variants, expected);
    }

    #[test]
    fn a_lone_variant_ending_in_underscore_digits_is_not_the_revision() {
        let out = parse_installed("  p5-x @1.0_2+perl5_34 (active)\n").unwrap();
        assert_eq!(out[0].version, "1.0");
        assert_eq!(out[0].revision.as_deref(), Some("2"));
        assert!(out[0].variants.contains("+perl5_34"));
        assert_eq!(out[0].variants.len(), 1);
    }

    #[test]
    fn a_negated_variant_ending_in_underscore_digits_is_not_the_revision() {
        let out = parse_installed("  x @1.2.15_3-perl5_34+doc (active)\n").unwrap();
        assert_eq!(out[0].version, "1.2.15");
        assert_eq!(out[0].revision.as_deref(), Some("3"));
        assert!(out[0].variants.contains("-perl5_34"));
        assert!(out[0].variants.contains("+doc"));
    }

    #[test]
    fn a_variant_name_ending_in_digits_without_underscore_parses() {
        let out = parse_installed("  vim @9.1.0_0+python312-x11 (active)\n").unwrap();
        assert_eq!(out[0].version, "9.1.0");
        assert_eq!(out[0].revision.as_deref(), Some("0"));
        assert!(out[0].variants.contains("+python312"));
        assert!(out[0].variants.contains("-x11"));
    }

    #[test]
    fn a_version_containing_underscores_keeps_them() {
        // `port lint` does not restrict a version's characters, so `1_2_3`
        // is a possible version; the revision is the run before the tail.
        let out = parse_installed("  x @1_2_3_0+doc (active)\n").unwrap();
        assert_eq!(out[0].version, "1_2_3");
        assert_eq!(out[0].revision.as_deref(), Some("0"));
        assert!(out[0].variants.contains("+doc"));
        let out = parse_installed("  x @1_2_3_0 (active)\n").unwrap();
        assert_eq!(out[0].version, "1_2_3");
        assert_eq!(out[0].revision.as_deref(), Some("0"));
        assert!(out[0].variants.is_empty());
    }

    #[test]
    fn a_row_without_a_version_tuple_is_a_shape_change() {
        let err = parse_installed("The following ports are currently installed:\n").unwrap_err();
        assert_eq!(err.id, ErrorId::ProviderOutputChanged);
        assert_eq!(
            parse_installed("  wget 1.25.0\n").unwrap_err().id,
            ErrorId::ProviderOutputChanged
        );
        assert_eq!(
            parse_installed("  -evil @1.0_0\n").unwrap_err().id,
            ErrorId::ProviderOutputChanged
        );
    }

    #[test]
    fn name_lists_reject_anything_that_is_not_a_port_name() {
        let set = parse_name_list("wget\ngnutls\n", "echo").unwrap();
        assert_eq!(set.len(), 2);
        assert_eq!(
            parse_name_list("wget\n--force\n", "echo").unwrap_err().id,
            ErrorId::ProviderOutputChanged
        );
    }

    #[test]
    fn index_version_records_are_three_or_four_tokens() {
        let out =
            parse_index_version("wget 1.25.0 1\npy-pygments 2.17.2 0 py312-pygments\n").unwrap();
        assert_eq!(out["wget"].version, "1.25.0");
        assert_eq!(out["wget"].revision, "1");
        assert!(out["wget"].replaced_by.is_none());
        assert_eq!(
            out["py-pygments"].replaced_by.as_deref(),
            Some("py312-pygments")
        );
        assert!(parse_index_version("wget 1.25.0\n").is_err());
        assert!(parse_index_version("wget 1.25.0 1 a b\n").is_err());
    }

    #[test]
    fn a_licence_containing_spaces_survives_because_it_is_last() {
        let out = parse_index_homepage_license(
            "libidn2 https://www.gnu.org/software/libidn/ LGPL-3+ or GPL-2+\nquiet\n",
        )
        .unwrap();
        assert_eq!(
            out["libidn2"].homepage.as_deref(),
            Some("https://www.gnu.org/software/libidn/")
        );
        assert_eq!(out["libidn2"].license.as_deref(), Some("LGPL-3+ or GPL-2+"));
        assert_eq!(out["quiet"], IndexOrigin::default());
    }

    #[test]
    fn a_record_with_no_homepage_reads_the_remainder_as_the_licence() {
        let out = parse_index_homepage_license("thing BSD-2-Clause\n").unwrap();
        assert_eq!(out["thing"].homepage, None);
        assert_eq!(out["thing"].license.as_deref(), Some("BSD-2-Clause"));
    }

    #[test]
    fn descriptions_take_the_whole_remainder() {
        let out = parse_index_description("wget Internet file retriever\nbare\n").unwrap();
        assert_eq!(out["wget"], "Internet file retriever");
        assert!(!out.contains_key("bare"));
    }

    #[test]
    fn dependency_specs_reduce_to_their_port_name() {
        let out = parse_index_depends(
            "wget port:openssl3 port:libidn2\nnginx port:zlib, path:bin/pkg-config:pkgconfig\nzlib\n",
            "depends_lib",
        )
        .unwrap();
        assert_eq!(out["wget"].len(), 2);
        assert!(out["wget"].contains("openssl3"));
        assert!(out["nginx"].contains("pkgconfig"));
        assert!(out["zlib"].is_empty());
        assert!(parse_index_depends("wget port:--force\n", "depends_lib").is_err());
    }

    #[test]
    fn provides_reads_the_port_name_and_never_guesses() {
        assert_eq!(
            parse_provides("/opt/local/bin/wget is provided by: wget\n").as_deref(),
            Some("wget")
        );
        assert_eq!(parse_provides("wget\n").as_deref(), Some("wget"));
        assert_eq!(parse_provides("is provided by: not a name\n"), None);
        assert_eq!(parse_provides(""), None);
    }
}
