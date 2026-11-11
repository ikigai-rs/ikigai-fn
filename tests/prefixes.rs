//! Prefixes as scoped names: a prefix table declared as DATA, bound into a dynamic space
//! ([`ikigai_fn::prefix_space`]) that a host injects as a corridor around composition.
//!
//! Fixture names live under the IANA-reserved `urn:example:`; `urn:json:query` is a
//! stand-in for ikigai-json's, which ikigai-fn does not depend on.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use common::*;
use futures::executor::block_on;
use ikigai_core::{
    Alias, AliasTable, AsyncFnEndpoint, Capability, Description, DynamicSpace, EndpointSpace,
    Error, Exact, FnEndpoint, Invocation, Kernel, ReprType, Representation, Request, Scope, Space,
    UriTemplate, Verb,
};
use ikigai_fn::{prefix_space, JQ_QUERY};

/// The prefix table as a declaration: one `ik:Alias` of root-resolving prefix rules over an
/// empty space. `rules` is `(prefix, namespace)`.
fn table(rules: &[(&str, &str)]) -> String {
    let mut out = String::from(
        "@prefix ik: <https://ikigai-rs.dev/ns#> .\n\
         @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n\
         <urn:example:prefixes> a ik:Alias ;\n    ik:maxHops 8 ;\n",
    );
    for i in 1..=rules.len() {
        out.push_str(&format!(
            "    ik:rewrites <urn:example:prefixes:rule:{i}> ;\n"
        ));
    }
    out.push_str("    ik:space <urn:example:prefixes:space> .\n");
    for (i, (from, to)) in rules.iter().enumerate() {
        out.push_str(&format!(
            "<urn:example:prefixes:rule:{}> a ik:RewriteRule ;\n    ik:ruleKind \"prefix\" ;\n    \
             ik:logical \"{from}\" ;\n    ik:canonical \"{to}\" ;\n    ik:resolveAt \"root\" .\n",
            i + 1
        ));
    }
    out.push_str("<urn:example:prefixes:space> a ik:EndpointSpace ;\n    ik:doors rdf:nil .\n");
    out
}

/// The book's table: `json:`, `fn:`, `ui:` and the page's own data.
const RULES: &[(&str, &str)] = &[
    ("json:", "urn:json:"),
    ("fn:", "urn:iki:fn:"),
    ("ui:", "urn:example:ui:"),
    ("page:", "urn:example:data:"),
];

/// A stand-in for `urn:json:query`: it READS `src` (so a prefixed `src` must resolve where
/// the jq endpoint runs, outside any level) and answers a fixed selection by program, as
/// ikigai-json would for this CSV-as-JSON input.
fn query() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("query", |inv| {
        Box::pin(async move {
            let src = iri(inv.inline_str("src")?);
            let q = inv.inline_str("q")?.to_string();
            let read = inv.source(&src).await?;
            let input = String::from_utf8(read.bytes).expect("UTF-8");
            let answer = match q.as_str() {
                // What jq answers for this input, CSV already transrepted to JSON.
                "map(select(.lang == \"rust\"))" => {
                    r#"[{"name":"ikigai","lang":"rust"},{"name":"oxigraph","lang":"rust"}]"#
                        .to_string()
                }
                ".[0].name" => "ikigai".to_string(),
                "length" => input.matches('{').count().to_string(),
                other => return Err(Error::Endpoint(format!("stand-in: no program {other}"))),
            };
            Ok(
                Representation::new(ReprType::new("application/json"), answer.into_bytes())
                    .cacheable(),
            )
        })
    })
}

