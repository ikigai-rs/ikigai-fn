//! The template language `compose` speaks, one claim per test: the three markers, what
//! terminates, template arguments, per-marker fallbacks, and `composeOver`.
//!
//! The markers:
//!
//! | marker | splices | the spliced text's own markers |
//! |---|---|---|
//! | `$a{…}` | as it is | expanded (0.2.x's only marker, unchanged) |
//! | `$r{…}` | as it is | left alone |
//! | `$h{…}` | HTML-escaped | left alone |
//!
//! Fixture names live under the IANA-reserved `urn:example:`.

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::*;
use ikigai_core::{
    EndpointSpace, Error, Exact, Expiry, FnEndpoint, Invocation, Kernel, Representation,
    UriTemplate,
};

/// A resource that answers the name it was asked by — so a test can see exactly which
/// name a marker built.
fn name_echo() -> FnEndpoint {
    FnEndpoint::new("name", |inv: &Invocation<'_>| {
        Ok(
            Representation::new(text_html(), inv.request.target.as_str().as_bytes().to_vec())
                .cacheable(),
        )
    })
}

/// A kernel over `space` with the template under test bound at `urn:example:page`.
fn kernel_with(page: &'static str, space: EndpointSpace) -> Kernel {
    Kernel::new(Arc::new(
        space.bind(Exact::new("urn:example:page"), html(page)),
    ))
}

