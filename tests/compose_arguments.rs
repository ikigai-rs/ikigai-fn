//! Compose's public contract must let the REPL pass template parameters by name.

mod common;

use std::sync::Arc;

use common::*;
use ikigai_core::{
    Description, Exact, Kernel, MetaRenderer, ReprType, Representation, Request, Verb,
};
use ikigai_engine::{Action, Engine};

/// The JSON face of Meta, without depending on a host's renderer crate.
struct JsonRenderer;

impl MetaRenderer for JsonRenderer {
    fn render(
        &self,
        description: &Description,
        _target: &ReprType,
    ) -> ikigai_core::Result<Representation> {
        Ok(Representation::new(
            ReprType::new("application/json"),
            serde_json::to_vec(description).unwrap(),
        ))
    }
}

fn kernel() -> Kernel {
    Kernel::with_meta_renderer(
        Arc::new(
            ikigai_fn::space().bind(Exact::new("urn:file:param.txt"), html("minimum: $h{{min}}")),
        ),
        Arc::new(JsonRenderer),
    )
}

#[test]
fn compose_meta_json_declares_open_template_arguments() {
    let repr = issue(
        &kernel(),
        Request::new(Verb::Meta, iri("urn:iki:fn:compose")),
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&repr.bytes).unwrap();
    assert_eq!(
        json["arguments"],
        serde_json::json!({"open": "each fills a {name} marker in the body"})
    );
    let description: Description = serde_json::from_value(json).unwrap();
    assert_eq!(
        description
            .action_specs()
            .into_iter()
            .find(|action| action.verb == Verb::Source)
            .unwrap()
            .arguments,
        description.arguments
    );
}

#[test]
fn repl_supplies_a_named_template_parameter_to_compose() {
    let engine = Engine::new(kernel());
    let action = engine.eval("source urn:iki:fn:compose src=urn:file:param.txt min=6");
    let Action::Output(entry) = action else {
        panic!("expected the composed response");
    };
    assert_eq!(entry.result, Ok("minimum: 6".to_string()));
}

/// An endpoint without a contract sees the argument exactly as compose issued it.
fn marker_kernel(template: &'static str) -> Kernel {
    use ikigai_core::{ArgRef, FnEndpoint, Invocation};
    let inspect = FnEndpoint::new("inspect argument", |inv: &Invocation<'_>| {
        let value = match inv.request.args.get("in").expect("marker supplies in") {
            ArgRef::Reference(iri) => format!("reference:{iri}"),
            ArgRef::Inline(bytes) => format!("inline:{}", String::from_utf8_lossy(bytes)),
            other => panic!("unexpected argument: {other:?}"),
        };
        Ok(Representation::new(
            ReprType::new("text/plain"),
            value.into_bytes(),
        ))
    });
    Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(Exact::new("urn:t:page"), html(template))
            .bind(Exact::new("urn:t:echo"), inspect)
            .bind(Exact::new("urn:t:data"), html("hello")),
    ))
}

#[test]
fn unquoted_angle_argument_is_issued_as_a_reference() {
    let kernel = marker_kernel("$r{urn:t:echo?in=<urn:t:data>}");
    assert_eq!(
        compose(&kernel, "urn:t:page", &[]).unwrap(),
        "reference:urn:t:data"
    );
}

#[test]
fn quoted_angle_argument_stays_inline() {
    let kernel = marker_kernel(r#"$r{urn:t:echo?in="<urn:t:data>"}"#);
    assert_eq!(
        compose(&kernel, "urn:t:page", &[]).unwrap(),
        "inline:<urn:t:data>"
    );
}

#[test]
fn reference_placeholder_is_percent_encoded() {
    let kernel = marker_kernel("$r{urn:t:echo?in=<urn:t:{var}>}");
    assert_eq!(
        compose(&kernel, "urn:t:page", &[("var", "data/one?x=y&z=#two")]).unwrap(),
        "reference:urn:t:data%2Fone%3Fx%3Dy%26z%3D%23two"
    );
}

#[test]
fn kernel_settles_marker_reference_by_target_contract() {
    let kernel = marker_kernel("$r{urn:iki:fn:toUpper?in=<urn:t:data>}");
    assert_eq!(compose(&kernel, "urn:t:page", &[]).unwrap(), "HELLO");
}

#[test]
fn reference_query_delimiters_stay_inside_the_reference() {
    let kernel = marker_kernel("$r{urn:t:echo?in=<urn:t:data?a=1&b=2>&unused=yes}");
    assert_eq!(
        compose(&kernel, "urn:t:page", &[]).unwrap(),
        "reference:urn:t:data?a=1&b=2"
    );
}

#[test]
fn invalid_reference_iri_uses_marker_fallback() {
    let kernel = marker_kernel("$r{urn:t:echo?in=<not-an-iri> || urn:t:data}");
    assert_eq!(compose(&kernel, "urn:t:page", &[]).unwrap(), "hello");
}
