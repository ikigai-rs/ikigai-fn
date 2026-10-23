//! `urn:iki:fn:each`: one template fill per element of a JSON array.
//!
//! Fixture names live under the IANA-reserved `urn:example:`.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use common::*;
use ikigai_core::{
    EndpointSpace, Error, Exact, FnEndpoint, Invocation, Kernel, ReprType, Representation,
    UriTemplate,
};
use ikigai_fn::{EACH_MAX_ELEMENTS, EACH_MAX_OUTPUT};

/// A resource that answers `body` as cacheable JSON.
fn json(body: &'static str) -> FnEndpoint {
    FnEndpoint::new("json", move |_: &Invocation<'_>| {
        Ok(
            Representation::new(ReprType::new("application/json"), body.as_bytes().to_vec())
                .cacheable(),
        )
    })
}

/// A resource that answers `body`, built once, as JSON.
fn json_owned(body: String) -> FnEndpoint {
    FnEndpoint::new("json", move |_: &Invocation<'_>| {
        Ok(
            Representation::new(ReprType::new("application/json"), body.clone().into_bytes())
                .cacheable(),
        )
    })
}

fn space() -> EndpointSpace {
    ikigai_fn::space()
        .bind(
            Exact::new("urn:example:people"),
            json(r#"[{"name":"Ada","born":1815},{"name":"<Grace>","born":1906,"index":"x"}]"#),
        )
        .bind(
            Exact::new("urn:example:scalars"),
            json(r#"["a",2,1.5,true,null]"#),
        )
        .bind(Exact::new("urn:example:nothing"), json("[]"))
        .bind(
            Exact::new("urn:example:nested-field"),
            json(r#"[{"name":"Ada"},{"name":{"first":"Grace"}}]"#),
        )
        .bind(Exact::new("urn:example:nested-element"), json(r#"[1,[2]]"#))
        .bind(
            Exact::new("urn:example:not-array"),
            json(r#"{"name":"Ada"}"#),
        )
        .bind(
            Exact::new("urn:example:hostile"),
            json(r#"[{"name":"$a{urn:example:secret}","id":"a/b?c=d#e"}]"#),
        )
        .bind(
            Exact::new("urn:example:nulls"),
            json(r#"[{"name":null,"nick":"G"}]"#),
        )
        .bind(Exact::new("urn:example:secret"), html("SECRET"))
        .bind(
            Exact::new("urn:example:row"),
            html("<li>$h{{index}}:$h{{name}}:$h{{born}}</li>"),
        )
        .bind(
            Exact::new("urn:example:cell"),
            html("[$h{{index}}=$h{{value} || {index}}]"),
        )
        .bind(Exact::new("urn:example:none"), html("<p>none</p>"))
        .bind(Exact::new("urn:example:raw-arg"), html("$r{{name}}"))
        .bind(
            Exact::new("urn:example:raw-request"),
            html("$r{urn:iki:fn:toUpper?in={name}}"),
        )
        .bind(
            Exact::new("urn:example:raw-nested"),
            html("<i>$a{urn:example:raw-arg}</i>"),
        )
        .bind(
            Exact::new("urn:example:reserved"),
            html("$h{urn:example:{+id}}"),
        )
        .bind(
            Exact::new("urn:example:fallback"),
            html("$h{{name} || {nick}}"),
        )
        .bind(
            Exact::new("urn:example:echoed"),
            html("$h{urn:demo:echo/{id}}|$h{{name}}"),
        )
        .bind(
            Exact::new("urn:example:raw-fixed"),
            html("$r{urn:example:none}$h{{name}};"),
        )
}

fn kernel() -> Kernel {
    Kernel::new(Arc::new(space()))
}

fn each(kernel: &Kernel, args: &[(&str, &str)]) -> ikigai_core::Result<String> {
    let repr = issue(kernel, source_request("urn:iki:fn:each", args))?;
    Ok(String::from_utf8(repr.bytes).expect("UTF-8"))
}

fn refusal(result: ikigai_core::Result<String>) -> String {
    result.expect_err("refused").to_string()
}

#[test]
fn each_fills_the_template_once_per_object_in_order() {
    let out = each(
        &kernel(),
        &[
            ("src", "urn:example:people"),
            ("template", "urn:example:row"),
        ],
    );
    // A field is escaped as `$h` escapes, a number is its decimal text, and `index` is the
    // position even where the element carries a field of that name.
    assert_eq!(
        out.unwrap(),
        "<li>0:Ada:1815</li><li>1:&lt;Grace&gt;:1906</li>"
    );
}

#[test]
fn a_scalar_is_value_and_null_is_absent() {
    let out = each(
        &kernel(),
        &[
            ("src", "urn:example:scalars"),
            ("template", "urn:example:cell"),
        ],
    );
    assert_eq!(out.unwrap(), "[0=a][1=2][2=1.5][3=true][4=4]");
    let out = each(
        &kernel(),
        &[
            ("src", "urn:example:nulls"),
            ("template", "urn:example:fallback"),
        ],
    );
    assert_eq!(
        out.unwrap(),
        "G",
        "a null field is absent, so its fallback renders"
    );
}

#[test]
fn the_answer_is_the_templates_media_type() {
    let repr = issue(
        &kernel(),
        source_request(
            "urn:iki:fn:each",
            &[
                ("src", "urn:example:people"),
                ("template", "urn:example:row"),
            ],
        ),
    )
    .unwrap();
    assert_eq!(repr.repr_type, text_html());
}

#[test]
fn an_empty_array_answers_nothing_or_the_empty_template() {
    let k = kernel();
    let out = each(
        &k,
        &[
            ("src", "urn:example:nothing"),
            ("template", "urn:example:row"),
        ],
    );
    assert_eq!(out.unwrap(), "");
    let out = each(
        &k,
        &[
            ("src", "urn:example:nothing"),
            ("template", "urn:example:row"),
            ("empty", "urn:example:none"),
        ],
    );
    assert_eq!(out.unwrap(), "<p>none</p>");
}

#[test]
fn the_separator_goes_between_fills_escaped() {
    let out = each(
        &kernel(),
        &[
            ("src", "urn:example:scalars"),
            ("template", "urn:example:cell"),
            ("separator", "<br>"),
        ],
    );
    assert_eq!(
        out.unwrap(),
        "[0=a]&lt;br&gt;[1=2]&lt;br&gt;[2=1.5]&lt;br&gt;[3=true]&lt;br&gt;[4=4]"
    );
}

#[test]
fn a_nested_value_is_refused_naming_the_element() {
    let k = kernel();
    let field = refusal(each(
        &k,
        &[
            ("src", "urn:example:nested-field"),
            ("template", "urn:example:row"),
        ],
    ));
    assert!(
        field.contains("element 1: a nested object or array"),
        "{field}"
    );
    let element = refusal(each(
        &k,
        &[
            ("src", "urn:example:nested-element"),
            ("template", "urn:example:cell"),
        ],
    ));
    assert!(
        element.contains("element 1: a nested object or array"),
        "{element}"
    );
    let object = refusal(each(
        &k,
        &[
            ("src", "urn:example:not-array"),
            ("template", "urn:example:row"),
        ],
    ));
    assert!(object.contains("not a JSON array"), "{object}");
}

/// A deeply nested array is refused at its first nested bracket, without descending, so
/// it cannot overflow a request thread. Run on a 2 MiB stack, a tokio worker's: an overflow
/// would abort this test binary, which fails the test as loudly as an assertion.
#[test]
fn deep_nesting_is_refused_without_recursing() {
    let deep = |prefix: &str| format!("{prefix}{}{}", "[".repeat(200_000), "]".repeat(200_000));
    let documents = [deep(""), deep(r#"[{"a":"#), deep("[1,")];
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            for document in documents {
                let k = Kernel::new(Arc::new(
                    ikigai_fn::space()
                        .bind(Exact::new("urn:example:deep"), json_owned(document))
                        .bind(Exact::new("urn:example:cell"), html("$h{{value}}")),
                ));
                let out = refusal(each(
                    &k,
                    &[
                        ("src", "urn:example:deep"),
                        ("template", "urn:example:cell"),
                    ],
                ));
                assert!(out.contains("nested object or array"), "{out}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn the_element_count_is_bounded() {
    let body = format!("[{}]", vec!["1"; EACH_MAX_ELEMENTS + 1].join(","));
    let k = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(Exact::new("urn:example:many"), json_owned(body))
            .bind(Exact::new("urn:example:cell"), html("$h{{value}}")),
    ));
    let out = refusal(each(
        &k,
        &[
            ("src", "urn:example:many"),
            ("template", "urn:example:cell"),
        ],
    ));
    assert!(out.contains("EACH_MAX_ELEMENTS"), "{out}");
    // At the bound itself, it renders.
    let body = format!("[{}]", vec!["1"; EACH_MAX_ELEMENTS].join(","));
    let k = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(Exact::new("urn:example:many"), json_owned(body))
            .bind(Exact::new("urn:example:cell"), html("$h{{value}}")),
    ));
    let out = each(
        &k,
        &[
            ("src", "urn:example:many"),
            ("template", "urn:example:cell"),
        ],
    );
    assert_eq!(out.unwrap().len(), EACH_MAX_ELEMENTS);
}

#[test]
fn the_output_size_is_bounded() {
    let field = "x".repeat(1 << 20);
    let body = format!("[{}]", vec![format!("\"{field}\""); 9].join(","));
    let k = Kernel::new(Arc::new(
        ikigai_fn::space()
            .bind(Exact::new("urn:example:big"), json_owned(body))
            .bind(Exact::new("urn:example:cell"), html("$h{{value}}")),
    ));
    let out = refusal(each(
        &k,
        &[("src", "urn:example:big"), ("template", "urn:example:cell")],
    ));
    assert!(out.contains("EACH_MAX_OUTPUT"), "{out}");
    const { assert!(9 << 20 > EACH_MAX_OUTPUT) };
}

// ---- element data is DATA ---------------------------------------------------------

#[test]
fn element_data_is_never_spliced_as_it_is() {
    let k = kernel();
    for template in [
        "urn:example:raw-arg",
        "urn:example:raw-request",
        "urn:example:raw-nested",
    ] {
        let out = refusal(each(
            &k,
            &[("src", "urn:example:people"), ("template", template)],
        ));
        assert!(
            out.contains("reach the output only through `$h`"),
            "{template}: {out}"
        );
    }
    // A `$r` that reads no element data is the template's own markup, and stays allowed.
    let out = each(
        &k,
        &[
            ("src", "urn:example:people"),
            ("template", "urn:example:raw-fixed"),
        ],
    );
    assert_eq!(out.unwrap(), "<p>none</p>Ada;<p>none</p>&lt;Grace&gt;;");
}

#[test]
fn element_data_is_never_reserved_expanded() {
    let out = refusal(each(
        &kernel(),
        &[
            ("src", "urn:example:hostile"),
            ("template", "urn:example:reserved"),
        ],
    ));
    assert!(out.contains("reserved-expanded never"), "{out}");
}

#[test]
fn a_field_names_no_resource_and_holds_no_marker() {
    // The marker in the field is text; `urn:example:secret` is never read. The `id` is one
    // percent-encoded segment of the name the template fixed: it adds no path, query or
    // fragment.
    let out = each(
        &kernel(),
        &[
            ("src", "urn:example:hostile"),
            ("template", "urn:example:echoed"),
        ],
    );
    assert_eq!(out.unwrap(), "a%2Fb%3Fc%3Dd%23e|$a{urn:example:secret}");
}

#[test]
fn the_requests_own_arguments_are_not_template_arguments() {
    // `born` is missing from no element, `name` from none; ask for a field the elements
    // lack and supply it on the request: the request's value is not used.
    let k = Kernel::new(Arc::new(
        space().bind(Exact::new("urn:example:asks"), html("$h{{title}}")),
    ));
    let err = issue(
        &k,
        source_request(
            "urn:iki:fn:each",
            &[
                ("src", "urn:example:people"),
                ("template", "urn:example:asks"),
                ("title", "injected"),
            ],
        ),
    )
    .unwrap_err();
    assert!(
        matches!(err, Error::MissingArgument(ref name) if name == "title"),
        "{err}"
    );
}

// ---- caching -------------------------------------------------------------------------

#[test]
fn each_is_cached_and_recomputes_when_its_src_or_template_changes() {
    let held = Arc::new(Mutex::new(BTreeMap::new()));
    let k = Kernel::new(Arc::new(ikigai_fn::space().bind(
        UriTemplate::parse("urn:example:store:{key}").unwrap(),
        store(Arc::clone(&held)),
    )));
    sink(&k, "urn:example:store:rows", r#"[{"n":"a"}]"#).unwrap();
    sink(&k, "urn:example:store:row", "<b>$h{{n}}</b>").unwrap();
    let request = || {
        source_request(
            "urn:iki:fn:each",
            &[
                ("src", "urn:example:store:rows"),
                ("template", "urn:example:store:row"),
            ],
        )
    };
    let read = || String::from_utf8(issue(&k, request()).unwrap().bytes).unwrap();
    assert_eq!(read(), "<b>a</b>");
    assert!(cached(&k, &request()), "pure over cacheable reads: cached");

    sink(&k, "urn:example:store:rows", r#"[{"n":"a"},{"n":"b"}]"#).unwrap();
    assert!(!cached(&k, &request()), "a write to src cuts it");
    assert_eq!(read(), "<b>a</b><b>b</b>");

    sink(&k, "urn:example:store:row", "<i>$h{{n}}</i>").unwrap();
    assert!(!cached(&k, &request()), "a write to the template cuts it");
    assert_eq!(read(), "<i>a</i><i>b</i>");
}

#[test]
fn src_and_template_must_be_iris() {
    let k = kernel();
    let err = issue(
        &k,
        source_request(
            "urn:iki:fn:each",
            &[("src", "not an iri"), ("template", "urn:example:row")],
        ),
    )
    .unwrap_err();
    assert!(
        matches!(err, Error::InvalidArgument { ref name, .. } if name == "src"),
        "{err}"
    );
    let err = issue(
        &k,
        source_request("urn:iki:fn:each", &[("src", "urn:example:people")]),
    )
    .unwrap_err();
    assert!(
        matches!(err, Error::MissingArgument(ref name) if name == "template"),
        "{err}"
    );
}
