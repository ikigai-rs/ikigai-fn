//! Named views in a DECLARED space: `compose_over(…).named(…)` against core's builder.
//!
//! Core 0.1.83 builds a live space from its declaration (`Topology::from_turtle`, then
//! `build` over a `Registry`). A door binds its endpoint BY NAME (`ik:endpointName`), and
//! the registry refuses a second, different endpoint under a name already taken. Every
//! `compose_over` instance used to answer `composeOver`, so an application with several
//! views — the tutorial's tic-tac-toe binds five — could not be declared at all: five
//! endpoints, one name. A named view is its own endpoint by name, and this file proves the
//! whole path: register, render, parse, build, resolve.

mod common;

use std::sync::Arc;

use common::*;
use ikigai_core::{
    build, DeclarationError, Endpoint, EndpointSpace, Error, FnEndpoint, Invocation, Kernel,
    Registry, Representation, Space, SpaceKind, Topology, UriTemplate,
};
use ikigai_fn::{compose_over, ComposeOver, COMPOSE_OVER};

/// The five square views of the tutorial's tic-tac-toe, each over its own template.
const VIEWS: [&str; 5] = [
    "square",
    "square-empty",
    "square-open",
    "square-taken",
    "square-closed",
];

/// The name a view is registered and declared under.
fn view_name(kind: &str) -> String {
    format!("ttt-view-{kind}")
}

/// A view's door: parameterized by the square it renders.
fn view_pattern(kind: &str) -> String {
    format!("urn:example:ttt:view:{kind}:{{x}}:{{y}}")
}

fn view(kind: &str) -> ComposeOver {
    compose_over(iri(&format!("urn:example:ttt:template:{kind}"))).named(view_name(kind))
}

/// Every template, told apart by its kind, filling the square's coordinates — so a door
/// bound to the wrong view answers visibly wrong bytes.
fn templates() -> FnEndpoint {
    FnEndpoint::new("ttt-templates", |inv: &Invocation<'_>| {
        let name = inv
            .bindings
            .get("name")
            .ok_or_else(|| Error::MissingArgument("name".to_string()))?;
        let body = format!(r#"<td class="{name}" data-at="$h{{{{x}}}},$h{{{{y}}}}"></td>"#);
        Ok(Representation::new(text_html(), body.into_bytes()).cacheable())
    })
}

/// A view's door pattern and the endpoint bound there.
type Door = (String, Arc<dyn Endpoint>);

/// The views and the templates as shared endpoints: the SAME `Arc`s go into the coded
/// space and the registry, which is how a host registers what it also binds.
fn endpoints() -> (Arc<dyn Endpoint>, Vec<Door>) {
    let views = VIEWS
        .iter()
        .map(|kind| {
            (
                view_pattern(kind),
                Arc::new(view(kind)) as Arc<dyn Endpoint>,
            )
        })
        .collect();
    (Arc::new(templates()), views)
}

fn at(pattern: &str) -> UriTemplate {
    UriTemplate::parse(pattern).expect("a valid template")
}

/// The arrangement written in code.
fn coded(templates: &Arc<dyn Endpoint>, views: &[Door]) -> EndpointSpace {
    views.iter().fold(
        EndpointSpace::new().bind_arc(at("urn:example:ttt:template:{name}"), Arc::clone(templates)),
        |space, (pattern, view)| space.bind_arc(at(pattern), Arc::clone(view)),
    )
}

/// The five named views register side by side in one registry, a declaration of the
/// space they are bound in builds, and each door answers with its OWN template.
#[test]
fn five_named_views_declare_and_build() {
    let (templates, views) = endpoints();
    let mut registry = Registry::new();
    registry
        .register(Arc::clone(&templates))
        .expect("templates");
    for (_, view) in &views {
        registry.register(Arc::clone(view)).expect("a named view");
    }
    assert_eq!(registry.len(), 6, "{registry:?}");

    // Render the coded arrangement as a declaration, then build from the declaration.
    let coded = coded(&templates, &views);
    let turtle = coded.topology().to_turtle();
    for kind in VIEWS {
        let door = format!("ik:endpointName \"{}\"", view_name(kind));
        assert!(turtle.contains(&door), "{door} in:\n{turtle}");
    }
    assert!(!turtle.contains(COMPOSE_OVER), "{turtle}");
    let declared = Topology::from_turtle(&turtle).expect("a declaration");
    let space = build(&declared, &registry).expect("the declared space builds");
    assert_eq!(space.topology(), coded.topology());

    // Each door names its own view …
    let SpaceKind::EndpointSpace { doors } = &space.topology().kind else {
        panic!("an endpoint space");
    };
    for kind in VIEWS {
        let door = doors
            .iter()
            .find(|door| door.pattern == view_pattern(kind))
            .unwrap_or_else(|| panic!("a door for {kind}"));
        assert_eq!(door.endpoint, view_name(kind));
    }

    // … and answers with its own template, filled for the square the name captured.
    let kernel = Kernel::new(space);
    for kind in VIEWS {
        let square = text(&kernel, &format!("urn:example:ttt:view:{kind}:1:2")).expect(kind);
        assert_eq!(square, format!(r#"<td class="{kind}" data-at="1,2"></td>"#));
    }
}

/// A named view is described under its name, so the catalog, `urn:kernel:validate` and
/// the MCP projection see five endpoints, not one `composeOver` with five summaries. The
/// kind stays in the title.
#[test]
fn each_named_view_describes_itself_under_its_name() {
    let (templates, views) = endpoints();
    let kernel = Kernel::new(Arc::new(coded(&templates, &views)));
    for kind in VIEWS {
        let description = kernel
            .describe_pattern(&view_pattern(kind))
            .unwrap_or_else(|| panic!("{kind} describes itself"));
        assert_eq!(description.id, view_name(kind));
        assert_eq!(description.title, "Compose over a template");
        assert!(
            description
                .summary
                .contains(&format!("urn:example:ttt:template:{kind}")),
            "{}",
            description.summary
        );
        description
            .validate()
            .expect("a named view's id is a valid endpoint id");
    }
}

/// The ambiguity the name exists to remove: two unnamed views are two different
/// endpoints called `composeOver`, and the registry refuses the second, naming it.
#[test]
fn two_unnamed_views_are_refused_by_name() {
    let mut registry = Registry::new();
    registry
        .register(Arc::new(compose_over(iri("urn:example:template:a"))))
        .expect("the first is fine");
    let refused = registry
        .register(Arc::new(compose_over(iri("urn:example:template:b"))))
        .expect_err("a second, different composeOver");
    assert_eq!(
        refused,
        DeclarationError::DuplicateEndpoint {
            id: COMPOSE_OVER.to_string()
        }
    );
    assert!(refused.to_string().contains("`composeOver`"), "{refused}");
}

/// Naming is opt-in: an unnamed instance is still `composeOver`, by name and by
/// description, exactly as before `named` existed.
#[test]
fn an_unnamed_view_is_still_compose_over() {
    let plain = compose_over(iri("urn:example:template:a"));
    assert_eq!(plain.name(), "composeOver");
    let description = plain.describe();
    assert_eq!(description.id, "composeOver");
    assert_eq!(description.title, "Compose over a template");
    assert_eq!(
        description.summary,
        "Fills the template `urn:example:template:a` as compose does, with the variables \
         this name captured (and any other arguments) as its template arguments."
    );
}
