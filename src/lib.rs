//! `ikigai-fn` — a library of reusable function endpoints.
//!
//! This is the first **ikigai module crate**: a standalone library of
//! resolvable function endpoints that a host *links in* and mounts, rather than
//! the engine shipping behaviour itself. It depends only on the published
//! `ikigai-core` kernel, uses no OS or platform APIs, and compiles to
//! `wasm32-unknown-unknown` — so the same module links into a native CLI and
//! into the in-browser WebAssembly host alike (no WASI required yet).
//!
//! Each `snake_case` constructor builds an endpoint whose `lowerCamelCase`
//! identifier matches its name — [`to_upper`] builds the `toUpper` endpoint,
//! conventionally resolved at `urn:fn:toUpper`. A host either pulls the
//! constructors and binds them at IRIs of its choosing, or mounts the whole
//! library at its conventional IRIs with [`space`].

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use futures_util::future::try_join_all;

use ikigai_core::{
    ArgRef, ArgSpec, Description, Endpoint, EndpointSpace, Error, Exact, FnEndpoint, Invocation,
    Iri, ReprType, Representation, Request, Result, UriTemplate, Verb,
};

/// The conventional `text/plain; charset=utf-8` representation type.
fn text_plain_utf8() -> ReprType {
    ReprType::new("text/plain").with_param("charset", "utf-8")
}

/// The same media type as a description-output string.
const TEXT_PLAIN_UTF8: &str = "text/plain;charset=utf-8";

// --- simple, idempotent, perfectly cacheable functions --------------------

fn to_upper_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let input = inv.inline_str("in")?;
    Ok(Representation::new(text_plain_utf8(), input.to_uppercase().into_bytes()).cacheable())
}

/// `toUpper`: upper-cases the UTF-8 string in the `in` argument.
pub fn to_upper() -> FnEndpoint {
    FnEndpoint::new("toUpper", to_upper_impl).with_description(
        Description::new("toUpper")
            .title("Upper-case")
            .summary("Upper-cases the UTF-8 text supplied in the `in` argument.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("in").summary("the text to upper-case"))
            .output(TEXT_PLAIN_UTF8),
    )
}

fn reverse_list_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let input = inv.inline_str("in")?;
    let mut items: Vec<&str> = input.split('\n').collect();
    items.reverse();
    Ok(Representation::new(text_plain_utf8(), items.join("\n").into_bytes()).cacheable())
}

/// `reverseList`: reverses the order of newline-separated items in `in`.
pub fn reverse_list() -> FnEndpoint {
    FnEndpoint::new("reverseList", reverse_list_impl).with_description(
        Description::new("reverseList")
            .title("Reverse list")
            .summary("Reverses the order of newline-separated items in the `in` argument.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("in").summary("newline-separated items"))
            .output(TEXT_PLAIN_UTF8),
    )
}

fn split_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let items = inv
        .inline_str("in")?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Representation::new(text_plain_utf8(), items.into_bytes()).cacheable())
}

/// `split`: splits the `in` argument on commas (trimming each) into a
/// newline-separated list — a *list producer* for the `..` map operator
/// (`source urn:demo:split "a, b, c" .. urn:fn:toUpper`). The newline list is the
/// same convention [`reverse_list`] reads.
pub fn split() -> FnEndpoint {
    FnEndpoint::new("split", split_impl).with_description(
        Description::new("split")
            .title("Split")
            .summary("Splits the `in` argument on commas into newline-separated items.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("in").summary("comma-separated items"))
            .output(TEXT_PLAIN_UTF8),
    )
}

fn wrap_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let text = inv.inline_str("text")?;
    Ok(Representation::new(text_plain_utf8(), format!("[{text}]").into_bytes()).cacheable())
}

/// `wrap`: surrounds the `text` argument with square brackets. Its argument is
/// deliberately named `text`, not `in`, so contract-driven routing is visible —
/// `source urn:demo:wrap hi` works only because the contract says the input goes
/// to `text` (and pipelines show their work: `… | urn:demo:wrap` → `[HI]`).
pub fn wrap() -> FnEndpoint {
    FnEndpoint::new("wrap", wrap_impl).with_description(
        Description::new("wrap")
            .title("Wrap")
            .summary("Surrounds the `text` argument with square brackets.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("text").summary("the text to wrap"))
            .output(TEXT_PLAIN_UTF8),
    )
}

fn greet_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let greeting = inv.inline_str("greeting")?;
    let name = inv.inline_str("name")?;
    Ok(Representation::new(text_plain_utf8(), format!("{greeting}, {name}").into_bytes()).cacheable())
}