/// `ikigai_fn::space()` and the fixtures most tests share.
fn base() -> EndpointSpace {
    ikigai_fn::space()
        .bind(
            Exact::new("urn:example:mark"),
            html(r#"<b>"&'</b> $a{urn:example:secret}"#),
        )
        .bind(
            UriTemplate::parse("urn:example:name:{v}").unwrap(),
            name_echo(),
        )
}

fn page(kernel: &Kernel) -> ikigai_core::Result<String> {
    compose(kernel, "urn:example:page", &[])
}

// ---- escaping ------------------------------------------------------------------

/// `$h` makes a value safe in element text and in an attribute quoted either way; `'`
/// is `&#39;`, the tutorial's template spec.
#[test]
fn h_escapes_a_value_for_text_and_both_quoted_attribute_forms() {
    let kernel = kernel_with(
        r#"<p title="$h{urn:example:mark}" data-x='$h{urn:example:mark}'>$h{urn:example:mark}</p>"#,
        base(),
    );
    let escaped = "&lt;b&gt;&quot;&amp;&#39;&lt;/b&gt; $a{urn:example:secret}";
    assert_eq!(
        page(&kernel).unwrap(),
        format!(r#"<p title="{escaped}" data-x='{escaped}'>{escaped}</p>"#)
    );
}

/// `$r` splices as it is, and `$$` keeps both new markers literal.
#[test]
fn r_splices_as_it_is_and_a_double_dollar_keeps_every_marker_literal() {
    let kernel = kernel_with(
        "[$r{urn:iki:fn:toUpper?in=<i>x</i>}] $$h{urn:example:mark} $$r{urn:example:mark}",
        base(),
    );
    assert_eq!(
        page(&kernel).unwrap(),
        "[<I>X</I>] $h{urn:example:mark} $r{urn:example:mark}"
    );
}

// ---- terminate -----------------------------------------------------------------

/// Content read from an atom is not a template: through `$h` or `$r`, a marker inside it
/// is text and resolves nothing. Through `$a` — the opt-in — it IS expanded. Both ways
/// pinned, with the resource the marker names counting its reads.
#[test]
fn h_and_r_terminate_and_only_a_recurses() {
    let reads = Arc::new(AtomicUsize::new(0));
    let space = || {
        base().bind(
            Exact::new("urn:example:secret"),
            counted("SECRET", Arc::clone(&reads)),
        )
    };

    let escaped = kernel_with("$h{urn:example:mark}", space());
    assert!(page(&escaped).unwrap().ends_with("$a{urn:example:secret}"));
    let raw = kernel_with("$r{urn:example:mark}", space());
    assert!(page(&raw).unwrap().ends_with(" $a{urn:example:secret}"));
    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "a terminated splice resolved nothing"
    );

    let recursed = kernel_with("$a{urn:example:mark}", space());
    assert_eq!(page(&recursed).unwrap(), r#"<b>"&'</b> SECRET"#);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "`$a` expands what it splices"
    );
}

/// `$a` over a COMPOSED view expands it a second time: the inner compose has already
/// collapsed its `$$` escapes, so its literal marker comes back live. Pinned as the
/// behavior 0.2.x had, and as the reason `$r` is how a composed view is spliced.
#[test]
fn a_re_expands_a_composed_view_and_r_does_not() {
    let space = || {
        base()
            .bind(
                Exact::new("urn:example:inner"),
                html("shows $$a{urn:iki:fn:toUpper?in=x}"),
            )
            .bind(Exact::new("urn:example:secret"), html("SECRET"))
    };
    let over_a = kernel_with("$a{urn:iki:fn:compose?src=urn:example:inner}", space());
    assert_eq!(page(&over_a).unwrap(), "shows X");
    let over_r = kernel_with("$r{urn:iki:fn:compose?src=urn:example:inner}", space());
    assert_eq!(page(&over_r).unwrap(), "shows $a{urn:iki:fn:toUpper?in=x}");
}

// ---- template arguments ----------------------------------------------------------

/// In an IRI an argument is percent-encoded, so it cannot add a segment, a query or an
/// argument.
#[test]
fn an_argument_in_an_iri_is_percent_encoded() {
    let kernel = kernel_with("$h{urn:example:name:{v}}", base());
    assert_eq!(
        compose(&kernel, "urn:example:page", &[("v", "a b/c?d&e=f")]).unwrap(),
        "urn:example:name:a%20b%2Fc%3Fd%26e%3Df"
    );
}

/// In an unquoted value an argument is the value, whole — an `&` in it does not start a
/// second argument — and in a quoted value `{…}` is literal text.
#[test]
fn an_argument_in_a_value_is_one_value_and_a_quoted_value_is_literal() {
    let kernel = kernel_with(
        r#"$h{urn:iki:fn:toUpper?in={v}} / $h{urn:iki:fn:toUpper?in="{v}"}"#,
        base(),
    );
    assert_eq!(
        compose(&kernel, "urn:example:page", &[("v", "x&in=y")]).unwrap(),
        "X&amp;IN=Y / {V}"
    );
}

/// A body that is only an argument splices its value and resolves nothing.
#[test]
fn an_argument_alone_is_its_value() {
    let kernel = kernel_with(r#"<i id="c-$h{{x}}">$r{{x}}</i>"#, base());
    assert_eq!(
        compose(&kernel, "urn:example:page", &[("x", "<3>")]).unwrap(),
        r#"<i id="c-&lt;3&gt;"><3></i>"#
    );
}

/// An argument is a value, never a template: `$a` over one is refused, whatever the
/// argument says.
#[test]
fn a_over_an_argument_is_refused() {
    let kernel = kernel_with("$a{{x}}", base());
    let refused = compose(&kernel, "urn:example:page", &[("x", "hi")]).unwrap_err();
    assert!(
        matches!(&refused, Error::Endpoint(detail) if detail.contains("never a template")),
        "{refused:?}"
    );
}

/// An argument the request does not carry is a typed `MissingArgument`, naming it.
#[test]
fn a_missing_argument_is_missing_argument() {
    let kernel = kernel_with("$h{urn:example:name:{v}}", base());
    assert!(matches!(
        compose(&kernel, "urn:example:page", &[]),
        Err(Error::MissingArgument(name)) if name == "v"
    ));
}

/// Composed with different arguments, the same template is two cached answers.
#[test]
fn arguments_are_part_of_the_cache_key() {
    let kernel = kernel_with("<p>$h{{who}}</p>", base());
    let ann = compose_request("urn:example:page", &[("who", "Ann")]);
    let bob = compose_request("urn:example:page", &[("who", "Bob")]);
    assert_eq!(issue(&kernel, ann.clone()).unwrap().bytes, b"<p>Ann</p>");
    assert_eq!(issue(&kernel, bob.clone()).unwrap().bytes, b"<p>Bob</p>");
    assert!(cached(&kernel, &ann) && cached(&kernel, &bob));
}

// ---- per-marker errors -----------------------------------------------------------

/// With no fallback a failed marker fails the whole compose, and its error stays typed.
#[test]
fn without_a_fallback_a_failure_fails_the_whole_and_stays_typed() {
    let space = base()
        .bind(
            Exact::new("urn:example:gone"),
            failing(|| Error::NotFound("gone".into())),
        )
        .bind(
            Exact::new("urn:example:denied"),
            failing(|| Error::Denied("no".into())),
        );
    let kernel = kernel_with("a $h{urn:example:gone} b", space);
    assert!(matches!(page(&kernel), Err(Error::NotFound(_))));

    let space = base().bind(
        Exact::new("urn:example:denied"),
        failing(|| Error::Denied("no".into())),
    );
    let kernel = kernel_with("a $r{urn:example:denied} b", space);
    assert!(matches!(page(&kernel), Err(Error::Denied(_))));
}

/// A fallback standing in for an ABSENT resource is cached — and the write that makes
/// the resource exist cuts it, so the next read shows the real thing.
#[test]
fn a_fallback_for_an_absent_resource_is_cached_and_cut_when_it_appears() {
    let held = Arc::new(Mutex::new(BTreeMap::new()));
    let space = base()
        .bind(
            UriTemplate::parse("urn:example:stored:{key}").unwrap(),
            store(Arc::clone(&held)),
        )
        .bind(Exact::new("urn:example:untitled"), html("Untitled"));
    let kernel = kernel_with(
        "<h1>$h{urn:example:stored:title || urn:example:untitled}</h1>",
        space,
    );
    let request = compose_request("urn:example:page", &[]);
    assert_eq!(
        issue(&kernel, request.clone()).unwrap().bytes,
        b"<h1>Untitled</h1>"
    );
    assert!(
        cached(&kernel, &request),
        "absence is a dependency like any other"
    );

    sink(&kernel, "urn:example:stored:title", "Fish & Chips").unwrap();
    assert!(!cached(&kernel, &request), "the write cut the fallback");
    assert_eq!(
        issue(&kernel, request).unwrap().bytes,
        b"<h1>Fish &amp; Chips</h1>"
    );
}

/// A fallback standing in for a DENIAL renders, and makes the composite uncacheable: a
/// denial is not a state any write will cut.
#[test]
fn a_fallback_for_a_denial_renders_uncached() {
    let space = base()
        .bind(
            Exact::new("urn:example:denied"),
            failing(|| Error::Denied("no".into())),
        )
        .bind(Exact::new("urn:example:hidden"), html("(hidden)"));
    let kernel = kernel_with("$r{urn:example:denied || urn:example:hidden}", space);
    let request = compose_request("urn:example:page", &[]);
    let rendered = issue(&kernel, request.clone()).unwrap();
    assert_eq!(rendered.bytes, b"(hidden)");
    assert_eq!(rendered.expiry, Expiry::Always);
    assert!(!cached(&kernel, &request));
}

/// Alternatives are tried left to right; when every one fails, the LAST one's typed
/// error is what the compose reports.
#[test]
fn every_alternative_failing_reports_the_last_error() {
    let space = base()
        .bind(
            Exact::new("urn:example:gone"),
            failing(|| Error::NotFound("gone".into())),
        )
        .bind(
            Exact::new("urn:example:denied"),
            failing(|| Error::Denied("no".into())),
        );
    let kernel = kernel_with("$h{urn:example:gone || urn:example:denied}", space);
    assert!(matches!(page(&kernel), Err(Error::Denied(_))));

    let space = base()
        .bind(
            Exact::new("urn:example:gone"),
            failing(|| Error::NotFound("gone".into())),
        )
        .bind(
            Exact::new("urn:example:denied"),
            failing(|| Error::Denied("no".into())),
        );
    let kernel = kernel_with(
        "$h{urn:example:denied || urn:example:gone || urn:iki:fn:toUpper?in=third}",
        space,
    );
    assert_eq!(page(&kernel).unwrap(), "THIRD");
}

/// A fallback covers a missing argument too — an optional argument, spelled.
#[test]
fn a_fallback_covers_a_missing_argument() {
    let space = base().bind(Exact::new("urn:example:untitled"), html("Untitled"));
    let kernel = kernel_with("<h1>$h{{title} || urn:example:untitled}</h1>", space);
    assert_eq!(
        compose(&kernel, "urn:example:page", &[]).unwrap(),
        "<h1>Untitled</h1>"
    );
    assert_eq!(
        compose(&kernel, "urn:example:page", &[("title", "Mine")]).unwrap(),
        "<h1>Mine</h1>"
    );
}

/// A marker that cannot be PARSED is the template's fault, and fails the whole compose
/// whatever fallbacks it names.
#[test]
fn an_unparseable_marker_fails_the_whole_even_with_a_fallback() {
    for page_text in [
        "$h{urn:iki:fn:toUpper?oops || urn:example:mark}",
        "$h{urn:example:name:{not a name} || urn:example:mark}",
        "$h{ || urn:example:mark}",
    ] {
        let kernel = kernel_with(page_text, base());
        assert!(
            matches!(page(&kernel), Err(Error::Endpoint(_))),
            "{page_text}"
        );
    }
}

// ---- composeOver ---------------------------------------------------------------

/// A template bound at a name: the variables the name captured are its arguments, and
/// they win over an inline argument of the same name — the name is the identity.
#[test]
fn compose_over_takes_its_arguments_from_the_name() {
    let space = base()
        .bind(
            Exact::new("urn:example:template:cell"),
            html("<td>$h{{x}},$h{{y}}</td>"),
        )
        .bind(
            UriTemplate::parse("urn:example:view:cell:{x}:{y}").unwrap(),
            ikigai_fn::compose_over(iri("urn:example:template:cell")),
        );
    let kernel = Kernel::new(Arc::new(space));
    assert_eq!(
        text(&kernel, "urn:example:view:cell:0:2").unwrap(),
        "<td>0,2</td>"
    );
    let overridden = source_request("urn:example:view:cell:0:2", &[("x", "9")]);
    assert_eq!(issue(&kernel, overridden).unwrap().bytes, b"<td>0,2</td>");
    // The view is cached under its own name.
    assert!(cached(
        &kernel,
        &source_request("urn:example:view:cell:0:2", &[])
    ));
}

// ---- conditional's `equals` ------------------------------------------------------

/// `equals` branches on a value: the condition's trimmed text, exactly.
#[test]
fn conditional_equals_branches_on_a_value() {
    let space = base()
        .bind(Exact::new("urn:example:dash"), html("-\n"))
        .bind(Exact::new("urn:example:yes"), html("yes"))
        .bind(Exact::new("urn:example:no"), html("no"));
    let kernel = Kernel::new(Arc::new(space));
    let ask = |equals: &str| {
        let request = source_request(
            "urn:iki:fn:conditional",
            &[
                ("if", "urn:example:dash"),
                ("equals", equals),
                ("then", "urn:example:yes"),
                ("else", "urn:example:no"),
            ],
        );
        String::from_utf8(issue(&kernel, request).unwrap().bytes).unwrap()
    };
    assert_eq!(ask("-"), "yes");
    assert_eq!(ask("X"), "no");
    // Without `equals` a `-` is not a boolean, as before.
    let request = source_request(
        "urn:iki:fn:conditional",
        &[("if", "urn:example:dash"), ("then", "urn:example:yes")],
    );
    assert!(matches!(issue(&kernel, request), Err(Error::Endpoint(_))));
}

// ---- 0.2.x behavior, pinned ------------------------------------------------------

/// A non-text part is not inlined by `$a` or `$r`: a comment says so, as 0.2.x's `$a`
/// did. `$h` refuses it instead — a placeholder escaped into a value would be visible
/// garbage, or land inside an attribute.
#[test]
fn a_non_text_part_is_a_comment_for_a_and_r_and_refused_by_h() {
    let bytes = || {
        FnEndpoint::new("bytes", |_: &Invocation<'_>| {
            Ok(Representation::new(text_html(), vec![0xff, 0xfe]).cacheable())
        })
    };
    let space = || base().bind(Exact::new("urn:example:bytes"), bytes());
    for marker in ["a", "r"] {
        let kernel = kernel_with(
            if marker == "a" {
                "[$a{urn:example:bytes}]"
            } else {
                "[$r{urn:example:bytes}]"
            },
            space(),
        );
        assert_eq!(
            page(&kernel).unwrap(),
            "[<!-- compose: `urn:example:bytes` is non-text (2 bytes), not inlined -->]"
        );
    }
    let kernel = kernel_with("[$h{urn:example:bytes}]", space());
    assert!(matches!(page(&kernel), Err(Error::Endpoint(_))));
}

/// A `{` or `}` inside a QUOTED value is literal text, as it always was — the only place
/// 0.2.x could carry one, since an unquoted `}` ended the marker.
#[test]
fn braces_in_a_quoted_value_are_literal_as_before() {
    let kernel = kernel_with(r#"$a{urn:iki:fn:toUpper?in="{a} } {"}"#, base());
    assert_eq!(page(&kernel).unwrap(), "{A} } {");
}

/// A quoted value may now carry `&`, which 0.2.x split into a broken second argument and
/// refused: a refusal became an answer, and nothing that worked changed.
#[test]
fn an_ampersand_in_a_quoted_value_is_part_of_the_value() {
    let kernel = kernel_with(r#"$a{urn:iki:fn:toUpper?in="fish & chips"}"#, base());
    assert_eq!(page(&kernel).unwrap(), "FISH & CHIPS");
}
