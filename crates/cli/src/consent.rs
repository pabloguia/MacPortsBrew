// SPDX-License-Identifier: Apache-2.0

//! Asking a person for elevation consent (pass 08 slice 4, D10; INV-06).
//!
//! **Elevation: built, not wired.** No caller of [`ask`] exists outside this
//! module's tests: the execution gate refuses every elevated step in this
//! build, and a flip tripwire (`crates/cli/tests/history_preconditions.rs`)
//! fails the build that turns mutations on while nothing calls it.
//!
//! **The plan: wired** (work order 16 part E-2). [`confirm_plan`] is called by
//! `migrate --yes` under a mutation allowance, which this build never grants,
//! so it is unreachable in a shipped build until the switch is flipped.
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
//! 4. **The confirmation is an exact ASCII phrase naming the step and a code
//!    drawn for that question alone** (SR 10), never translated and never
//!    `y`; only a trailing line ending is removed. The application's `confirm`
//!    checks it and is the only way consent is made. A line typed before the
//!    question cannot carry a code nobody had seen; it is read, does not match,
//!    and the answer is a refusal. With no code, nothing is asked.
//! 5. **What is shown is unambiguous.** Every value is passed through the
//!    domain's `escape_hostile`, the invoking user's home is shown as `~` (the
//!    prompt says whose home that is), each argument is on its own line in
//!    quotes, and nothing is truncated.
//! 6. **`--yes` is not a parameter** here, so it cannot stand for consent.