/// `greet`: combines `greeting` and `name` into `"{greeting}, {name}"` — the
/// *multi-argument* function (`source urn:demo:greet greeting=Hello name=World`).
pub fn greet() -> FnEndpoint {
    FnEndpoint::new("greet", greet_impl).with_description(
        Description::new("greet")
            .title("Greet")
            .summary("Combines `greeting` and `name` into a greeting.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("greeting").summary("the salutation, e.g. Hello"))
            .input(ArgSpec::new("name").summary("who to greet"))
            .output(TEXT_PLAIN_UTF8),
    )
}

fn echo_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let message = inv
        .bindings
        .get("message")
        .ok_or_else(|| Error::MissingArgument("message".to_string()))?;
    Ok(Representation::new(text_plain_utf8(), message.as_bytes().to_vec()).cacheable())
}

/// `echo`: returns the `message` variable captured by the resolving grammar —
/// demonstrates grammar bindings (e.g. `urn:demo:echo/{message}`) flowing to an
/// endpoint.
pub fn echo() -> FnEndpoint {
    FnEndpoint::new("echo", echo_impl).with_description(
        Description::new("echo")
            .title("Echo")
            .summary("Returns the `message` segment captured from the resource identifier.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("message")
                    .summary("the text to echo, captured from the path by the resolving grammar")
                    .binding(),
            )
            .output(TEXT_PLAIN_UTF8),
    )
}

// --- compose: recursive `$a{<iri>}` resource transclusion -----------------

/// Maximum `$a{}` expansion depth — a backstop against a shape that transcludes
/// itself, directly or through a cycle.
const COMPOSE_MAX_DEPTH: usize = 32;

/// `compose`: recursive resource transclusion.
///
/// Sources the resource named by the `src` argument and expands every
/// `$a{<iri>}` marker in its (UTF-8 text) representation by resolving the
/// embedded resource through the kernel and splicing the result in — recursively,
/// so a transcluded shape may itself contain markers. A marker may carry inline
/// arguments (`$a{urn:fn:toUpper?in="resource oriented computing"}`); a literal
/// marker is written `$$a{…}` (a `$$` is a literal `$`).
///
/// The `a` is for *asynchronous*: the markers at one level are forked and joined,
/// so a kernel driven on a concurrent executor resolves them simultaneously,
/// while a single-threaded executor (the browser, for now) resolves them in turn.
///
/// The output mirrors the source's media type. It declares itself cacheable, so
/// the kernel keeps it cacheable only while every transcluded part is — one
/// volatile constituent makes the whole composite volatile, automatically.
pub struct Compose;

#[async_trait]
impl Endpoint for Compose {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        let src = inv.inline_str("src")?;
        let iri = Iri::parse(src).map_err(|e| Error::InvalidArgument {
            name: "src".to_string(),
            detail: format!("not an IRI: {e}"),
        })?;
        let shape = inv.source(&iri).await?;
        let Representation {
            repr_type, bytes, ..
        } = shape;
        let text = String::from_utf8(bytes).map_err(|_| {
            Error::Endpoint(format!("compose: `{}` is not UTF-8 text", iri.as_str()))
        })?;
        let expanded = expand(inv, text, 0).await?;
        Ok(Representation::new(repr_type, expanded.into_bytes()).cacheable())
    }

    fn name(&self) -> &str {
        "compose"
    }

    fn describe(&self) -> Description {
        Description::new("compose")
            .title("Compose")
            .summary(
                "Recursively expands `$a{<iri>}` transclusion markers in the resource named by \
                 the `src` argument, resolving each embedded resource through the kernel and \
                 splicing it in. A literal marker is written `$$a{…}`. The output mirrors the \
                 source's media type and stays cacheable only while every transcluded part is.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("src").summary("the IRI of the shape resource to compose"))
            .output("text/html")
    }
}

