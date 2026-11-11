//! Boundary and dependency checks for the template/composer audit.
mod common;

use common::*;
use ikigai_core::{
    ArgRef, Error, Exact, FnEndpoint, Invocation, Kernel, Representation, UriTemplate,
};
use ikigai_fn::COMPOSE_MAX_NODES;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn owned(body: String) -> FnEndpoint {
    FnEndpoint::new("text", move |_: &Invocation<'_>| {
        Ok(Representation::new(text_html(), body.as_bytes().to_vec()).cacheable())
    })
}

fn kernel(template: String) -> Kernel {
    Kernel::new(Arc::new(
        ikigai_fn::space().bind(Exact::new("urn:example:template"), owned(template)),
    ))
}

#[test]
fn node_limit_accepts_exact_boundary_and_refuses_the_next_node_before_resolution() {
    for (count, succeeds) in [(COMPOSE_MAX_NODES, true), (COMPOSE_MAX_NODES + 1, false)] {
        let reads = Arc::new(AtomicUsize::new(0));
        let k = Kernel::new(Arc::new(
            ikigai_fn::space()
                .bind(
                    Exact::new("urn:example:template"),
                    owned("$r{urn:example:leaf}".repeat(count)),
                )
                .bind(
                    Exact::new("urn:example:leaf"),
                    counted("x", Arc::clone(&reads)),
                ),
        ));
        let answer = compose(&k, "urn:example:template", &[]);
        if succeeds {
            assert_eq!(answer.unwrap(), "x".repeat(count));
            // Force a new fill with a different cache key: its budget starts fresh.
            assert_eq!(
                compose(&k, "urn:example:template", &[("unused", "new")]).unwrap(),
                "x".repeat(count)
            );
        } else {
            assert!(matches!(answer, Err(Error::Endpoint(s)) if s.contains("node limit")));
            assert_eq!(reads.load(Ordering::SeqCst), 0);
        }
    }
}

#[test]
fn sibling_expansions_share_a_budget_and_fallback_cannot_hide_exhaustion() {
    // Each child fits alone. Their combined nodes exceed the fill's budget.
    let k = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(Exact::new("urn:example:template"), html("$a{urn:example:child || urn:example:ok}$a{urn:example:child || urn:example:ok}"))
            .bind(Exact::new("urn:example:child"), owned("$h{{x}}".repeat(COMPOSE_MAX_NODES / 2)))
            .bind(Exact::new("urn:example:ok"), html("fallback")),
    ));
    assert!(matches!(compose(&k, "urn:example:template", &[("x", "x")]),
        Err(Error::Endpoint(s)) if s.contains("node limit")));
}

#[test]
fn literal_segments_also_count_as_produced_nodes() {
    let k = kernel("x$h{{x}}".repeat(COMPOSE_MAX_NODES / 2) + "x");
    assert!(matches!(compose(&k, "urn:example:template", &[("x", "")]),
        Err(Error::Endpoint(s)) if s.contains("node limit")));
}

#[test]
fn direct_and_indirect_transclusion_cycles_are_typed_refusals() {
    for child in ["$a{urn:example:template}", "$a{urn:example:child}"] {
        let k = Kernel::new(Arc::new(
            ikigai_fn::space()
                .bind(Exact::new("urn:example:template"), html(child))
                .bind(
                    Exact::new("urn:example:child"),
                    html("$a{urn:example:template}"),
                ),
        ));
        assert!(matches!(compose(&k, "urn:example:template", &[]),
            Err(Error::Endpoint(s)) if s.contains("recursion limit")));
    }
}

#[test]
fn marker_data_is_terminated_for_arguments_and_resources_with_exact_whitespace() {
    let data = "$h{urn:example:absent} $r{urn:example:absent} $a{urn:example:absent} {x} }\0<&\"'";
    let k = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(
                Exact::new("urn:example:template"),
                html(" \t$h{{x}}\n$h{urn:example:data}\r\n$r{{x}} "),
            )
            .bind(Exact::new("urn:example:data"), owned(data.to_string())),
    ));
    let escaped = ikigai_fn::html::html_text(data);
    let expected = format!(" \t{escaped}\n{escaped}\r\n{data} ");
    for _ in 0..2 {
        assert_eq!(
            compose(&k, "urn:example:template", &[("x", data)]).unwrap(),
            expected
        );
    }
}

