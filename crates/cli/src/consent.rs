// SPDX-License-Identifier: Apache-2.0

//! Asking a person for elevation consent (pass 08 slice 4, D10; INV-06).
//!
//! **Built, not wired.** No caller exists outside this module's tests: the
//! execution gate refuses every elevated step in this build, and a flip
//! tripwire (`crates/cli/tests/history_preconditions.rs`) fails the build that
//! turns mutations on while nothing calls [`ask`].
//!
//! The rules, each a review finding of the slice 4 plan (M3):
//!
//! 1. **Both streams must be terminals.** Input, and the stream the question is
//!    written to; otherwise nobody could have seen the question, so it is
//!    refused without reading a byte.
//! 2. **The question goes to the prompt stream, never stdout**, so `--json`
//!    output stays a document.
//! 3. **Input is read with a bound**, one line per operation; end of input is
//!    a refusal.
//! 4. **The confirmation is an exact ASCII phrase naming the step**, never
//!    translated and never `y`; only a trailing line ending is removed. The
//!    application's `confirm` checks it and is the only way consent is made.
//! 5. **What is shown is unambiguous.** Every value is passed through the
//!    domain's `escape_hostile`, the invoking user's home is shown as `~` (the
//!    prompt says whose home that is), each argument is on its own line in
//!    quotes, and nothing is truncated.
//! 6. **`--yes` is not a parameter** here, so it cannot stand for consent.

use crate::i18n::Messages;
use macportsbrew_application::{
    confirm, confirmation_phrase, ConsentDecision, ConsentRefusal, ElevationRequest,
};
use macportsbrew_domain::{escape_hostile, redact_home, HomeRedaction};
use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};

/// The most a confirmation line may hold; a longer one is not the phrase.
const MAX_LINE_BYTES: u64 = 256;

/// Where the question is asked and the answer read.
pub struct ConsentStreams<'a> {
    /// Whether input is a terminal.
    pub input_is_terminal: bool,
    /// Whether the stream the question is written to is a terminal.
    pub prompt_is_terminal: bool,
    /// The answers.
    pub input: &'a mut dyn BufRead,
    /// Where the question is written: standard error, never standard output.
    pub prompt: &'a mut dyn Write,
}