/// `compose`: recursive `$a{<iri>}` resource transclusion. See [`Compose`].
pub fn compose() -> Compose {
    Compose
}

/// One piece of a scanned shape: literal text, or a marker body to resolve.
enum Segment {
    Lit(String),
    Marker(String),
}

/// Split `text` into ordered literal/marker segments. `$$` collapses to a literal
/// `$` (so `$$a{…}` becomes the literal text `$a{…}`); `$a{ … }` becomes a marker
/// holding its inner `<iri>[?args]`; an unterminated `$a{` stays literal.
fn scan(text: &str) -> Vec<Segment> {
    let b = text.as_bytes();
    let mut segments = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            // `$$` is a literal `$` — collapses the escape for `$a{`.
            if b.get(i + 1) == Some(&b'$') {
                lit.push('$');
                i += 2;
                continue;
            }
            // `$a{ … }` — a transclusion marker.
            if text[i..].starts_with("$a{") {
                let brace = i + 2; // the `{`
                if let Some(close) = find_marker_end(text, brace) {
                    if !lit.is_empty() {
                        segments.push(Segment::Lit(std::mem::take(&mut lit)));
                    }
                    segments.push(Segment::Marker(text[brace + 1..close].trim().to_string()));
                    i = close + 1;
                    continue;
                }
                // Unterminated marker: fall through and keep the `$` literal.
            }
        }
        let len = utf8_len(b[i]);
        lit.push_str(&text[i..i + len]);
        i += len;
    }
    if !lit.is_empty() {
        segments.push(Segment::Lit(lit));
    }
    segments
}

/// Expand every `$a{<iri>}` marker in `text`. The `a` is for *asynchronous*: a
/// level's markers are forked and joined, so a concurrency-capable kernel pulls
/// them simultaneously, while a single-threaded executor (the browser, for now)
/// resolves them in turn. Each result is itself expanded, so a transcluded
/// shape's own markers recurse. Boxed for the async recursion.
fn expand<'a>(
    inv: &'a Invocation<'_>,
    text: String,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
    Box::pin(async move {
        if depth >= COMPOSE_MAX_DEPTH {
            return Err(Error::Endpoint(format!(
                "compose: recursion limit ({COMPOSE_MAX_DEPTH}) exceeded — cyclic transclusion?"
            )));
        }
        let segments = scan(&text);
        // Fork: one resolution future per marker, in document order.
        let jobs: Vec<_> = segments
            .iter()
            .filter_map(|segment| match segment {
                Segment::Marker(inner) => Some(resolve_marker(inv, inner.clone(), depth)),
                Segment::Lit(_) => None,
            })
            .collect();
        // Join: resolve the level concurrently (the kernel parallelizes if it can).
        let mut resolved = try_join_all(jobs).await?.into_iter();
        // Reassemble in document order.
        let mut out = String::with_capacity(text.len());
        for segment in &segments {
            match segment {
                Segment::Lit(t) => out.push_str(t),
                Segment::Marker(_) => {
                    out.push_str(&resolved.next().expect("one result per marker"))
                }
            }
        }
        Ok(out)
    })
}

/// Resolve one marker body `<iri>[?args]` through the kernel, then expand the
/// result so a transcluded shape's own markers recurse. Non-text isn't inlined.
fn resolve_marker<'a>(
    inv: &'a Invocation<'_>,
    inner: String,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
    Box::pin(async move {
        let repr = inv.issue(parse_marker(&inner)?).await?;
        match String::from_utf8(repr.bytes) {
            Ok(s) => expand(inv, s, depth + 1).await,
            Err(e) => Ok(format!(
                "<!-- compose: `{inner}` is non-text ({} bytes), not inlined -->",
                e.into_bytes().len()
            )),
        }
    })
}

/// The index of the `}` closing the marker whose `{` is at `brace`, skipping any
/// `}` inside a `"…"` span (where `\"` and `\\` are escapes). `None` if unterminated.
fn find_marker_end(text: &str, brace: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = brace + 1;
    let mut in_quote = false;
    while i < b.len() {
        match b[i] {
            b'\\' if in_quote => i += 2,
            b'"' => {
                in_quote = !in_quote;
                i += 1;
            }
            b'}' if !in_quote => return Some(i),
            c => i += utf8_len(c),
        }
    }
    None
}

