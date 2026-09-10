//! The module recipe as one test: `ikigai-conformance` walks every endpoint
//! [`ikigai_fn::space`] binds and reports every violation at once.
//!
//! ## Six pure functions, two resolvers
//!
//! `toUpper`, `reverseList`, `split`, `wrap`, `greet` and `echo` read nothing but
//! their inline arguments (or, for `echo`, the segment the grammar captured): no
//! file, network, clock or platform read. Each is declared `pure` — a cacheable
//! result with an empty golden-thread set is right for them, nothing ever needs to
//! cut it — and `cacheable`, so a future dependency that silently downgraded the
//! effective expiry is a red test rather than a ~2000× slowdown.
//!
//! `compose` and `conditional` are NOT pure. Each sources other resources through
//! the kernel (`inv.source`), and the kernel folds every sub-resolution's expiry
//! and golden threads into the result. So a composite is exactly as cacheable as
//! the shape it expands, and a conditional as cacheable as its condition and the
//! branch it takes — and no more:
//!
//! - a resource served UNDER A THREAD (an `ikigai-fs` cacheable mount names
//!   `urn:file:<path>`) makes the result cacheable under that same thread; a cut
//!   recomputes it ([`a_composite_is_as_cacheable_as_its_shape`],
//!   [`a_conditional_recomputes_when_its_condition_is_cut`]);
//! - a resource served UNCACHEABLE makes the result uncacheable: every call
//!   recomputes and nothing is ever served stale
//!   ([`over_live_resources_nothing_is_cached`]).
//!
//! Both mark their result `.cacheable()` and declare no thread of their own — the
//! thread is the resolved resource's to name and to cut. [`conforms`] walks the
//! threaded fixtures, where the suite holds them to a cache hit, byte-identical
//! results and a non-empty thread set; neither is declared `pure`, so an empty
//! thread set (a composite cached forever, surviving an edit to its shape) would
//! be a finding.
//!
//! ## Fixtures
//!
//! The suite's minimal `xsd:anyURI` value is `urn:example:conformance`, which
//! binds nothing, so `compose` and `conditional` each take a [`Fixture`] naming
//! resources this file binds beside the module. The suite walks EVERYTHING the
//! kernel binds, the fixtures included, so each describes itself the way a module
//! endpoint must (a kebab-case id, a Source action, an output) and is declared
//! `cacheable` like the rest.
//!
//! ## The two `lowerCamelCase` ids
//!
//! `toUpper` and `reverseList` are the known exception to the kebab-case naming
//! convention: they are live MCP tool names, and they move only in the coordinated
//! rename wave (`ikigai-core-PENDING.md` §1). The suite cannot opt a single id out
//! of `NAMES` (its PENDING #6), so [`conforms`] runs the check and pins the exact
//! set it reports: the rename makes this test red, on purpose, and so does a third
//! camelCase id.
//!
//! ## What the suite cannot see, pinned by hand
//!
//! - **Required is required** (PENDING #49/#99): every required by-value input,
//!   dropped from a working call, is a `MissingArgument` — including `conditional`'s
//!   `then` when `if` is false, which for four releases was demanded only on the
//!   taken side ([`required_inputs_are_required`]).
//! - **A class is enforced** (PENDING #118/#124): a non-IRI in an `xsd:anyURI`
//!   input is an `InvalidArgument` naming that input, on the UNTAKEN branch too;
//!   non-UTF-8 bytes in an `xsd:string` input likewise
//!   ([`classed_inputs_are_held_to_their_class`]).
//! - **An inherited gate passes through** (PENDING #87): `conditional` declares no
//!   capability, correctly — it enforces none of its own. A condition resource that
//!   is gated refuses under no grants with a typed `Denied`, and `conditional`
//!   propagates it unchanged rather than flattening it
//!   ([`a_gate_on_the_condition_passes_through_as_denied`]).
//!
//! No opt-outs, no module namespace (there is no RDF face).

use ikigai_conformance::{Check, Finding, Fixture, Suite};
use ikigai_core::{
    ArgRef, Capability, Description, Error, Exact, Expiry, FnEndpoint, Invocation, Iri, Kernel,
    ReprType, Representation, Request, Verb,
};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

/// The endpoints `space()` binds that are pure functions of their inputs, by
/// description id.
const PURE: [&str; 6] = ["toUpper", "reverseList", "split", "wrap", "greet", "echo"];

