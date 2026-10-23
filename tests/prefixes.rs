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