/// Parse a marker body `<iri>[?k=v&…]` into a SOURCE request.
fn parse_marker(inner: &str) -> Result<Request> {
    let (iri_str, query) = match inner.split_once('?') {
        Some((iri, q)) => (iri.trim(), Some(q)),
        None => (inner, None),
    };
    let iri = Iri::parse(iri_str)
        .map_err(|e| Error::Endpoint(format!("compose: bad IRI in marker `{inner}`: {e}")))?;
    let mut request = Request::new(Verb::Source, iri);
    if let Some(q) = query {
        for (key, value) in parse_query(q)? {
            request = request.with_arg(key, ArgRef::Inline(value.into_bytes()));
        }
    }
    Ok(request)
}

/// Parse `k=v&k2="v with spaces"` marker arguments. A value may be double-quoted
/// (the quotes are stripped and `\"` / `\\` unescaped inside).
fn parse_query(query: &str) -> Result<Vec<(String, String)>> {
    let mut args = Vec::new();
    for pair in query.split('&') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').ok_or_else(|| {
            Error::Endpoint(format!(
                "compose: marker argument `{pair}` is not key=value"
            ))
        })?;
        args.push((key.trim().to_string(), unquote(value.trim())));
    }
    Ok(args)
}

/// Strip surrounding double quotes from a marker argument value, unescaping
/// `\"` and `\\`. An unquoted value is returned unchanged.
fn unquote(value: &str) -> String {
    let b = value.as_bytes();
    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
        let mut out = String::with_capacity(value.len() - 2);
        let mut chars = value[1..value.len() - 1].chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.push(chars.next().unwrap_or('\\')),
                _ => out.push(c),
            }
        }
        out
    } else {
        value.to_string()
    }
}

/// The byte length of the UTF-8 sequence starting with `first`.
fn utf8_len(first: u8) -> usize {
    match first {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    }
}

// --- the library as a mountable space -------------------------------------

/// The reusable function library as a mountable [`EndpointSpace`], binding every
/// function at its conventional IRI. A host mounts this and chains its own
/// host-specific bindings on top — `EndpointSpace::bind` is a builder, so:
///
/// ```ignore
/// let space = ikigai_fn::space()
///     .bind(Exact::new("urn:data:page"), page())
///     .bind(Exact::new("urn:host:info"), host_info(nature));
/// ```
///
/// Hosts that want different IRIs (binding authority is a host concern) can
/// instead pull the individual constructors and bind them as they like.
pub fn space() -> EndpointSpace {
    let echo_template = UriTemplate::parse("urn:demo:echo/{message}").expect("valid template");
    EndpointSpace::new()
        .bind(Exact::new("urn:fn:toUpper"), to_upper())
        .bind(Exact::new("urn:fn:reverseList"), reverse_list())
        .bind(Exact::new("urn:fn:compose"), compose())
        .bind(Exact::new("urn:demo:wrap"), wrap())
        .bind(Exact::new("urn:demo:split"), split())
        .bind(Exact::new("urn:demo:greet"), greet())
        .bind(echo_template, echo())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use ikigai_core::{Capability, Kernel};
    use std::sync::Arc;

    fn kernel() -> Kernel {
        Kernel::new(Arc::new(space()))
    }

    fn source(kernel: &Kernel, iri: &str, args: &[(&str, &[u8])]) -> Representation {
        let mut request = Request::new(Verb::Source, Iri::parse(iri).unwrap());
        for (key, value) in args {
            request = request.with_arg(*key, ArgRef::Inline(value.to_vec()));
        }
        block_on(kernel.issue(request, &Capability::root())).unwrap()
    }

    #[test]
    fn to_upper_upper_cases_the_in_argument() {
        assert_eq!(
            source(&kernel(), "urn:fn:toUpper", &[("in", b"hi there")]).bytes,
            b"HI THERE"
        );
    }

    #[test]
    fn reverse_list_reverses_newline_items() {
        assert_eq!(
            source(&kernel(), "urn:fn:reverseList", &[("in", b"a\nb\nc")]).bytes,
            b"c\nb\na"
        );
    }

    #[test]
    fn split_makes_a_newline_list_for_map() {
        assert_eq!(
            source(&kernel(), "urn:demo:split", &[("in", b"a, b ,c")]).bytes,
            b"a\nb\nc"
        );
    }

    #[test]
    fn wrap_routes_the_text_argument() {
        assert_eq!(
            source(&kernel(), "urn:demo:wrap", &[("text", b"hi")]).bytes,
            b"[hi]"
        );
    }

    #[test]
    fn greet_combines_two_arguments() {
        assert_eq!(
            source(
                &kernel(),
                "urn:demo:greet",
                &[("greeting", b"Hello"), ("name", b"World")]
            )
            .bytes,
            b"Hello, World"
        );
    }

    #[test]
    fn echo_returns_the_captured_binding() {
        assert_eq!(source(&kernel(), "urn:demo:echo/ping", &[]).bytes, b"ping");
    }
}