/// A gated resource and the one grant it requires.
const SCOPE: &str = "urn:cap:example:secret:read";
fn gated() -> FnEndpoint {
    FnEndpoint::new("example-gated", |_: &Invocation<'_>| {
        Ok(Representation::new(text_html(), b"SECRET".to_vec()).cacheable())
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

/// The host: ikigai-fn, the jq stand-in, the page's data and templates, a store for the
/// prefix table, and a gated resource.
fn host(held: Arc<Mutex<BTreeMap<String, String>>>) -> EndpointSpace {
    ikigai_fn::space()
        .bind(Exact::new(JQ_QUERY), query())
        .bind(
            Exact::new("urn:example:data:repos"),
            FnEndpoint::new("repos", |_: &Invocation<'_>| {
                Ok(Representation::new(
                    ReprType::new("application/json"),
                    br#"[{"name":"ikigai","lang":"rust"},{"name":"jq","lang":"c"},{"name":"oxigraph","lang":"rust"}]"#
                        .to_vec(),
                )
                .cacheable())
            }),
        )
        .bind(
            Exact::new("urn:example:data:rust-repos"),
            ikigai_fn::compose_over(iri("urn:example:ui:select-rust")).named("rust-repos"),
        )
        .bind(
            Exact::new("urn:example:ui:select-rust"),
            FnEndpoint::new("select-rust", |_: &Invocation<'_>| {
                // A template that is itself a selection: `$r` of the jq answer, so it stays
                // JSON (the JSON is the host's own data, read through its own template).
                Ok(Representation::new(
                    ReprType::new("application/json"),
                    br#"$r{json:query?src=page:repos&q="map(select(.lang == \"rust\"))"}"#.to_vec(),
                )
                .cacheable())
            }),
        )
        .bind(
            Exact::new("urn:example:ui:row"),
            html("<tr><td>$h{{index}}</td><td>$h{{name}}</td><td>$h{{lang}}</td></tr>"),
        )
        .bind(Exact::new("urn:example:ui:no-rows"), html("<tr><td>no rows</td></tr>"))
        .bind(
            Exact::new("urn:example:ui:table"),
            html(
                "<table><caption>$j{page:repos .[0].name}</caption>\
                 $r{fn:each?src=page:rust-repos&template=ui:row&empty=ui:no-rows}</table>",
            ),
        )
        .bind(Exact::new("urn:example:secret"), gated())
        .bind(Exact::new("urn:example:public"), html("public"))
        .bind(
            UriTemplate::parse("urn:example:store:{key}").unwrap(),
            store(held),
        )
}

struct Fixture {
    kernel: Kernel,
    prefixes: Arc<DynamicSpace>,
}

/// A kernel over the host, and the prefix space built from the table in the store,
/// bound once (as a host's `drive` loop does on start).
fn fixture(rules: &[(&str, &str)]) -> Fixture {
    let held = Arc::new(Mutex::new(BTreeMap::new()));
    let kernel = Kernel::new(Arc::new(host(held)));
    sink(&kernel, "urn:example:store:prefixes", &table(rules)).unwrap();
    let prefixes = Arc::new(prefix_space("book", iri("urn:example:store:prefixes")));
    block_on(prefixes.refresh(&kernel, &Capability::root())).unwrap();
    Fixture { kernel, prefixes }
}

impl Fixture {
    /// The corridor: the one line a host adds where it composes.
    fn inside(&self) -> Scope {
        Scope::empty().with(Arc::clone(&self.prefixes) as Arc<dyn Space>)
    }

    fn issue_inside(
        &self,
        request: Request,
        capability: &Capability,
    ) -> ikigai_core::Result<String> {
        let repr = block_on(self.kernel.issue_in(request, capability, self.inside()))?;
        Ok(String::from_utf8(repr.bytes).expect("UTF-8"))
    }

    fn compose_inside(&self, src: &str) -> ikigai_core::Result<String> {
        self.issue_inside(compose_request(src, &[]), &Capability::root())
    }
}

/// THE EXAMPLE: a table view — `each` over a jq selection of (CSV-as-JSON) data, written
/// with prefixes, inside the dynamic space.
#[test]
fn a_table_view_written_with_prefixes_resolves_inside_the_space() {
    let f = fixture(RULES);
    assert_eq!(
        f.compose_inside("urn:example:ui:table").unwrap(),
        "<table><caption>ikigai</caption>\
         <tr><td>0</td><td>ikigai</td><td>rust</td></tr>\
         <tr><td>1</td><td>oxigraph</td><td>rust</td></tr></table>"
    );
}

