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