/// The two that resolve other resources — as cacheable as what they resolve.
const RESOLVERS: [&str; 2] = ["compose", "conditional"];

/// The ids that are not kebab-case: exactly the coordinated-wave exceptions.
const CAMEL_CASE: [&str; 2] = ["toUpper", "reverseList"];

/// The resources this file binds beside the module for the walk. Each IRI doubles
/// as the golden thread its threaded variant names — the `ikigai-fs` convention
/// (`depends_on` the resource's own IRI), so a cut is keyed on the name the caller
/// resolved.
const SHAPE_IRI: &str = "urn:conformance:shape";
const FLAG_IRI: &str = "urn:conformance:flag";
const THEN_IRI: &str = "urn:conformance:then";
const ELSE_IRI: &str = "urn:conformance:else";

const SHAPE: &str = "<p>$a{urn:iki:fn:toUpper?in=hi}</p>";
const SHAPE_EDITED: &str = "<p>$a{urn:iki:fn:toUpper?in=bye}</p>";

/// A fixture endpoint's description id, from its IRI: `urn:conformance:shape` →
/// `conformance-shape`.
fn fixture_id(iri: &'static str) -> String {
    iri.trim_start_matches("urn:").replace(':', "-")
}

/// A resource behind an IRI — what a stored shape, a flag file or a branch page is
/// to the module: bytes the module reaches through the kernel. `thread` is the
/// golden thread a store that can change names (and cuts on a change); `None` is a
/// live store that must be read every time, served uncacheable. The body sits
/// behind a lock so a test can change it in place. The suite walks this endpoint
/// beside the module's, so it describes itself the way a module endpoint must.
fn resource(
    iri: &'static str,
    media_type: &'static str,
    body: Arc<RwLock<&'static str>>,
    thread: Option<&'static str>,
) -> FnEndpoint {
    let id = fixture_id(iri);
    FnEndpoint::new(id.clone(), move |_inv: &Invocation<'_>| {
        let bytes = body.read().expect("body lock").as_bytes().to_vec();
        let repr = Representation::new(ReprType::new(media_type), bytes);
        Ok(match thread {
            Some(thread) => repr.cacheable().depends_on(thread),
            None => repr,
        })
    })
    .with_description(
        Description::new(id)
            .title("Conformance resource")
            .summary("A fixed body served as a kernel resource for the walk.")
            .verb(Verb::Source)
            .output(media_type),
    )
}

/// The module's space with the four fixture resources bound, plus handles to the
/// shape and the flag so a test can change them. `threaded` selects the store's
/// kind (see the file docs): every resource under a golden thread named after its
/// IRI, or every resource live.
struct Fixtures {
    kernel: Kernel,
    shape: Arc<RwLock<&'static str>>,
    flag: Arc<RwLock<&'static str>>,
}

fn fixtures(threaded: bool) -> Fixtures {
    let shape = Arc::new(RwLock::new(SHAPE));
    let flag = Arc::new(RwLock::new("true"));
    let thread = |iri: &'static str| threaded.then_some(iri);
    let space = ikigai_fn::space()
        .bind(
            Exact::new(SHAPE_IRI),
            resource(
                SHAPE_IRI,
                "text/html",
                Arc::clone(&shape),
                thread(SHAPE_IRI),
            ),
        )
        .bind(
            Exact::new(FLAG_IRI),
            resource(FLAG_IRI, "text/plain", Arc::clone(&flag), thread(FLAG_IRI)),
        )
        .bind(
            Exact::new(THEN_IRI),
            resource(
                THEN_IRI,
                "text/plain",
                Arc::new(RwLock::new("branch-then")),
                thread(THEN_IRI),
            ),
        )
        .bind(
            Exact::new(ELSE_IRI),
            resource(
                ELSE_IRI,
                "text/plain",
                Arc::new(RwLock::new("branch-else")),
                thread(ELSE_IRI),
            ),
        );
    Fixtures {
        kernel: Kernel::new(Arc::new(space)),
        shape,
        flag,
    }
}

fn request(iri: &str, args: &[(&str, &str)]) -> Request {
    let mut request = Request::new(Verb::Source, Iri::parse(iri).unwrap());
    for &(name, value) in args {
        request = request.with_arg(name, ArgRef::Inline(value.as_bytes().to_vec()));
    }
    request
}

fn compose_request() -> Request {
    request("urn:iki:fn:compose", &[("src", SHAPE_IRI)])
}