#[test]
fn outside_the_space_the_short_names_do_not_exist() {
    let f = fixture(RULES);
    let err = issue(&f.kernel, compose_request("urn:example:ui:table", &[])).unwrap_err();
    assert!(
        matches!(err, Error::Unresolved(ref name) if name.as_str().starts_with("page:")),
        "{err:?}"
    );
    let err = issue(&f.kernel, source_request("fn:toUpper", &[("in", "x")])).unwrap_err();
    assert!(matches!(err, Error::Unresolved(_)), "{err:?}");
    // Inside, the same name answers.
    assert_eq!(
        f.issue_inside(
            source_request("fn:toUpper", &[("in", "x")]),
            &Capability::root()
        )
        .unwrap(),
        "X"
    );
}

#[test]
fn a_request_argument_cannot_add_or_change_a_prefix() {
    let f = fixture(RULES);
    // Arguments that look like table edits are just template arguments: the table is read
    // from its declaration resource, under the host's capability, and nowhere else.
    let request = compose_request(
        "urn:example:ui:table",
        &[
            ("json:", "urn:example:"),
            ("data", "urn:example:secret"),
            ("prefix", "evil:"),
        ],
    );
    assert!(f
        .issue_inside(request, &Capability::root())
        .unwrap()
        .contains("<caption>ikigai</caption>"));
    // A name under a prefix the table does not declare is not a name, whatever the request says.
    let err = f
        .issue_inside(
            source_request("evil:secret", &[("prefix", "evil:")]),
            &Capability::root(),
        )
        .unwrap_err();
    assert!(matches!(err, Error::Unresolved(_)), "{err:?}");
    assert_eq!(f.prefixes.status().installs, 1, "nothing rebound the table");
}

