//! Whose authority a template runs under (ledger #609), one claim per test.
//!
//! The rule: compose is its CALLER'S deputy. The template named by `src`, and every
//! marker in it, resolve under the capability the compose request was issued with —
//! never more, and never less unless the caller narrows it. So:
//!
//! - nothing a caller cannot source directly is reachable through compose, by any
//!   path: as the template itself, through any marker letter, through a fallback, a
//!   nested compose, a `composeOver` view, a `conditional` branch, or the concurrent
//!   fan-out of a scheduled kernel;
//! - a caller about to fill a template it does not trust narrows its OWN capability
//!   first (`Capability::attenuate`, or `Invocation::issue_attenuated` from inside an
//!   endpoint), and the narrowing reaches every marker at every depth.
//!
//! Narrowing BY DEFAULT was weighed and declined. Compose cannot know what a template
//! needs (its markers are its content), so a default would have to be "nothing", and
//! templates that read gated resources on purpose would stop working: ikigai-cli's
//! `urn:data:control` transcludes `urn:kernel:cache`, which requires
//! `urn:cap:kernel:inspect`, and its alias demo transcludes the manifold "under YOUR
//! capability". Who may fill a template with what is the filler's decision, made with
//! the tool the kernel already has for it.
//!
//! Fixture names live under the IANA-reserved `urn:example:`.

mod common;

use std::sync::Arc;

use common::*;
use futures::executor::block_on;
use ikigai_core::{
    AsyncFnEndpoint, BoxFuture, Capability, Description, EndpointSpace, Error, Exact, FnEndpoint,
    Invocation, Kernel, Representation, Request, Result, Spawner, Verb,
};

/// The one grant the gated fixture requires.
const SCOPE: &str = "urn:cap:example:secret:read";

/// The gated resource's text: if it appears in an answer, the gate was crossed.
const SECRET: &str = "the-secret";

/// A resource that DECLARES `SCOPE`, so the kernel refuses it before dispatch to any
/// capability that does not grant it.
fn gated() -> FnEndpoint {
    FnEndpoint::new("example-gated", |_: &Invocation<'_>| {
        Ok(Representation::new(text_html(), SECRET.as_bytes().to_vec()).cacheable())
    })
    .with_description(
        Description::new("example-gated")
            .title("Gated")
            .summary("A resource behind a capability.")
            .verb(Verb::Source)
            .requires(SCOPE)
            .output("text/html;charset=utf-8"),
    )
}

/// `ikigai_fn::space()`, the gated resource, a public one, and every template the tests
/// fill, each at its own name.
fn space() -> EndpointSpace {
    ikigai_fn::space()
        .bind(Exact::new("urn:example:secret"), gated())
        .bind(Exact::new("urn:example:public"), html("public"))
        .bind(Exact::new("urn:example:yes"), html("true"))
        .bind(
            Exact::new("urn:example:t:h"),
            html("[$h{urn:example:secret}]"),
        )
        .bind(
            Exact::new("urn:example:t:r"),
            html("[$r{urn:example:secret}]"),
        )
        .bind(
            Exact::new("urn:example:t:a"),
            html("[$a{urn:example:secret}]"),
        )
        .bind(
            Exact::new("urn:example:t:fallback"),
            html("[$h{urn:example:secret || urn:example:public}]"),
        )
        .bind(
            Exact::new("urn:example:t:nested"),
            html("[$r{urn:iki:fn:compose?src=urn:example:t:h}]"),
        )
        .bind(
            Exact::new("urn:example:t:view"),
            html("[$r{urn:example:view}]"),
        )
        .bind(
            Exact::new("urn:example:t:branch"),
            html("[$r{urn:iki:fn:conditional?if=urn:example:yes&then=urn:example:secret}]"),
        )
        .bind(
            Exact::new("urn:example:t:public"),
            html("[$h{urn:example:public}]"),
        )
        .bind(
            Exact::new("urn:example:view"),
            ikigai_fn::compose_over(iri("urn:example:t:h")),
        )
}

fn kernel() -> Kernel {
    Kernel::new(Arc::new(space()))
}

/// Runs a spawned task when it is joined: enough for the kernel to take its concurrent
/// fan-out path, where each branch carries a CLONE of the invocation's capability.
struct InlineSpawner;

impl Spawner for InlineSpawner {
    fn spawn(&self, task: BoxFuture<()>) -> BoxFuture<()> {
        task
    }
}

fn under(kernel: &Kernel, request: Request, capability: &Capability) -> Result<String> {
    let repr = block_on(kernel.issue(request, capability))?;
    Ok(String::from_utf8(repr.bytes).expect("UTF-8"))
}

/// No grants at all: every requires-free resource, and nothing gated.
fn nothing() -> Capability {
    Capability::scoped(Vec::<String>::new())
}

fn holder() -> Capability {
    Capability::scoped([SCOPE])
}

/// Every template that reaches the secret, by every path compose has.
const REACHING: [&str; 6] = [
    "urn:example:t:h",
    "urn:example:t:r",
    "urn:example:t:a",
    "urn:example:t:nested",
    "urn:example:t:view",
    "urn:example:t:branch",
];

