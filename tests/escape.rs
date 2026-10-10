//! Escaping text for `compose`: [`ikigai_fn::escape_markers`] and
//! [`ikigai_fn::MARKER_LETTERS`] (ledger #646).
//!
//! A host that puts arbitrary text into a template (a generated comment, a summary, a
//! stored value spliced through `$a`) must escape every marker compose recognizes. A
//! host that keeps its own copy of the marker letters escapes only the letters it knew
//! when the copy was made: fn 0.3.0 added `$r` and `$h`, compose's own summary spelled
//! both, and ikigai-cli's generated alias prelude, whose copy knew only `$a`, stopped
//! composing (ikigai-rs/ikigai-cli PR 372). These tests pin the exported escape against
//! the parser itself, so a letter added to one is a letter added to the other.
//!
//! Fixture names live under the IANA-reserved `urn:example:`. `urn:example:secret` is
//! never bound, so a marker that stayed live fails the compose with `Unresolved`.

mod common;

use std::sync::Arc;

use common::*;
use ikigai_core::{EndpointSpace, Error, Exact, Kernel};
use ikigai_fn::{escape_markers, MARKER_LETTERS};

/// Compose `text` as the whole template, with `toUpper` bound so a live marker that
/// resolves would show itself by changing the output.
fn compose_text(text: &str) -> ikigai_core::Result<String> {
    let page: &'static str = Box::leak(text.to_string().into_boxed_str());
    let space = EndpointSpace::new()
        .bind(Exact::new("urn:iki:fn:compose"), ikigai_fn::compose())
        .bind(Exact::new("urn:iki:fn:toUpper"), ikigai_fn::to_upper())
        .bind(Exact::new("urn:example:page"), html(page));
    compose(&Kernel::new(Arc::new(space)), "urn:example:page", &[])
}

/// Text holding every marker form the exported list names, live and already escaped,
/// and a few `$` that are not markers at all.
fn every_marker_form() -> String {
    let mut text = String::from("costs $5, or $$ at the end, or a bare $");
    for letter in MARKER_LETTERS {
        text.push_str(&format!(
            "\n${letter}{{urn:example:secret}} ${letter}{{urn:iki:fn:toUpper?in=x}} \
             $${letter}{{<iri>}} $$${letter}{{urn:example:secret}} ${letter}"
        ));
    }
    // Unterminated last, where no later `}` can close them.
    for letter in MARKER_LETTERS {
        text.push_str(&format!(" ${letter}{{unterminated"));
    }
    text
}

#[test]
fn the_unescaped_forms_are_live() {
    // The premise: without escaping, the text is not literal — a marker in it resolves.
    let error = compose_text(&every_marker_form()).unwrap_err();
    assert!(matches!(error, Error::Unresolved(_)), "{error:?}");
}

#[test]
fn every_marker_form_survives_compose_when_escaped() {
    let text = every_marker_form();
    assert_eq!(compose_text(&escape_markers(&text)).unwrap(), text);
}

#[test]
fn escaping_is_exact_for_every_run_of_dollars() {
    // compose(escape(s)) == s for every run length before every kind of follower: a
    // marker (closed or not), a non-marker letter, a brace, another `$`, the end.
    let mut followers: Vec<String> = vec![
        String::new(),
        "{urn:example:secret}".into(),
        "b{urn:example:secret}".into(),
        "x".into(),
        " ".into(),
    ];
    for letter in MARKER_LETTERS {
        followers.push(format!("{letter}{{urn:example:secret}}"));
        followers.push(format!("{letter}{{"));
        followers.push(format!("{letter}"));
    }
    for run in 0..5 {
        for follower in &followers {
            let text = format!("<{}{follower}>", "$".repeat(run));
            let escaped = escape_markers(&text);
            assert_eq!(
                compose_text(&escaped).unwrap(),
                text,
                "escaped as {escaped:?}"
            );
        }
    }
}

#[test]
fn escaping_leaves_text_without_markers_alone() {
    // A lone `$` that opens nothing is already literal, so the escape is not noisy.
    let text = "costs $5 {and} a{b} — no markers here, $";
    assert_eq!(escape_markers(text), text);
}

#[test]
fn the_exported_letters_are_exactly_the_letters_the_parser_reads() {
    // Every printable ASCII letter after `$` and before `{`: live (the compose fails on
    // the unbound name) iff the exported list names it.
    for letter in (b'!'..=b'~').map(char::from).filter(|c| *c != '$') {
        let text = format!("${letter}{{urn:example:secret}}");
        let live = compose_text(&text).is_err();
        assert_eq!(
            live,
            MARKER_LETTERS.contains(&letter),
            "`{text}` is {} but the list says otherwise",
            if live { "a marker" } else { "literal" }
        );
    }
}

/// The hazard, as a host would write it: an escape over a hand-copied list of letters
/// (the shape of ikigai-cli's `escape_markers` before ledger #646).
fn escape_with(letters: &[char], text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            let mut j = i;
            while j < chars.len() && chars[j] == '$' {
                j += 1;
            }
            if chars.get(j + 1) == Some(&'{') && chars.get(j).is_some_and(|c| letters.contains(c)) {
                out.push_str("$$");
            } else {
                out.extend(&chars[i..j]);
            }
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[test]
fn a_hand_copied_list_misses_the_newest_letter() {
    // A copy made before the newest letter existed: everything but the last.
    let (newest, copied) = MARKER_LETTERS.split_last().expect("at least one letter");
    let text = format!("documents ${newest}{{<iri>}} for its users");
    assert!(
        compose_text(&escape_with(copied, &text)).is_err(),
        "a copy without `{newest}` leaves `${newest}{{` live"
    );
    assert_eq!(compose_text(&escape_markers(&text)).unwrap(), text);
}