#[test]
fn a_prefix_under_a_scheme_the_host_uses_is_refused_and_the_table_in_force_stays() {
    let f = fixture(RULES);
    for (from, to) in [
        ("urn:", "urn:example:"),
        ("urn:json:", "urn:example:"),
        ("http:", "urn:example:"),
        ("HTTPS:", "urn:example:"),
        ("file:", "urn:example:"),
        ("no-colon", "urn:example:"),
    ] {
        sink(
            &f.kernel,
            "urn:example:store:prefixes",
            &table(&[(from, to)]),
        )
        .unwrap();
        let refused = block_on(f.prefixes.refresh(&f.kernel, &Capability::root()));
        assert!(refused.is_err(), "`{from}` must be refused");
        assert!(
            f.prefixes.status().refusal.is_some(),
            "the refusal is in the status for `{from}`"
        );
    }
    // The host's own names are untouched inside the corridor, and the table in force is the
    // first one.
    assert_eq!(
        f.issue_inside(
            source_request("urn:iki:fn:toUpper", &[("in", "x")]),
            &Capability::root()
        )
        .unwrap(),
        "X"
    );
    assert!(f
        .compose_inside("urn:example:ui:table")
        .unwrap()
        .contains("ikigai"));
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn a_table_that_binds_anything_or_resolves_in_its_child_is_refused() {
    let f = fixture(RULES);
    // A door can never bind (the space's registry is empty, so core refuses it first); the
    // check refuses the shape too, as defense in depth.
    let doored = Alias::new(
        Arc::new(AliasTable::new().prefix_at_root("json:", "urn:json:")),
        Arc::new(EndpointSpace::new().bind(Exact::new("urn:example:x"), ikigai_fn::to_upper())),
    )
    .topology();
    assert!(block_on(f.prefixes.install(&f.kernel, &doored.to_turtle())).is_err());
    let refusal = block_on(ikigai_fn::prefix_check()(doored)).unwrap_err();
    assert!(refusal.contains("binds nothing"), "{refusal}");
    let bare = EndpointSpace::new().topology();
    let refusal = block_on(ikigai_fn::prefix_check()(bare)).unwrap_err();
    assert!(refusal.contains("one ik:Alias"), "{refusal}");
    let child = table(RULES).replace(
        "    ik:resolveAt \"root\" .\n<urn:example:prefixes:space>",
        "    ik:resolveAt \"child\" .\n<urn:example:prefixes:space>",
    );
    assert_ne!(child, table(RULES));
    let refusal = block_on(f.prefixes.install(&f.kernel, &child)).unwrap_err();
    assert!(
        format!("{refusal:?}").contains("resolve from the root"),
        "{refusal:?}"
    );
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn a_prefix_grants_nothing() {
    let f = fixture(&[("sec:", "urn:example:")]);
    let nothing = Capability::scoped(Vec::<String>::new());
    let holder = Capability::scoped([SCOPE]);
    // Directly through the short name: refused as the full name is.
    let err = f
        .issue_inside(source_request("sec:secret", &[]), &nothing)
        .unwrap_err();
    assert!(matches!(err, Error::Denied { .. }), "{err:?}");
    assert_eq!(
        f.issue_inside(source_request("sec:secret", &[]), &holder)
            .unwrap(),
        "SECRET"
    );
    // Through a template: the fallback renders, never the secret.
    let held = Arc::new(Mutex::new(BTreeMap::new()));
    let kernel = Kernel::new(Arc::new(host(held).bind(
        Exact::new("urn:example:t"),
        html("[$h{sec:secret || sec:public}]"),
    )));
    sink(
        &kernel,
        "urn:example:store:prefixes",
        &table(&[("sec:", "urn:example:")]),
    )
    .unwrap();
    let prefixes = Arc::new(prefix_space("grants", iri("urn:example:store:prefixes")));
    block_on(prefixes.refresh(&kernel, &Capability::root())).unwrap();
    let scope = Scope::empty().with(prefixes as Arc<dyn Space>);
    let out =
        block_on(kernel.issue_in(compose_request("urn:example:t", &[]), &nothing, scope)).unwrap();
    assert_eq!(out.bytes, b"[public]");
}

#[test]
fn changing_the_table_recomputes_what_was_composed_with_it() {
    let f = fixture(RULES);
    let request = || compose_request("urn:example:ui:table", &[]);
    f.issue_inside(request(), &Capability::root()).unwrap();
    assert!(f
        .kernel
        .is_cached_in(&request(), &Capability::root(), &f.inside()));

    // Move `ui:` somewhere with no templates: a write to the declaration, then the host's
    // re-read (what `drive` does on the cut).
    let moved: Vec<(&str, &str)> = RULES
        .iter()
        .map(|&(from, to)| {
            if from == "ui:" {
                (from, "urn:example:elsewhere:")
            } else {
                (from, to)
            }
        })
        .collect();
    sink(&f.kernel, "urn:example:store:prefixes", &table(&moved)).unwrap();
    block_on(f.prefixes.refresh(&f.kernel, &Capability::root())).unwrap();
    assert!(
        !f.kernel
            .is_cached_in(&request(), &Capability::root(), &f.inside()),
        "the bind cut the space's thread"
    );
    let err = f.issue_inside(request(), &Capability::root()).unwrap_err();
    assert!(
        matches!(err, Error::Unresolved(ref name) if name.as_str().starts_with("urn:example:elsewhere:")),
        "{err:?}"
    );
}

/// The outline compiler's need (ledger #3377): a composition writes `ui:row`, and must
/// STORE the name that stands for — a face renders outside the corridor, where `ui:row`
/// does not exist. `prefix_expand` names what the corridor resolves the short name to, by
/// the table in force, and the full name answers what the short one did, with no corridor.
#[test]
fn prefix_expand_names_what_a_short_name_stands_for() {
    let f = fixture(RULES);
    let expanded = ikigai_fn::prefix_expand(&f.inside(), &iri("ui:row")).unwrap();
    assert_eq!(expanded, iri("urn:example:ui:row"));
    let short = f
        .issue_inside(source_request("ui:row", &[]), &Capability::root())
        .unwrap();
    assert_eq!(text(&f.kernel, expanded.as_str()).unwrap(), short);
    // A screen reference hidden by a prefix is recognizable once expanded.
    assert_eq!(
        ikigai_fn::prefix_expand(&f.inside(), &iri("page:screen:name:home")).unwrap(),
        iri("urn:example:data:screen:name:home")
    );
    // A full name, or one under no declared prefix, stands as written.
    for name in ["urn:example:ui:row", "evil:secret", "urn:kernel:topology"] {
        assert_eq!(
            ikigai_fn::prefix_expand(&f.inside(), &iri(name)),
            Some(iri(name))
        );
    }
    // Outside the corridor nothing expands: there `ui:row` is only `ui:row`.
    assert_eq!(
        ikigai_fn::prefix_expand(&Scope::empty(), &iri("ui:row")),
        Some(iri("ui:row"))
    );
}

#[test]
fn prefix_expand_follows_the_table_in_force_and_ignores_a_refused_one() {
    let f = fixture(RULES);
    let moved: Vec<(&str, &str)> = RULES
        .iter()
        .map(|&(from, to)| {
            if from == "ui:" {
                (from, "urn:example:elsewhere:")
            } else {
                (from, to)
            }
        })
        .collect();
    sink(&f.kernel, "urn:example:store:prefixes", &table(&moved)).unwrap();
    block_on(f.prefixes.refresh(&f.kernel, &Capability::root())).unwrap();
    assert_eq!(
        ikigai_fn::prefix_expand(&f.inside(), &iri("ui:row")),
        Some(iri("urn:example:elsewhere:row"))
    );
    // A refused table never binds, so it never expands: the one in force still does.
    sink(
        &f.kernel,
        "urn:example:store:prefixes",
        &table(&[("ui:", "urn:example:x:"), ("urn:", "urn:example:")]),
    )
    .unwrap();
    assert!(block_on(f.prefixes.refresh(&f.kernel, &Capability::root())).is_err());
    assert_eq!(
        ikigai_fn::prefix_expand(&f.inside(), &iri("ui:row")),
        Some(iri("urn:example:elsewhere:row"))
    );
    // A space that is not a prefix table (here, the host itself as a corridor) expands nothing.
    let held = Arc::new(Mutex::new(BTreeMap::new()));
    let doors = Scope::empty().with(Arc::new(host(held)) as Arc<dyn Space>);
    assert_eq!(
        ikigai_fn::prefix_expand(&doors, &iri("ui:row")),
        Some(iri("ui:row"))
    );
}

/// Expansion and resolution agree where a name does not resolve. Core refuses a cyclic
/// table at bind, but two corridors can still form a cycle between them (`a:` → `b:` in one,
/// `b:` → `a:` in the other): that expands to nothing, never to whichever name the walk
/// stopped on, and the request fails too.
#[test]
fn prefix_expand_refuses_what_resolution_refuses() {
    let f = fixture(&[("a:", "b:")]);
    sink(&f.kernel, "urn:example:store:back", &table(&[("b:", "a:")])).unwrap();
    let back = Arc::new(prefix_space("back", iri("urn:example:store:back")));
    block_on(back.refresh(&f.kernel, &Capability::root())).unwrap();
    let scope = f.inside().with(back as Arc<dyn Space>);
    assert_eq!(ikigai_fn::prefix_expand(&scope, &iri("a:x")), None);
    assert!(block_on(
        f.kernel
            .issue_in(source_request("a:x", &[]), &Capability::root(), scope)
    )
    .is_err());
}

/// Two prefix corridors: the name the first rewrites is re-issued inside both, so the second
/// may rewrite it again; the innermost corridor is consulted first.
#[test]
fn prefix_expand_walks_every_corridor_as_resolution_does() {
    let f = fixture(RULES);
    sink(
        &f.kernel,
        "urn:example:store:outer",
        &table(&[("x:", "ui:")]),
    )
    .unwrap();
    let outer = Arc::new(prefix_space("outer", iri("urn:example:store:outer")));
    block_on(outer.refresh(&f.kernel, &Capability::root())).unwrap();
    let scope = Scope::empty()
        .with(outer as Arc<dyn Space>)
        .with(Arc::clone(&f.prefixes) as Arc<dyn Space>);
    assert_eq!(
        ikigai_fn::prefix_expand(&scope, &iri("x:row")),
        Some(iri("urn:example:ui:row"))
    );
    let short = block_on(f.kernel.issue_in(
        source_request("x:row", &[]),
        &Capability::root(),
        scope,
    ))
    .unwrap();
    assert_eq!(
        String::from_utf8(short.bytes).unwrap(),
        text(&f.kernel, "urn:example:ui:row").unwrap()
    );
}

#[test]
fn overlapping_prefixes_are_refused_at_bind() {
    let f = fixture(RULES);
    let err = block_on(f.prefixes.install(
        &f.kernel,
        &table(&[("ui:", "urn:example:ui:"), ("ui:row", "urn:example:other:")]),
    ))
    .expect_err("overlapping prefixes must not bind");
    assert!(err.to_string().contains("overlap"), "{err}");
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn duplicate_prefixes_are_refused_at_bind() {
    let f = fixture(RULES);
    let err = block_on(f.prefixes.install(
        &f.kernel,
        &table(&[("ui:", "urn:example:ui:"), ("ui:", "urn:example:other:")]),
    ))
    .expect_err("duplicate prefixes must not bind");
    assert!(err.to_string().contains("duplicate"), "{err}");
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn malformed_prefixes_are_refused_with_a_reason() {
    let f = fixture(RULES);
    for (from, to, reason) in [
        ("", "urn:example:ui:", "scheme"),
        ("ui:", "relative", "IRI"),
        ("ui:", "", "IRI"),
    ] {
        let err = block_on(f.prefixes.install(&f.kernel, &table(&[(from, to)])))
            .expect_err("malformed prefixes must not bind");
        assert!(err.to_string().contains(reason), "{err}");
    }
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn only_plain_prefix_rules_can_bind() {
    let f = fixture(RULES);
    let exact = table(&[("ui:row", "urn:example:ui:row")])
        .replace("ik:ruleKind \"prefix\"", "ik:ruleKind \"exact\"");
    let named = table(&[("ui:", "urn:example:ui:")]).replace(
        "ik:resolveAt \"root\"",
        "ik:verb \"Source\" ; ik:resolveAt \"root\"",
    );
    for declaration in [named, exact] {
        let err = block_on(f.prefixes.install(&f.kernel, &declaration))
            .expect_err("a prefix table only renames a prefix, without request parts");
        assert!(err.to_string().contains("plain prefix"), "{err}");
    }
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn expansion_does_not_overflow_large_declared_hop_budgets() {
    let f = fixture(RULES);
    let declaration = table(&[("x:", "urn:example:ui:")])
        .replace("ik:maxHops 8", &format!("ik:maxHops {}", usize::MAX));
    let huge = Arc::new(prefix_space("huge", iri("urn:example:huge")));
    block_on(huge.install(&f.kernel, &declaration)).unwrap();
    let scope = f.inside().with(huge as Arc<dyn Space>);
    assert_eq!(
        ikigai_fn::prefix_expand(&scope, &iri("x:row")),
        Some(iri("urn:example:ui:row"))
    );
}

#[test]
fn expansion_refuses_a_name_answered_by_an_earlier_non_prefix_corridor() {
    let f = fixture(RULES);
    let doors = Arc::new(EndpointSpace::new().bind(Exact::new("ui:row"), html("corridor row")));
    let scope = f.inside().with(doors as Arc<dyn Space>);
    let short = block_on(f.kernel.issue_in(
        source_request("ui:row", &[]),
        &Capability::root(),
        scope.clone(),
    ))
    .unwrap();
    assert_eq!(short.bytes, b"corridor row");
    assert_ne!(
        short.bytes,
        text(&f.kernel, "urn:example:ui:row").unwrap().as_bytes()
    );
    // There is no table-derived full name for this corridor's own answer.
    assert_eq!(ikigai_fn::prefix_expand(&scope, &iri("ui:row")), None);
}

#[test]
fn expansion_stops_at_the_default_kernel_nesting_bound() {
    let f = fixture(RULES);
    let mut scope = Scope::empty();
    for i in 0..=ikigai_core::DEFAULT_MAX_DEPTH {
        let from = format!("p{i}:");
        let to = if i == ikigai_core::DEFAULT_MAX_DEPTH {
            "urn:example:ui:".to_string()
        } else {
            format!("p{}:", i + 1)
        };
        let prefixes = Arc::new(prefix_space(
            &format!("depth-{i}"),
            iri("urn:example:depth"),
        ));
        block_on(prefixes.install(&f.kernel, &table(&[(&from, &to)]))).unwrap();
        scope = scope.with(prefixes as Arc<dyn Space>);
    }
    let err = block_on(f.kernel.issue_in(
        source_request("p0:row", &[]),
        &Capability::root(),
        scope.clone(),
    ))
    .unwrap_err();
    assert!(matches!(err, Error::DepthExceeded { .. }), "{err}");
    assert_eq!(ikigai_fn::prefix_expand(&scope, &iri("p0:row")), None);
    assert_eq!(
        ikigai_fn::prefix_expand(&scope, &iri("p1:row")),
        Some(iri("urn:example:ui:row"))
    );
}

#[test]
fn terminating_revisits_are_not_limited_by_the_sum_of_table_budgets() {
    let f = fixture(&[("a:x:", "b:"), ("a:row", "urn:example:ui:row")]);
    let back = Arc::new(prefix_space("consume", iri("urn:example:consume")));
    block_on(back.install(&f.kernel, &table(&[("b:", "a:")]))).unwrap();
    let scope = f.inside().with(back as Arc<dyn Space>);
    let name = format!("a:{}row", "x:".repeat(10));
    let resolved = block_on(f.kernel.issue_in(
        source_request(&name, &[]),
        &Capability::root(),
        scope.clone(),
    ))
    .unwrap();
    assert_eq!(
        resolved.bytes,
        text(&f.kernel, "urn:example:ui:row").unwrap().as_bytes()
    );
    assert_eq!(
        ikigai_fn::prefix_expand(&scope, &iri(&name)),
        Some(iri("urn:example:ui:row"))
    );
}

#[test]
fn a_prefix_must_itself_be_an_absolute_iri() {
    let f = fixture(RULES);
    let err = block_on(
        f.prefixes
            .install(&f.kernel, &table(&[("ui:bad space", "urn:example:ui:")])),
    )
    .expect_err("a prefix containing a space must not bind");
    assert!(err.to_string().contains("IRI"), "{err}");
    assert_eq!(f.prefixes.status().installs, 1);
}

#[test]
fn prefix_check_validates_expansion_iris_without_relying_on_install() {
    for target in ["relative", "", "urn:bad space"] {
        let topology = Alias::new(
            Arc::new(AliasTable::new().prefix_at_root("ui:", target)),
            Arc::new(EndpointSpace::new()),
        )
        .topology();
        let err = block_on(ikigai_fn::prefix_check()(topology))
            .expect_err("the public check must refuse a non-IRI expansion");
        assert!(err.contains("IRI"), "{err}");
    }
}

#[test]
fn compose_over_sources_a_short_template_and_inherits_prefixes_in_its_markers() {
    let root = ikigai_fn::space()
        .bind(
            Exact::new("urn:example:template"),
            html("<$h{fn:toUpper?in=roc}>"),
        )
        .bind(
            Exact::new("urn:example:view"),
            ikigai_fn::compose_over(iri("ui:template")),
        );
    let kernel = Kernel::new(Arc::new(root));
    let prefixes = Arc::new(prefix_space("compose-over", iri("urn:example:prefixes")));
    block_on(prefixes.install(
        &kernel,
        &table(&[("ui:", "urn:example:"), ("fn:", "urn:iki:fn:")]),
    ))
    .unwrap();
    let result = block_on(kernel.issue_in(
        source_request("urn:example:view", &[]),
        &Capability::root(),
        Scope::empty().with(prefixes as Arc<dyn Space>),
    ))
    .unwrap();
    assert_eq!(result.bytes, b"<ROC>");
    assert!(issue(&kernel, source_request("urn:example:view", &[])).is_err());
}

#[test]
fn an_inner_prefix_table_wins_over_an_outer_table_for_the_same_name() {
    let f = fixture(&[("ui:", "urn:example:wrong:")]);
    let inner = Arc::new(prefix_space("inner", iri("urn:example:inner")));
    block_on(inner.install(&f.kernel, &table(&[("ui:", "urn:example:ui:")]))).unwrap();
    let scope = f.inside().with(inner as Arc<dyn Space>);
    let resolved = block_on(f.kernel.issue_in(
        source_request("ui:row", &[]),
        &Capability::root(),
        scope.clone(),
    ))
    .unwrap();
    let expanded = ikigai_fn::prefix_expand(&scope, &iri("ui:row")).unwrap();
    assert_eq!(expanded, iri("urn:example:ui:row"));
    assert_eq!(
        resolved.bytes,
        text(&f.kernel, expanded.as_str()).unwrap().as_bytes()
    );
}