#[cfg(test)]
mod compose_tests {
    use super::*;
    use futures::executor::block_on;
    use ikigai_core::{Capability, Expiry, Kernel};
    use std::sync::Arc;

    /// A shape resource: returns a fixed `text/html` body (which may carry markers).
    fn shape(html: &'static str) -> FnEndpoint {
        FnEndpoint::new("shape", move |_inv: &Invocation<'_>| {
            Ok(Representation::new(ReprType::new("text/html"), html.as_bytes().to_vec()).cacheable())
        })
    }

    /// A kernel binding `compose`, `toUpper`, and a `urn:data:page` shape.
    fn kernel(page: &'static str) -> Kernel {
        let space = EndpointSpace::new()
            .bind(Exact::new("urn:fn:compose"), compose())
            .bind(Exact::new("urn:fn:toUpper"), to_upper())
            .bind(Exact::new("urn:data:page"), shape(page));
        Kernel::new(Arc::new(space))
    }

    fn compose_page(kernel: &Kernel) -> Representation {
        block_on(
            kernel.issue(
                Request::new(Verb::Source, Iri::parse("urn:fn:compose").unwrap())
                    .with_arg("src", ArgRef::Inline(b"urn:data:page".to_vec())),
                &Capability::root(),
            ),
        )
        .unwrap()
    }

    #[test]
    fn expands_a_marker_with_a_quoted_argument() {
        let rep = compose_page(&kernel(r#"<h1>$a{urn:fn:toUpper?in="hi there"}</h1>"#));
        assert_eq!(rep.bytes, b"<h1>HI THERE</h1>".to_vec());
    }

    #[test]
    fn preserves_the_source_media_type() {
        let rep = compose_page(&kernel("<p>$a{urn:fn:toUpper?in=x}</p>"));
        assert_eq!(rep.repr_type.media_type, "text/html");
    }

    #[test]
    fn a_double_dollar_keeps_a_marker_literal() {
        let rep = compose_page(&kernel("show $$a{urn:fn:toUpper?in=x} verbatim"));
        assert_eq!(rep.bytes, b"show $a{urn:fn:toUpper?in=x} verbatim".to_vec());
    }

    #[test]
    fn recurses_into_transcluded_shapes() {
        let space = EndpointSpace::new()
            .bind(Exact::new("urn:fn:compose"), compose())
            .bind(Exact::new("urn:fn:toUpper"), to_upper())
            .bind(Exact::new("urn:data:page"), shape("[$a{urn:data:inner}]"))
            .bind(
                Exact::new("urn:data:inner"),
                shape("$a{urn:fn:toUpper?in=hi}"),
            );
        let kernel = Kernel::new(Arc::new(space));
        let rep = compose_page(&kernel);
        assert_eq!(rep.bytes, b"[HI]".to_vec());
    }

    #[test]
    fn a_composite_of_cacheable_parts_is_cacheable() {
        let rep = compose_page(&kernel("<p>$a{urn:fn:toUpper?in=hi}</p>"));
        assert_eq!(rep.expiry, Expiry::Never);
    }

    #[test]
    fn text_around_and_between_markers_is_preserved() {
        let rep = compose_page(&kernel(
            "a $a{urn:fn:toUpper?in=b} c $a{urn:fn:toUpper?in=d} e",
        ));
        assert_eq!(rep.bytes, b"a B c D e".to_vec());
    }
}
