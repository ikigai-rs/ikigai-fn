//! `$j{<src> <program>}`: sugar for `$h{urn:json:query?src=<src>&q=<program>&as=text/plain}`.
//!
//! ikigai-fn takes no dependency on ikigai-json: the marker only NAMES `urn:json:query`. So
//! these tests bind a stand-in there that reports exactly what it was asked, and the shape of
//! that request is what is pinned.
//!
//! Fixture names live under the IANA-reserved `urn:example:`.

mod common;

use std::sync::Arc;

use common::*;
use ikigai_core::{EndpointSpace, Error, Exact, FnEndpoint, Invocation, Kernel, Representation};
use ikigai_fn::{escape_markers, JQ_QUERY};

/// The stand-in for ikigai-json: answers `src|q|as`, unescaped, so the test sees every
/// byte the marker sent and that compose escaped the answer.
fn query() -> FnEndpoint {
    FnEndpoint::new("query", |inv: &Invocation<'_>| {
        let answer = format!(
            "{}|{}|{}",
            inv.inline_str("src")?,
            inv.inline_str("q")?,
            inv.inline_str("as")?
        );
        Ok(Representation::new(text_html(), answer.into_bytes()).cacheable())
    })
}

fn space() -> EndpointSpace {
    ikigai_fn::space()
        .bind(Exact::new(JQ_QUERY), query())
        .bind(
            Exact::new("urn:example:t:name"),
            html("<p>$j{urn:example:people .[0].name}</p>"),
        )
        .bind(
            Exact::new("urn:example:t:program"),
            html(r#"$j{urn:example:people {a: .b, "c": "}|{x}"} | .a // "<none>"}"#),
        )
        .bind(
            Exact::new("urn:example:t:arg"),
            html("$j{urn:example:cell:{x} .[{x}]}"),
        )
        .bind(
            Exact::new("urn:example:t:empty"),
            html("$j{urn:example:people}"),
        )
        .bind(
            Exact::new("urn:example:t:escaped"),
            html("$$j{urn:example:people .}"),
        )
        .bind(Exact::new("urn:example:t:h"), html("$h{urn:json:query}"))
}

fn kernel() -> Kernel {
    Kernel::new(Arc::new(space()))
}

#[test]
fn j_sources_the_jq_query_and_splices_it_escaped() {
    let out = compose(&kernel(), "urn:example:t:name", &[]).unwrap();
    assert_eq!(out, "<p>urn:example:people|.[0].name|text/plain</p>");
}

#[test]
fn a_program_runs_to_the_balanced_close_and_goes_in_as_written() {
    // Braces, quotes (with a `}` inside one), pipes, `//` and spaces all reach `q` as
    // written, and the answer is escaped as `$h` escapes.
    let out = compose(&kernel(), "urn:example:t:program", &[("x", "injected")]).unwrap();
    assert_eq!(
        out,
        r#"urn:example:people|{a: .b, &quot;c&quot;: &quot;}|{x}&quot;} | .a // &quot;&lt;none&gt;&quot;|text/plain"#
    );
}

#[test]
fn a_request_argument_fills_the_source_but_never_the_program() {
    // `{x}` in the source is a template argument, percent-encoded as one segment; `{x}` in
    // the program is jq text and is passed as written.
    let out = compose(&kernel(), "urn:example:t:arg", &[("x", "0/1?evil")]).unwrap();
    assert_eq!(out, "urn:example:cell:0%2F1%3Fevil|.[{x}]|text/plain");
}

#[test]
fn a_j_without_a_program_is_refused() {
    let err = compose(&kernel(), "urn:example:t:empty", &[]).unwrap_err();
    assert!(
        err.to_string()
            .contains("needs a source IRI, whitespace, and a jq program"),
        "{err}"
    );
}

#[test]
fn dollar_dollar_j_is_literal_and_escape_markers_writes_it() {
    let out = compose(&kernel(), "urn:example:t:escaped", &[]).unwrap();
    assert_eq!(out, "$j{urn:example:people .}");
    assert_eq!(escape_markers("$j{a .}"), "$$j{a .}");
}

#[test]
fn with_no_jq_endpoint_bound_j_fails_as_an_unresolved_h_does() {
    let bare = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(
                Exact::new("urn:example:t:name"),
                html("<p>$j{urn:example:people .[0].name}</p>"),
            )
            .bind(Exact::new("urn:example:t:h"), html("$h{urn:json:query}")),
    ));
    let j = compose(&bare, "urn:example:t:name", &[]).unwrap_err();
    let h = compose(&bare, "urn:example:t:h", &[]).unwrap_err();
    assert!(
        matches!(j, Error::Unresolved(ref iri) if iri.as_str() == JQ_QUERY),
        "{j:?}"
    );
    assert_eq!(j.to_string(), h.to_string());
}