fn conditional_request(with_else: bool) -> Request {
    let mut args = vec![("if", FLAG_IRI), ("then", THEN_IRI)];
    if with_else {
        args.push(("else", ELSE_IRI));
    }
    request("urn:iki:fn:conditional", &args)
}

fn issue(kernel: &Kernel, request: Request) -> Result<Representation, Error> {
    futures::executor::block_on(kernel.issue(request, &Capability::root()))
}

fn text(kernel: &Kernel, request: Request) -> String {
    let repr = issue(kernel, request).expect("resolves");
    String::from_utf8(repr.bytes).expect("UTF-8")
}

// ---- the walk ------------------------------------------------------------------

#[test]
fn conforms() {
    let Fixtures { kernel, .. } = fixtures(true);
    let fixture_ids: Vec<String> = [SHAPE_IRI, FLAG_IRI, THEN_IRI, ELSE_IRI]
        .into_iter()
        .map(fixture_id)
        .collect();
    let suite = PURE
        .iter()
        .fold(Suite::new(), |suite, id| suite.pure(*id).cacheable(*id));
    let suite = RESOLVERS
        .iter()
        .map(|id| id.to_string())
        .chain(fixture_ids.iter().cloned())
        .fold(suite, |suite, id| suite.cacheable(id));
    let suite = suite
        .fixture(Fixture::new("compose", Verb::Source).arg("src", SHAPE_IRI))
        .fixture(
            Fixture::new("conditional", Verb::Source)
                .arg("if", FLAG_IRI)
                .arg("then", THEN_IRI)
                .arg("else", ELSE_IRI),
        );
    let report = suite.run_blocking(&kernel);
    eprintln!("{report}");

    // NAMES reports exactly the coordinated-wave exceptions — no more (a new
    // camelCase id), no fewer (the rename landed: retire this pin with it).
    let named: BTreeSet<&str> = report
        .of(Check::Names)
        .map(|f| f.endpoint.as_str())
        .collect();
    assert_eq!(
        named,
        CAMEL_CASE.iter().copied().collect::<BTreeSet<&str>>(),
        "the NAMES set is the two known exceptions: {report}"
    );
    let other: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.check != Check::Names)
        .collect();
    assert!(other.is_empty(), "{report}");

    // The walk saw exactly the endpoints declared above. A ninth function bound
    // without a `pure`/`cacheable` line would be held to a weaker standard (the
    // suite cannot know which endpoints it was not told about); a declared id that
    // binds nothing is a stale list. Both change this count or fail the checks.
    let declared = PURE.len() + RESOLVERS.len() + fixture_ids.len();
    assert_eq!(
        report.endpoints, declared,
        "every binding is declared: {report}"
    );
    assert_eq!(
        report.actions, declared,
        "one Source action per endpoint: {report}"
    );
}

// ---- as cacheable as what they resolve ----------------------------------------

/// Over a threaded shape the composite is cached; an edit with no cut is served
/// stale (no watcher here — that is the store's job); the store's cut of the
/// shape's thread recomputes the composite.
#[test]
fn a_composite_is_as_cacheable_as_its_shape() {
    let Fixtures { kernel, shape, .. } = fixtures(true);

    let first = issue(&kernel, compose_request()).unwrap();
    assert_eq!(first.bytes, b"<p>HI</p>");
    assert_eq!(first.expiry, Expiry::Never);
    assert!(
        first.threads().iter().any(|t| t.to_string() == SHAPE_IRI),
        "the composite carries the shape's thread: {:?}",
        first.threads()
    );
    assert!(
        kernel.is_cached(&compose_request(), &Capability::root()),
        "over a threaded shape the composite is cached"
    );

    *shape.write().expect("shape lock") = SHAPE_EDITED;
    assert_eq!(
        text(&kernel, compose_request()),
        "<p>HI</p>",
        "an edit with no cut is served from the cache"
    );

    kernel.cut(SHAPE_IRI);
    assert_eq!(
        text(&kernel, compose_request()),
        "<p>BYE</p>",
        "the cut recomputes the composite from the edited shape"
    );
}

