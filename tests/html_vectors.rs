//! The shared HTML vectors (`tests/vectors/html_vectors.json`, ledger #1813) against
//! [`ikigai_fn::html`].
//!
//! The file is the contract every port pins: a Python, Deno or XSLT copy runs the same cases
//! from a verbatim copy. Its expected outputs are written by hand, so this test fails when the
//! Rust drifts from the contract, and a case added to the file is a case every port must pass.
//! Every failing case is reported at once, not just the first.

mod common;

use std::sync::Arc;

use common::*;
use ikigai_core::{EndpointSpace, Exact, Kernel};
use ikigai_fn::html::{attr, html_text, json_attr, safe_href, url_segment};
use serde_json::Value;

const VECTORS: &str = include_str!("vectors/html_vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("html_vectors.json is JSON")
}

/// The cases under `key`, each `{"in", "out", …}`.
fn cases<'a>(doc: &'a Value, key: &str) -> &'a [Value] {
    let cases = doc[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is a list of cases"));
    assert!(!cases.is_empty(), "`{key}` has cases");
    cases
}

fn text<'a>(case: &'a Value, field: &str) -> &'a str {
    case[field]
        .as_str()
        .unwrap_or_else(|| panic!("`{field}` is a string in {case}"))
}

fn report(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} case(s) differ from the vectors:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn html_text_and_attr_match_the_escape_vectors() {
    let doc = vectors();
    let mut failures = Vec::new();
    for case in cases(&doc, "escape") {
        let (input, want) = (text(case, "in"), text(case, "out"));
        for (name, got) in [("html_text", html_text(input)), ("attr", attr(input))] {
            if got != want {
                failures.push(format!("{name}({input:?}) = {got:?}, want {want:?}"));
            }
        }
    }
    report(failures);
}

/// `compose`'s `$h` escapes with the same function: an argument spliced through `$h` and a
/// resource spliced through `$h` both give the vector's output, so there is one rule.
#[test]
fn compose_h_escapes_by_the_same_vectors() {
    let doc = vectors();
    let mut failures = Vec::new();
    for case in cases(&doc, "escape") {
        let (input, want) = (text(case, "in"), text(case, "out"));
        let held: &'static str = Box::leak(input.to_string().into_boxed_str());
        let space = EndpointSpace::new()
            .bind(Exact::new("urn:iki:fn:compose"), ikigai_fn::compose())
            .bind(
                Exact::new("urn:example:page"),
                html("[$h{{x}}][$h{urn:example:held}]"),
            )
            .bind(Exact::new("urn:example:held"), html(held));
        let kernel = Kernel::new(Arc::new(space));
        let got = compose(&kernel, "urn:example:page", &[("x", input)])
            .unwrap_or_else(|e| format!("error: {e}"));
        let expected = format!("[{want}][{want}]");
        if got != expected {
            failures.push(format!(
                "compose $h of {input:?} = {got:?}, want {expected:?}"
            ));
        }
    }
    report(failures);
}

#[test]
fn url_segment_matches_the_vectors() {
    let doc = vectors();
    let mut failures = Vec::new();
    for case in cases(&doc, "url_segment") {
        let (input, want) = (text(case, "in"), text(case, "out"));
        let got = url_segment(input);
        if got != want {
            failures.push(format!("url_segment({input:?}) = {got:?}, want {want:?}"));
        }
    }
    report(failures);
}

#[test]
fn safe_href_matches_the_vectors() {
    let doc = vectors();
    let mut failures = Vec::new();
    for case in cases(&doc, "safe_href") {
        let input = text(case, "in");
        let want = case["out"].as_str();
        assert!(
            want.is_some() || case["out"].is_null(),
            "`out` is a string or null in {case}"
        );
        let got = safe_href(input);
        if got != want {
            let why = text(case, "why");
            failures.push(format!(
                "safe_href({input:?}) = {got:?}, want {want:?} ({why})"
            ));
        }
    }
    report(failures);
}

#[test]
fn json_attr_matches_the_vectors() {
    let doc = vectors();
    let mut failures = Vec::new();
    for case in cases(&doc, "json_attr") {
        let pairs: Vec<(String, String)> = case["in"]
            .as_array()
            .unwrap_or_else(|| panic!("`in` is a list of pairs in {case}"))
            .iter()
            .map(|pair| {
                let pair: Vec<&str> = pair
                    .as_array()
                    .and_then(|p| p.iter().map(Value::as_str).collect())
                    .unwrap_or_else(|| panic!("a pair is [key, value] in {case}"));
                assert_eq!(pair.len(), 2, "a pair is [key, value] in {case}");
                (pair[0].to_string(), pair[1].to_string())
            })
            .collect();
        let want = text(case, "out");
        let got = json_attr(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        if got != want {
            failures.push(format!("json_attr({pairs:?}) = {got:?}, want {want:?}"));
        }
    }
    report(failures);
}

/// What `json_attr` emits is JSON once the attribute is decoded: undo the five entities
/// `attr` writes and a JSON parser reads back the same members.
#[test]
fn json_attr_decodes_to_the_same_object() {
    let pairs = [
        ("note", "say \"hi\" & <b>'x'</b>\n\u{1}"),
        ("path", "C:\\dir"),
        ("café", "✓ \u{1F600}"),
    ];
    let decoded = json_attr(pairs)
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let object: serde_json::Map<String, Value> =
        serde_json::from_str(&decoded).expect("the decoded attribute is a JSON object");
    // Compare members independently of serde_json's preserve_order feature, which
    // the engine enables. The vectors separately pin the emitted text's order.
    let mut members: Vec<(&str, &str)> = object
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str().expect("a string value")))
        .collect();
    members.sort();
    let mut want = pairs.to_vec();
    want.sort();
    assert_eq!(members, want);
}

/// Every output of `url_segment` is unreserved characters and `%XX` only, and decoding it
/// gives back the input's bytes: the encoding is lossless and can never add a delimiter.
#[test]
fn url_segment_round_trips_and_adds_no_delimiter() {
    let doc = vectors();
    for case in cases(&doc, "url_segment") {
        let input = text(case, "in");
        let encoded = url_segment(input);
        let bytes = encoded.as_bytes();
        let mut decoded = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'%' {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
                assert!(
                    hex.chars()
                        .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()),
                    "uppercase hex in {encoded:?}"
                );
                decoded.push(u8::from_str_radix(hex, 16).unwrap());
                i += 3;
            } else {
                assert!(
                    b.is_ascii_alphanumeric() || b"-._~".contains(&b),
                    "{encoded:?} holds a reserved byte {:?}",
                    char::from(b)
                );
                decoded.push(b);
                i += 1;
            }
        }
        assert_eq!(decoded, input.as_bytes(), "{input:?} round-trips");
    }
}

/// A target `safe_href` admits is still DATA: it may hold `&` and `"`, so it goes into the
/// attribute through `attr`, and the vectors' admitted targets prove it needs to.
#[test]
fn an_admitted_target_still_needs_attr() {
    let href = "https://example.org/a?b=1&c=\"2\"";
    let admitted = safe_href(href).expect("an https target is a link");
    assert_eq!(admitted, href, "admitted, unchanged");
    assert_eq!(
        format!("<a href=\"{}\">", attr(admitted)),
        "<a href=\"https://example.org/a?b=1&amp;c=&quot;2&quot;\">"
    );
}