use crate::i18n::Messages;
use macportsbrew_application::{
    confirm, confirm_shown, confirmation_phrase, plan_phrase, ConsentCode, ConsentDecision,
    ConsentRefusal, ElevationRequest, GateClear, ShownPlan,
};
use macportsbrew_domain::MigrationPlan;
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
    /// Three fresh bytes for each question's code, drawn before that question
    /// is written; `None` and nothing is asked (SR 10).
    pub code: &'a mut dyn FnMut() -> Option<[u8; 3]>,
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
    let draw = streams.code;
    // The first question's code is drawn before anything is written, so a
    // missing code asks nothing at all.
    let mut next = draw().map(ConsentCode::from_bytes);
    let mut given = Vec::new();
    for (i, request) in requests.iter().enumerate() {
        if i > 0 {
            next = draw().map(ConsentCode::from_bytes);
        }
        let Some(code) = next.take() else {
            let _ = writeln!(
                out,
                "{}",
                messages.render("consent.code_unavailable", &BTreeMap::new())
            );
            return ConsentDecision::Refused(ConsentRefusal::CodeUnavailable);
        };
        if i == 0 {
            let _ = writeln!(
                out,
                "{}",
                messages.render("consent.heading", &BTreeMap::new())
            );
        }
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
                &params(&[(
                    "phrase",
                    escape_hostile(&confirmation_phrase(request, &code))
                )]),
            )
        );
        let _ = out.flush();
        let mut line = String::new();
        let read = (&mut *input).take(MAX_LINE_BYTES).read_line(&mut line);
        let consent = match read {
            Ok(0) | Err(_) => None,
            Ok(_) => confirm(request, &code, &line),
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

/// Shows the plan the gate cleared and asks a person to confirm it (work order
/// 16 part E, DP-049 (b)): the interactive route to a [`ShownPlan`].
///
/// The plan is rendered here, by `render`, from the very value the witness is
/// minted from, so what is shown and what is confirmed cannot be two plans.
/// Both streams must be terminals (otherwise nothing is read or written); a
/// fresh code is drawn before anything is written (none: nothing is asked);
/// the plan goes to the prompt stream, never stdout; one bounded line is read.
///
/// # Errors
/// [`ConsentRefusal::NotATerminal`], [`ConsentRefusal::CodeUnavailable`], or
/// [`ConsentRefusal::Declined`] for anything but the exact phrase.
pub fn confirm_plan(
    cleared: &GateClear,
    render: &dyn Fn(&MigrationPlan) -> String,
    streams: ConsentStreams<'_>,
    messages: &Messages,
) -> Result<ShownPlan, ConsentRefusal> {
    if !(streams.input_is_terminal && streams.prompt_is_terminal) {
        return Err(ConsentRefusal::NotATerminal);
    }
    let out = streams.prompt;
    let Some(code) = (streams.code)().map(ConsentCode::from_bytes) else {
        let _ = writeln!(
            out,
            "{}",
            messages.render("consent.code_unavailable", &BTreeMap::new())
        );
        return Err(ConsentRefusal::CodeUnavailable);
    };
    let _ = write!(out, "{}", render(cleared.run().plan()));
    let _ = write!(
        out,
        "{} ",
        messages.render(
            "consent.type_to_run_plan",
            &params(&[("phrase", escape_hostile(&plan_phrase(&code)))]),
        )
    );
    let _ = out.flush();
    let mut line = String::new();
    let read = (&mut *streams.input)
        .take(MAX_LINE_BYTES)
        .read_line(&mut line);
    let shown = match read {
        Ok(0) | Err(_) => None,
        Ok(_) => confirm_shown(cleared, &code, &line),
    };
    shown.ok_or_else(|| {
        let _ = writeln!(
            out,
            "{}",
            messages.render("consent.declined", &BTreeMap::new())
        );
        ConsentRefusal::Declined
    })
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
            argv: vec!["verb".into(), "a b\u{1b}[2J".into(), "x".repeat(400)],
            known: true,
        }
    }

    fn home() -> HomeRedaction {
        HomeRedaction::Known(PathBuf::from("/Users/ada"))
    }

    /// The codes the test seam hands out, in order: `ab cd 0n` for the n-th
    /// draw, so each question's phrase is predictable.
    fn nth_code(n: u8) -> String {
        format!("abcd{n:02x}")
    }

    struct Asked {
        decision: ConsentDecision,
        prompt: String,
        read: u64,
        draws: u8,
    }

    fn asked_with(
        requests: &[ElevationRequest],
        terminals: (bool, bool),
        typed: &str,
        codes: bool,
    ) -> Asked {
        let mut input = Cursor::new(typed.as_bytes().to_vec());
        let mut prompt = Vec::new();
        let mut draws = 0u8;
        let mut code = || {
            let n = draws;
            draws += 1;
            codes.then_some([0xab, 0xcd, n])
        };
        let decision = ask(
            requests,
            ConsentStreams {
                input_is_terminal: terminals.0,
                prompt_is_terminal: terminals.1,
                input: &mut input,
                prompt: &mut prompt,
                code: &mut code,
            },
            &messages(),
            &home(),
        );
        Asked {
            decision,
            prompt: String::from_utf8(prompt).expect("utf-8"),
            read: input.position(),
            draws,
        }
    }

    fn asked(requests: &[ElevationRequest], terminals: (bool, bool), typed: &str) -> Asked {
        asked_with(requests, terminals, typed, true)
    }

    #[test]
    fn without_two_terminals_nothing_is_read_and_nothing_is_given() {
        for terminals in [(false, true), (true, false), (false, false)] {
            let typed = format!("allow step 3 {}\n", nth_code(0));
            let a = asked(&[request(3)], terminals, &typed);
            assert_eq!(
                a.decision,
                ConsentDecision::Refused(ConsentRefusal::NotATerminal)
            );
            assert_eq!(a.read, 0, "nothing was read");
            assert!(a.prompt.is_empty(), "nothing was asked");
            assert_eq!(a.draws, 0, "no code was drawn");
        }
    }

    #[test]
    fn only_the_exact_phrase_per_operation_is_consent() {
        let requests = [request(3), request(5)];
        let typed = format!(
            "allow step 3 {}\nallow step 5 {}\n",
            nth_code(0),
            nth_code(1)
        );
        let ConsentDecision::Given(tokens) = asked(&requests, (true, true), &typed).decision else {
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
            "y\n".to_string(),
            "\n".to_string(),
            String::new(),
            "allow step 3\nallow step 5\n".to_string(),
            format!("allow step 3 {}\n", nth_code(0)),
            format!("allow step 3 {0}\nallow step 3 {0}\n", nth_code(0)),
            format!(
                "ALLOW STEP 3 {}\nallow step 5 {}\n",
                nth_code(0),
                nth_code(1)
            ),
            format!(
                "allow step 3 {}\nallow step 5 {}\n",
                nth_code(0).to_uppercase(),
                nth_code(1)
            ),
            format!("autoriza etapa 3 {}\n", nth_code(0)),
        ] {
            let a = asked(&requests, (true, true), &typed);
            assert_eq!(
                a.decision,
                ConsentDecision::Refused(ConsentRefusal::Declined),
                "{typed:?}"
            );
            assert!(a.prompt.contains("Not allowed"), "{}", a.prompt);
        }
        let a = asked(&[], (true, true), "allow step 3\n");
        assert_eq!(
            a.decision,
            ConsentDecision::Refused(ConsentRefusal::NoRequests)
        );
    }

    #[test]
    fn a_line_typed_before_the_question_is_never_consent() {
        // SR 10: what was typed before the question could only be the old,
        // code-less phrase, or a guess; the right code is drawn after.
        let a = asked(&[request(3)], (true, true), "allow step 3\n");
        assert_eq!(
            a.decision,
            ConsentDecision::Refused(ConsentRefusal::Declined)
        );
        // A guessed code that is not the one drawn.
        let a = asked(&[request(3)], (true, true), "allow step 3 000000\n");
        assert_eq!(
            a.decision,
            ConsentDecision::Refused(ConsentRefusal::Declined)
        );
    }

    #[test]
    fn each_question_has_its_own_code() {
        let requests = [request(3), request(5)];
        let typed = format!("allow step 3 {0}\nallow step 5 {0}\n", nth_code(0));
        let a = asked(&requests, (true, true), &typed);
        assert_eq!(a.draws, 2, "one draw per question");
        assert_eq!(
            a.decision,
            ConsentDecision::Refused(ConsentRefusal::Declined),
            "the first question's code does not answer the second"
        );
        assert!(a.prompt.contains(&format!("allow step 3 {}", nth_code(0))));
        assert!(a.prompt.contains(&format!("allow step 5 {}", nth_code(1))));
    }

    #[test]
    fn without_a_code_nothing_is_asked() {
        let a = asked_with(&[request(3)], (true, true), "allow step 3\n", false);
        assert_eq!(
            a.decision,
            ConsentDecision::Refused(ConsentRefusal::CodeUnavailable)
        );
        assert_eq!(a.read, 0, "nothing was read");
        assert!(
            !a.prompt.contains("administrator"),
            "no question: {}",
            a.prompt
        );
        assert!(a.prompt.contains("could not be drawn"), "{}", a.prompt);
    }

    #[test]
    fn a_line_longer_than_the_bound_is_not_the_phrase() {
        let long = format!("allow step 3 {}{}\n", nth_code(0), " ".repeat(400));
        let a = asked(&[request(3)], (true, true), &long);
        assert_eq!(
            a.decision,
            ConsentDecision::Refused(ConsentRefusal::Declined)
        );
        assert!(a.read <= MAX_LINE_BYTES);
    }

    #[test]
    fn what_is_shown_is_unambiguous_escaped_untruncated_and_without_the_home() {
        let typed = format!("allow step 3 {}\n", nth_code(0));
        let prompt = asked(&[request(3)], (true, true), &typed).prompt;
        assert!(!prompt.contains("/Users/ada"), "{prompt}");
        assert!(prompt.contains("\"~/opt/bin/tool\""), "{prompt}");
        assert!(!prompt.contains('\u{1b}'), "{prompt:?}");
        assert!(prompt.contains("\"a b\\u{1b}[2J\""), "{prompt}");
        assert!(prompt.contains(&"x".repeat(400)), "not truncated");
        assert!(
            prompt.contains(&format!("allow step 3 {}", nth_code(0))),
            "the phrase with its code is shown"
        );
    }

    /// What happened, in order, across the seam, the prompt and the input.
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum Event {
        Draw,
        Wrote(String),
        Read,
    }

    type Log = std::rc::Rc<std::cell::RefCell<Vec<Event>>>;

    struct LoggedPrompt(Log);

    impl Write for LoggedPrompt {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .borrow_mut()
                .push(Event::Wrote(String::from_utf8_lossy(buf).into_owned()));
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct LoggedInput(Log, Cursor<Vec<u8>>);

    impl Read for LoggedInput {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().push(Event::Read);
            self.1.read(buf)
        }
    }

    impl BufRead for LoggedInput {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            self.0.borrow_mut().push(Event::Read);
            self.1.fill_buf()
        }
        fn consume(&mut self, amount: usize) {
            self.1.consume(amount);
        }
    }

    #[test]
    fn each_code_is_drawn_before_its_question_and_shown_before_the_answer_is_read() {
        let log: Log = Log::default();
        let typed = format!(
            "allow step 3 {}\nallow step 5 {}\n",
            nth_code(0),
            nth_code(1)
        );
        let mut input = LoggedInput(log.clone(), Cursor::new(typed.into_bytes()));
        let mut prompt = LoggedPrompt(log.clone());
        let mut n = 0u8;
        let draw_log = log.clone();
        let mut code = move || {
            draw_log.borrow_mut().push(Event::Draw);
            n += 1;
            Some([0xab, 0xcd, n - 1])
        };
        let decision = ask(
            &[request(3), request(5)],
            ConsentStreams {
                input_is_terminal: true,
                prompt_is_terminal: true,
                input: &mut input,
                prompt: &mut prompt,
                code: &mut code,
            },
            &messages(),
            &home(),
        );
        assert!(
            matches!(decision, ConsentDecision::Given(_)),
            "{decision:?}"
        );
        let events = log.borrow().clone();
        let at = |p: &dyn Fn(&Event) -> bool, from: usize| {
            events[from..]
                .iter()
                .position(p)
                .map(|i| i + from)
                .expect("the event happens")
        };
        let mut from = 0;
        for (step, k) in [(3u32, 0u8), (5, 1)] {
            let draw = at(&|e| *e == Event::Draw, from);
            let first_write = at(&|e| matches!(e, Event::Wrote(_)), from);
            let phrase = format!("allow step {step} {}", nth_code(k));
            let shown = at(
                &|e| matches!(e, Event::Wrote(t) if t.contains(&phrase)),
                from,
            );
            let read = at(&|e| *e == Event::Read, from);
            assert!(
                draw < first_write,
                "step {step}: drawn before anything of it is written"
            );
            assert!(
                shown < read,
                "step {step}: the phrase is shown before the answer is read"
            );
            from = read + 1;
        }
    }

    #[test]
    fn a_code_that_fails_mid_run_gives_no_partial_consent() {
        let mut input = Cursor::new(format!("allow step 3 {}\n", nth_code(0)).into_bytes());
        let mut prompt = Vec::new();
        let mut draws = 0u8;
        let mut code = || {
            draws += 1;
            (draws == 1).then_some([0xab, 0xcd, 0])
        };
        let decision = ask(
            &[request(3), request(5)],
            ConsentStreams {
                input_is_terminal: true,
                prompt_is_terminal: true,
                input: &mut input,
                prompt: &mut prompt,
                code: &mut code,
            },
            &messages(),
            &home(),
        );
        assert_eq!(
            decision,
            ConsentDecision::Refused(ConsentRefusal::CodeUnavailable)
        );
        let prompt = String::from_utf8(prompt).expect("utf-8");
        assert!(
            !prompt.contains("Step 5"),
            "the second question is never asked: {prompt}"
        );
        assert_eq!(
            input.position(),
            u64::try_from(format!("allow step 3 {}\n", nth_code(0)).len()).expect("small"),
            "only the first answer was read"
        );
    }
}