/// Over a threaded condition the taken branch is cached; flipping the flag with no
/// cut still serves the old branch; the cut re-evaluates and takes the other.
#[test]
fn a_conditional_recomputes_when_its_condition_is_cut() {
    let Fixtures { kernel, flag, .. } = fixtures(true);

    let first = issue(&kernel, conditional_request(true)).unwrap();
    assert_eq!(first.bytes, b"branch-then");
    assert_eq!(first.expiry, Expiry::Never);
    let threads: BTreeSet<String> = first.threads().iter().map(|t| t.to_string()).collect();
    assert!(
        threads.contains(FLAG_IRI) && threads.contains(THEN_IRI),
        "the condition's and the taken branch's threads, not the untaken one's: {threads:?}"
    );
    assert!(
        !threads.contains(ELSE_IRI),
        "the untaken branch was never sourced: {threads:?}"
    );
    assert!(kernel.is_cached(&conditional_request(true), &Capability::root()));

    *flag.write().expect("flag lock") = "false";
    assert_eq!(text(&kernel, conditional_request(true)), "branch-then");

    kernel.cut(FLAG_IRI);
    assert_eq!(
        text(&kernel, conditional_request(true)),
        "branch-else",
        "after the cut the conditional re-reads `if` and takes the other branch"
    );
}

/// The no-op result (false, no `else`) is a function of `if` alone, and cached
/// under `if`'s thread like any other outcome — a cut recomputes it into the
/// `then` branch. Before 0.2.1 this one outcome was served uncacheable.
#[test]
fn a_false_condition_with_no_else_is_cached_under_the_condition() {
    let Fixtures { kernel, flag, .. } = fixtures(true);
    *flag.write().expect("flag lock") = "false";

    let first = issue(&kernel, conditional_request(false)).unwrap();
    assert!(first.bytes.is_empty());
    assert_eq!(first.expiry, Expiry::Never);
    assert!(first.threads().iter().any(|t| t.to_string() == FLAG_IRI));
    assert!(kernel.is_cached(&conditional_request(false), &Capability::root()));

    *flag.write().expect("flag lock") = "true";
    kernel.cut(FLAG_IRI);
    assert_eq!(text(&kernel, conditional_request(false)), "branch-then");
}

/// Over live resources neither result is cached: the effective expiry is the
/// least cacheable part's, every call recomputes, and a change is seen at once.
#[test]
fn over_live_resources_nothing_is_cached() {
    let Fixtures {
        kernel,
        shape,
        flag,
    } = fixtures(false);

    let composite = issue(&kernel, compose_request()).unwrap();
    assert_eq!(composite.expiry, Expiry::Always);
    assert!(!kernel.is_cached(&compose_request(), &Capability::root()));
    *shape.write().expect("shape lock") = SHAPE_EDITED;
    assert_eq!(text(&kernel, compose_request()), "<p>BYE</p>");

    let branch = issue(&kernel, conditional_request(true)).unwrap();
    assert_eq!(branch.expiry, Expiry::Always);
    assert!(!kernel.is_cached(&conditional_request(true), &Capability::root()));
    *flag.write().expect("flag lock") = "false";
    assert_eq!(text(&kernel, conditional_request(true)), "branch-else");
}

// ---- what the suite cannot see -------------------------------------------------

/// Every required by-value input, dropped from a call that works, is refused as
/// `MissingArgument` naming it — "declared required" means required.
#[test]
fn required_inputs_are_required() {
    let Fixtures { kernel, flag, .. } = fixtures(true);
    /// (endpoint IRI, a call that works, the required input to drop from it).
    type Case = (
        &'static str,
        &'static [(&'static str, &'static str)],
        &'static str,
    );
    let cases: &[Case] = &[
        ("urn:iki:fn:toUpper", &[("in", "x")], "in"),
        ("urn:iki:fn:reverseList", &[("in", "x")], "in"),
        ("urn:demo:split", &[("in", "x")], "in"),
        ("urn:demo:wrap", &[("text", "x")], "text"),
        (
            "urn:demo:greet",
            &[("greeting", "x"), ("name", "y")],
            "greeting",
        ),
        (
            "urn:demo:greet",
            &[("greeting", "x"), ("name", "y")],
            "name",
        ),
        ("urn:iki:fn:compose", &[("src", SHAPE_IRI)], "src"),
        (
            "urn:iki:fn:conditional",
            &[("if", FLAG_IRI), ("then", THEN_IRI)],
            "if",
        ),
        (
            "urn:iki:fn:conditional",
            &[("if", FLAG_IRI), ("then", THEN_IRI)],
            "then",
        ),
    ];
    for (iri, args, drop) in cases {
        assert!(
            issue(&kernel, request(iri, args)).is_ok(),
            "{iri} works with {args:?}"
        );
        let fewer: Vec<(&str, &str)> = args.iter().copied().filter(|(n, _)| n != drop).collect();
        match issue(&kernel, request(iri, &fewer)) {
            Err(Error::MissingArgument(name)) => assert_eq!(&name, drop, "{iri}"),
            other => panic!("{iri} without `{drop}`: expected MissingArgument, got {other:?}"),
        }
    }

    // `then` is required whatever `if` says: the laziness is in the sourcing, not
    // the contract. A false condition without `then` used to succeed (empty).
    *flag.write().expect("flag lock") = "false";
    kernel.cut(FLAG_IRI);
    match issue(
        &kernel,
        request("urn:iki:fn:conditional", &[("if", FLAG_IRI)]),
    ) {
        Err(Error::MissingArgument(name)) => assert_eq!(name, "then"),
        other => panic!("false `if` without `then`: expected MissingArgument, got {other:?}"),
    }
}