#[test]
fn malformed_utf8_and_surrogate_encodings_are_refused_by_h() {
    for bytes in [vec![0xff], vec![0xed, 0xa0, 0x80]] {
        let k = Kernel::new(Arc::new(
            ikigai_fn::space()
                .bind(
                    Exact::new("urn:example:template"),
                    html("$h{urn:example:bytes}"),
                )
                .bind(
                    Exact::new("urn:example:bytes"),
                    FnEndpoint::new("bytes", move |_: &Invocation<'_>| {
                        Ok(Representation::new(text_html(), bytes.clone()))
                    }),
                ),
        ));
        assert!(
            matches!(compose(&k, "urn:example:template", &[]), Err(Error::Endpoint(s)) if s.contains("non-text"))
        );
    }
}

#[test]
fn marker_syntax_preserves_literals_and_refuses_empty_or_nested_argument_names() {
    for literal in [
        "{x}",
        "$h{unterminated",
        "$$h{}",
        " $r{urn:example:x?in=\"unterminated} ",
    ] {
        let k = kernel(literal.to_string());
        let expected = if literal == "$$h{}" { "$h{}" } else { literal };
        assert_eq!(compose(&k, "urn:example:template", &[]).unwrap(), expected);
    }
    for invalid in ["$h{}", "$h{ }", "$h{{{x}}}"] {
        assert!(matches!(
            compose(&kernel(invalid.to_string()), "urn:example:template", &[]),
            Err(Error::Endpoint(_))
        ));
    }
    assert!(
        matches!(compose(&kernel("$h{{x}}".to_string()), "urn:example:template", &[]),
        Err(Error::MissingArgument(s)) if s == "x")
    );
}

#[test]
fn nested_dependencies_cut_only_views_that_include_them() {
    let held = Arc::new(Mutex::new(BTreeMap::from([
        (
            "template".to_string(),
            "[$a{urn:example:stored:inner}]".to_string(),
        ),
        (
            "inner".to_string(),
            "$h{urn:example:stored:value}".to_string(),
        ),
        ("value".to_string(), "first".to_string()),
        ("other".to_string(), "unrelated".to_string()),
    ])));
    let k = Kernel::new(Arc::new(ikigai_fn::space().bind(
        UriTemplate::parse("urn:example:stored:{key}").unwrap(),
        store(held),
    )));
    let included = compose_request("urn:example:stored:template", &[]);
    let unrelated = compose_request("urn:example:stored:other", &[]);
    assert_eq!(issue(&k, included.clone()).unwrap().bytes, b"[first]");
    issue(&k, unrelated.clone()).unwrap();
    sink(&k, "urn:example:stored:value", "second &").unwrap();
    assert!(!cached(&k, &included));
    assert!(cached(&k, &unrelated));
    assert_eq!(
        issue(&k, included.clone()).unwrap().bytes,
        b"[second &amp;]"
    );
    sink(
        &k,
        "urn:example:stored:inner",
        "($h{urn:example:stored:value})",
    )
    .unwrap();
    assert!(!cached(&k, &included));
    assert!(cached(&k, &unrelated));
    assert_eq!(
        issue(&k, included.clone()).unwrap().bytes,
        b"[(second &amp;)]"
    );
    sink(
        &k,
        "urn:example:stored:template",
        "new $a{urn:example:stored:inner}",
    )
    .unwrap();
    assert!(!cached(&k, &included));
    assert!(cached(&k, &unrelated));
    assert_eq!(issue(&k, included).unwrap().bytes, b"new (second &amp;)");
}

fn conditional_request() -> ikigai_core::Request {
    source_request(
        "urn:iki:fn:conditional",
        &[("if", "urn:example:condition"), ("then", "urn:example:yes")],
    )
}