/// The claim, reproduced: a caller-named template's markers act with the caller's
/// authority. A holder of the grant composes a template that reads the gated resource,
/// and it resolves — through every marker letter and every composing path.
#[test]
fn a_template_runs_under_its_callers_capability() {
    let kernel = kernel();
    for template in REACHING {
        let got = under(&kernel, compose_request(template, &[]), &holder());
        assert!(
            got.as_deref().is_ok_and(|text| text.contains(SECRET)),
            "{template}: {got:?}"
        );
    }
}

/// And never more: a caller that cannot source the gated resource directly cannot reach
/// it through compose either — as `src`, or by any marker path. The refusal is the
/// kernel's typed `Denied`, not a compose error that hides what happened.
#[test]
fn compose_reaches_nothing_its_caller_cannot_source_directly() {
    let kernel = kernel();
    assert!(matches!(
        under(
            &kernel,
            source_request("urn:example:secret", &[]),
            &nothing()
        ),
        Err(Error::Denied(_))
    ));
    // As the template itself: compose as "dereference" (the tutorial's `..` map).
    let got = under(
        &kernel,
        compose_request("urn:example:secret", &[]),
        &nothing(),
    );
    assert!(matches!(got, Err(Error::Denied(_))), "as src: {got:?}");
    for template in REACHING {
        let got = under(&kernel, compose_request(template, &[]), &nothing());
        assert!(matches!(got, Err(Error::Denied(_))), "{template}: {got:?}");
    }
}

/// A fallback covers a denial without leaking what was denied: the public alternative
/// renders, and the secret never does.
#[test]
fn a_fallback_over_a_denial_renders_the_fallback_not_the_secret() {
    let kernel = kernel();
    let request = compose_request("urn:example:t:fallback", &[]);
    assert_eq!(
        under(&kernel, request.clone(), &nothing()).unwrap(),
        "[public]"
    );
    assert_eq!(under(&kernel, request, &holder()).unwrap(), "[the-secret]");
}

/// The concurrent fan-out of a scheduled kernel resolves each marker in its own task,
/// with its own copy of the capability: the same rule holds on that path.
#[test]
fn a_scheduled_kernels_fan_out_carries_the_callers_capability() {
    let kernel = Kernel::new(Arc::new(space())).into_scheduled(Arc::new(InlineSpawner));
    for template in REACHING {
        let got = under(&kernel, compose_request(template, &[]), &nothing());
        assert!(matches!(got, Err(Error::Denied(_))), "{template}: {got:?}");
        let got = under(&kernel, compose_request(template, &[]), &holder());
        assert!(
            got.as_deref().is_ok_and(|text| text.contains(SECRET)),
            "{template}: {got:?}"
        );
    }
}

/// The tool for a template the caller does not trust: narrow the capability BEFORE
/// composing, and the narrowing reaches every marker at every depth. A root caller that
/// attenuates to nothing still fills a template that reads only requires-free
/// resources, and is refused one that reaches past them.
#[test]
fn a_caller_that_narrows_first_narrows_every_marker() {
    let kernel = kernel();
    let narrowed = Capability::root().attenuate(Vec::<String>::new());
    assert_eq!(
        under(
            &kernel,
            compose_request("urn:example:t:public", &[]),
            &narrowed
        )
        .unwrap(),
        "[public]"
    );
    for template in REACHING {
        let got = under(&kernel, compose_request(template, &[]), &narrowed);
        assert!(matches!(got, Err(Error::Denied(_))), "{template}: {got:?}");
    }
}

/// The same from inside an endpoint: a host endpoint that forwards an untrusted `src` to
/// compose through `Invocation::issue_attenuated` keeps its caller's grant out of the
/// template's reach, while its own reads keep it.
#[test]
fn an_endpoint_forwarding_an_untrusted_src_narrows_with_issue_attenuated() {
    let forward = AsyncFnEndpoint::new("example-forward", |inv| {
        Box::pin(async move {
            // Its own read, under its caller's whole capability.
            let own = inv.source(&iri("urn:example:secret")).await?;
            // The untrusted template, under nothing.
            let src = inv.inline_str("src")?;
            let filled = inv
                .issue_attenuated(compose_request(src, &[]), Vec::<String>::new())
                .await?;
            let mut bytes = own.bytes;
            bytes.push(b'|');
            bytes.extend(filled.bytes);
            Ok(Representation::new(text_html(), bytes))
        })
    });
    let kernel = Kernel::new(Arc::new(
        space().bind(Exact::new("urn:example:forward"), forward),
    ));
    let forward = |src: &str| {
        under(
            &kernel,
            source_request("urn:example:forward", &[("src", src)]),
            &holder(),
        )
    };
    assert_eq!(
        forward("urn:example:t:public").unwrap(),
        "the-secret|[public]"
    );
    for template in REACHING {
        let got = forward(template);
        assert!(matches!(got, Err(Error::Denied(_))), "{template}: {got:?}");
    }
}