/// A declared class is held: a non-IRI in an `xsd:anyURI` input is an
/// `InvalidArgument` naming that input — on the branch that would not be taken
/// too — and non-UTF-8 bytes in an `xsd:string` input likewise.
#[test]
fn classed_inputs_are_held_to_their_class() {
    let Fixtures { kernel, flag, .. } = fixtures(true);
    const NOT_AN_IRI: &str = "no scheme here";
    let expect_invalid = |request: Request, input: &str| match issue(&kernel, request) {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, input),
        other => panic!("`{input}` = {NOT_AN_IRI:?}: expected InvalidArgument, got {other:?}"),
    };
    expect_invalid(request("urn:iki:fn:compose", &[("src", NOT_AN_IRI)]), "src");
    expect_invalid(
        request(
            "urn:iki:fn:conditional",
            &[("if", NOT_AN_IRI), ("then", THEN_IRI)],
        ),
        "if",
    );
    // `if` is true: `else` is the untaken branch, and still must be an IRI.
    expect_invalid(
        request(
            "urn:iki:fn:conditional",
            &[("if", FLAG_IRI), ("then", THEN_IRI), ("else", NOT_AN_IRI)],
        ),
        "else",
    );
    // `if` is false: now `then` is the untaken branch.
    *flag.write().expect("flag lock") = "false";
    kernel.cut(FLAG_IRI);
    expect_invalid(
        request(
            "urn:iki:fn:conditional",
            &[("if", FLAG_IRI), ("then", NOT_AN_IRI), ("else", ELSE_IRI)],
        ),
        "then",
    );

    let bytes = Request::new(Verb::Source, Iri::parse("urn:iki:fn:toUpper").unwrap())
        .with_arg("in", ArgRef::Inline(vec![0xff, 0xfe]));
    match issue(&kernel, bytes) {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "in"),
        other => panic!("non-UTF-8 `in`: expected InvalidArgument, got {other:?}"),
    }
}

/// `conditional` declares no capability and enforces none. A GATED condition
/// resource refuses under no grants, and the typed `Denied` passes through
/// unchanged — the finding the suite would report over such a fixture is the
/// host's gate leaking through, not a `requires` this module forgot.
#[test]
fn a_gate_on_the_condition_passes_through_as_denied() {
    const SCOPE: &str = "urn:cap:conformance:read";
    let gated = FnEndpoint::new("conformance-gated-flag", |_inv: &Invocation<'_>| {
        Ok(Representation::new(ReprType::new("text/plain"), b"true".to_vec()).cacheable())
    })
    .with_description(
        Description::new("conformance-gated-flag")
            .title("Gated flag")
            .summary("A condition resource behind a capability.")
            .verb(Verb::Source)
            .requires(SCOPE)
            .output("text/plain"),
    );
    let space = ikigai_fn::space().bind(Exact::new(FLAG_IRI), gated).bind(
        Exact::new(THEN_IRI),
        resource(
            THEN_IRI,
            "text/plain",
            Arc::new(RwLock::new("branch-then")),
            Some(THEN_IRI),
        ),
    );
    let kernel = Kernel::new(Arc::new(space));

    let none = Capability::scoped(Vec::<String>::new());
    match futures::executor::block_on(kernel.issue(conditional_request(false), &none)) {
        Err(Error::Denied(_)) => {}
        other => panic!("under no grants expected a typed Denied, got {other:?}"),
    }
    let granted = Capability::scoped([SCOPE]);
    let repr = futures::executor::block_on(kernel.issue(conditional_request(false), &granted))
        .expect("resolves under the condition's scope");
    assert_eq!(repr.bytes, b"branch-then");
}
