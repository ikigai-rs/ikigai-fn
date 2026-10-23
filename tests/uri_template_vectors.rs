//! The shared URI-template vectors (`tests/vectors/uri_template_vectors.json`, ledger #2132)
//! against [`ikigai_fn::html::url_segment`] and [`ikigai_fn::html::url_reserved`], and against
//! `compose` itself wherever a case is a template compose can fill.
//!
//! The file is the contract every port pins, as `html_vectors.json` is: a Python or Deno copy
//! runs the same cases from a verbatim copy. The template cases are RFC 6570's own examples
//! for the two forms compose supports (`{var}` and `{+var}`, one defined variable each), plus
//! cases marked `ikigai`. Every failing case is reported at once.

mod common;

use std::sync::Arc;

use common::*;
use ikigai_core::{Exact, FnEndpoint, Invocation, Kernel, Representation, UriTemplate};
use ikigai_fn::html::{url_reserved, url_segment};
use serde_json::Value;

const VECTORS: &str = include_str!("vectors/uri_template_vectors.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("uri_template_vectors.json is JSON")
}

fn field<'a>(case: &'a Value, name: &str) -> &'a str {
    case[name]
        .as_str()
        .unwrap_or_else(|| panic!("`{name}` is a string in {case}"))
}

fn report(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} case(s) differ from the vectors:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Expand `template` the way RFC 6570 does for the two forms: literal text as it is, `{name}`
/// through [`url_segment`], `{+name}` through [`url_reserved`]. This is the reference a port
/// writes too; `compose` has its own parser, checked against the same cases below.
fn expand(template: &str, variables: &Value) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').expect("a closed expression");
        let expr = &after[..close];
        let (reserved, name) = match expr.strip_prefix('+') {
            Some(name) => (true, name),
            None => (false, expr),
        };
        let value = variables[name]
            .as_str()
            .unwrap_or_else(|| panic!("`{name}` is a defined variable"));
        out.push_str(&if reserved {
            url_reserved(value)
        } else {
            url_segment(value)
        });
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[test]
fn the_value_cases_match_both_rules() {
    let doc = vectors();
    let mut failures = Vec::new();
    for case in doc["values"].as_array().expect("`values` is a list") {
        let input = field(case, "in");
        for (name, got, want) in [
            ("url_segment", url_segment(input), field(case, "simple")),
            ("url_reserved", url_reserved(input), field(case, "reserved")),
        ] {
            if got != want {
                failures.push(format!("{name}({input:?}) = {got:?}, want {want:?}"));
            }
        }
    }
    report(failures);
}

#[test]
fn the_template_cases_match_the_rfc() {
    let doc = vectors();
    let cases = doc["templates"].as_array().expect("`templates` is a list");
    assert!(
        cases
            .iter()
            .filter(|c| field(c, "source").starts_with("rfc6570"))
            .count()
            >= 10,
        "the RFC's examples are the vectors"
    );
    let mut failures = Vec::new();
    for case in cases {
        let (template, want) = (field(case, "template"), field(case, "out"));
        let got = expand(template, &doc["variables"]);
        if got != want {
            failures.push(format!("{template} = {got:?}, want {want:?}"));
        }
    }
    report(failures);
}

/// Every template case that is a compose IRI (an absolute name with no `?`, which a marker
/// reads as its arguments) is filled by compose, as a marker, to the same bytes.
#[test]
fn compose_fills_the_template_cases_it_can_carry_to_the_same_bytes() {
    let doc = vectors();
    let variables = doc["variables"]
        .as_object()
        .expect("`variables` is an object");
    let args: Vec<(&str, &str)> = variables
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str().expect("a string variable")))
        .collect();
    let mut failures = Vec::new();
    let mut filled = 0;
    for case in doc["templates"].as_array().expect("`templates` is a list") {
        let (template, want) = (field(case, "template"), field(case, "out"));
        if !template.starts_with("urn:") || template.contains('?') {
            continue;
        }
        filled += 1;
        let body = format!("$r{{{template}}}");
        let space = ikigai_fn::space()
            .bind(Exact::new("urn:example:template"), owned_html(body))
            .bind(
                UriTemplate::parse("urn:example:page:{rest}").unwrap(),
                FnEndpoint::new("name", |inv: &Invocation<'_>| {
                    Ok(Representation::new(
                        text_html(),
                        inv.request.target.as_str().as_bytes().to_vec(),
                    ))
                }),
            );
        let kernel = Kernel::new(Arc::new(space));
        match compose(&kernel, "urn:example:template", &args) {
            Ok(got) if got == want => {}
            other => failures.push(format!("compose {template} = {other:?}, want {want:?}")),
        }
    }
    assert!(filled >= 2, "compose carries some of the cases");
    report(failures);
}

fn owned_html(body: String) -> FnEndpoint {
    FnEndpoint::new("fixed", move |_: &Invocation<'_>| {
        Ok(Representation::new(text_html(), body.clone().into_bytes()).cacheable())
    })
}