#[test]
fn conditional_equals_trims_only_the_condition_and_does_not_normalize_or_expand() {
    let marker = "$h{urn:example:absent}";
    for (condition, equals, expected) in [
        (" \té\n", "é", "yes"),
        ("é", "e\u{301}", ""),
        ("é", " é ", ""),
        (marker, marker, "yes"),
    ] {
        let k = Kernel::new(Arc::new(
            ikigai_fn::space()
                .bind(Exact::new("urn:example:condition"), html(condition))
                .bind(Exact::new("urn:example:yes"), html("yes")),
        ));
        let request =
            conditional_request().with_arg("equals", ArgRef::Inline(equals.as_bytes().to_vec()));
        assert_eq!(issue(&k, request).unwrap().bytes, expected.as_bytes());
    }
}

#[test]
fn invalid_present_optional_arguments_do_not_become_defaults() {
    let k = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(Exact::new("urn:example:condition"), html("true"))
            .bind(Exact::new("urn:example:yes"), html("yes")),
    ));
    for (name, bytes) in [
        ("equals", vec![0xff]),
        ("else", vec![0xff]),
        ("else", b"not an IRI".to_vec()),
    ] {
        assert!(issue(
            &k,
            conditional_request().with_arg(name, ArgRef::Inline(bytes))
        )
        .is_err());
    }
    // Empty is a present text value, and is compared rather than treated as absence.
    let request = conditional_request().with_arg("equals", ArgRef::Inline(Vec::new()));
    assert!(issue(&k, request).unwrap().bytes.is_empty());
    assert!(issue(
        &k,
        source_request("urn:iki:fn:conditional", &[("if", "urn:example:condition")])
    )
    .is_err());
}

#[test]
fn conditional_tracks_the_condition_and_only_the_taken_branch() {
    let held = Arc::new(Mutex::new(BTreeMap::from([
        ("condition".to_string(), "true".to_string()),
        ("yes".to_string(), "$h{urn:example:absent}".to_string()),
        ("no".to_string(), "no".to_string()),
    ])));
    let k = Kernel::new(Arc::new(ikigai_fn::space().bind(
        UriTemplate::parse("urn:example:stored:{key}").unwrap(),
        store(held),
    )));
    let request = source_request(
        "urn:iki:fn:conditional",
        &[
            ("if", "urn:example:stored:condition"),
            ("then", "urn:example:stored:yes"),
            ("else", "urn:example:stored:no"),
        ],
    );
    assert_eq!(
        issue(&k, request.clone()).unwrap().bytes,
        b"$h{urn:example:absent}"
    );
    sink(&k, "urn:example:stored:no", "changed no").unwrap();
    assert!(cached(&k, &request));
    sink(&k, "urn:example:stored:condition", "false").unwrap();
    assert!(!cached(&k, &request));
    assert_eq!(issue(&k, request.clone()).unwrap().bytes, b"changed no");
    sink(&k, "urn:example:stored:yes", "changed yes").unwrap();
    assert!(cached(&k, &request));
    sink(&k, "urn:example:stored:no", "changed again").unwrap();
    assert!(!cached(&k, &request));
    assert_eq!(issue(&k, request).unwrap().bytes, b"changed again");
}

#[test]
fn each_invalid_present_options_are_refused_before_empty_or_nonempty_branches() {
    for collection in ["[]", "[1]"] {
        let k = Kernel::new(Arc::new(
            ikigai_fn::space()
                .bind(Exact::new("urn:example:collection"), html(collection))
                .bind(Exact::new("urn:example:template"), html("row")),
        ));
        let request = source_request(
            "urn:iki:fn:each",
            &[
                ("src", "urn:example:collection"),
                ("template", "urn:example:template"),
            ],
        );
        for (name, bytes) in [
            ("separator", vec![0xff]),
            ("empty", vec![0xff]),
            ("empty", b"invalid IRI".to_vec()),
        ] {
            assert!(issue(&k, request.clone().with_arg(name, ArgRef::Inline(bytes))).is_err());
        }
    }
}