fn params(pairs: &[(&str, String)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

/// `value` as a prompt shows it: the home as `~`, every hostile character
/// escaped, in quotes so `a b` cannot pass for two arguments.
fn shown(value: &str, home: &HomeRedaction) -> String {
    format!("\"{}\"", escape_hostile(&redact_home(value, home)))
}

/// Asks for consent to every request, one confirmation per operation.
pub fn ask(
    requests: &[ElevationRequest],
    streams: ConsentStreams<'_>,
    messages: &Messages,
    home: &HomeRedaction,
) -> ConsentDecision {
    if requests.is_empty() {
        return ConsentDecision::Refused(ConsentRefusal::NoRequests);
    }
    if !(streams.input_is_terminal && streams.prompt_is_terminal) {
        return ConsentDecision::Refused(ConsentRefusal::NotATerminal);
    }
    let out = streams.prompt;
    let input = streams.input;
    let _ = writeln!(
        out,
        "{}",
        messages.render("consent.heading", &BTreeMap::new())
    );
    let mut given = Vec::new();
    for request in requests {
        let key = if request.known {
            "consent.request"
        } else {
            "consent.request_unknown"
        };
        let _ = writeln!(
            out,
            "{}",
            messages.render(
                key,
                &params(&[
                    ("step", request.step.to_string()),
                    ("provider", escape_hostile(request.provider.as_str())),
                ]),
            )
        );
        let _ = writeln!(
            out,
            "  {}",
            messages.render(
                "consent.program",
                &params(&[("program", shown(&request.program, home))]),
            )
        );
        for arg in &request.argv {
            let _ = writeln!(out, "    {}", shown(arg, home));
        }
        let _ = write!(
            out,
            "{} ",
            messages.render(
                "consent.type_to_allow",
                &params(&[("phrase", confirmation_phrase(request))]),
            )
        );
        let _ = out.flush();
        let mut line = String::new();
        let read = (&mut *input).take(MAX_LINE_BYTES).read_line(&mut line);
        let consent = match read {
            Ok(0) | Err(_) => None,
            Ok(_) => confirm(request, &line),
        };
        let Some(token) = consent else {
            let _ = writeln!(
                out,
                "{}",
                messages.render("consent.declined", &BTreeMap::new())
            );
            return ConsentDecision::Refused(ConsentRefusal::Declined);
        };
        given.push(token);
    }
    ConsentDecision::Given(given)
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::ProviderId;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn messages() -> Messages {
        Messages::for_locale(&crate::i18n::resolve(Some("en"), None))
    }

    fn request(step: u32) -> ElevationRequest {
        ElevationRequest {
            step,
            provider: ProviderId::new("alpha"),
            program: "/Users/ada/opt/bin/tool".into(),
            argv: vec!["install".into(), "a b\u{1b}[2J".into(), "x".repeat(400)],
            known: true,
        }
    }

    fn home() -> HomeRedaction {
        HomeRedaction::Known(PathBuf::from("/Users/ada"))
    }

    fn asked(
        requests: &[ElevationRequest],
        terminals: (bool, bool),
        typed: &str,
    ) -> (ConsentDecision, String, u64) {
        let mut input = Cursor::new(typed.as_bytes().to_vec());
        let mut prompt = Vec::new();
        let decision = ask(
            requests,
            ConsentStreams {
                input_is_terminal: terminals.0,
                prompt_is_terminal: terminals.1,
                input: &mut input,
                prompt: &mut prompt,
            },
            &messages(),
            &home(),
        );
        (
            decision,
            String::from_utf8(prompt).expect("utf-8"),
            input.position(),
        )
    }

    #[test]
    fn without_two_terminals_nothing_is_read_and_nothing_is_given() {
        for terminals in [(false, true), (true, false), (false, false)] {
            let (decision, prompt, read) = asked(&[request(3)], terminals, "allow step 3\n");
            assert_eq!(
                decision,
                ConsentDecision::Refused(ConsentRefusal::NotATerminal)
            );
            assert_eq!(read, 0, "nothing was read");
            assert!(prompt.is_empty(), "nothing was asked");
        }
    }

    #[test]
    fn only_the_exact_phrase_per_operation_is_consent() {
        let requests = [request(3), request(5)];
        let (decision, _, _) = asked(&requests, (true, true), "allow step 3\nallow step 5\n");
        let ConsentDecision::Given(tokens) = decision else {
            panic!("given");
        };
        assert_eq!(
            tokens
                .iter()
                .map(macportsbrew_application::ConsentGiven::step)
                .collect::<Vec<_>>(),
            [3, 5]
        );
        for typed in [
            "y\n",
            "\n",
            "",
            "allow step 3\n",
            "allow step 3\nallow step 3\n",
            "ALLOW STEP 3\nallow step 5\n",
            "autoriza etapa 3\n",
        ] {
            let (decision, prompt, _) = asked(&requests, (true, true), typed);
            assert_eq!(
                decision,
                ConsentDecision::Refused(ConsentRefusal::Declined),
                "{typed:?}"
            );
            assert!(
                prompt.contains("declined") || prompt.contains("Nothing"),
                "{prompt}"
            );
        }
        let (decision, _, _) = asked(&[], (true, true), "allow step 3\n");
        assert_eq!(
            decision,
            ConsentDecision::Refused(ConsentRefusal::NoRequests)
        );
    }

    #[test]
    fn a_line_longer_than_the_bound_is_not_the_phrase() {
        let long = format!("allow step 3{}\n", " ".repeat(400));
        let (decision, _, read) = asked(&[request(3)], (true, true), &long);
        assert_eq!(decision, ConsentDecision::Refused(ConsentRefusal::Declined));
        assert!(read <= MAX_LINE_BYTES);
    }

    #[test]
    fn what_is_shown_is_unambiguous_escaped_untruncated_and_without_the_home() {
        let (_, prompt, _) = asked(&[request(3)], (true, true), "allow step 3\n");
        assert!(!prompt.contains("/Users/ada"), "{prompt}");
        assert!(prompt.contains("\"~/opt/bin/tool\""), "{prompt}");
        assert!(!prompt.contains('\u{1b}'), "{prompt:?}");
        assert!(prompt.contains("\"a b\\u{1b}[2J\""), "{prompt}");
        assert!(prompt.contains(&"x".repeat(400)), "not truncated");
        assert!(prompt.contains("allow step 3"), "the phrase is shown");
    }
}
