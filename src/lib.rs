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
//! conventionally resolved at `urn:iki:fn:toUpper`. A host either pulls the
//! constructors and binds them at IRIs of its choosing, or mounts the whole
//! library at its conventional IRIs with [`space`].

pub mod html;
pub mod prefix;

pub use prefix::{prefix_check, prefix_space, RESERVED_SCHEMES};

use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use futures_util::future::try_join_all;

use ikigai_core::{
    space_iri, ArgRef, ArgSpec, Description, Endpoint, EndpointSpace, Error, Exact, FnEndpoint,
    Invocation, Iri, ReprType, Representation, Request, Result, UriTemplate, Verb,
};

/// The conventional `text/plain; charset=utf-8` representation type.
fn text_plain_utf8() -> ReprType {
    ReprType::new("text/plain").with_param("charset", "utf-8")
}

/// The same media type as a description-output string.
const TEXT_PLAIN_UTF8: &str = "text/plain;charset=utf-8";

/// The datatype every text-valued input declares. `select_action` and `urn:kernel:actions
/// types=` match on [`ArgSpec::class`] and nothing else: an input WITHOUT a class is not
/// "untyped", it is invisible to type-driven selection (a required input with no class
/// makes the whole endpoint un-inferable). So every input in [`space`] carries one, and a
/// test pins that.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The datatype of an input whose value is the IRI of another resource (compose's `src`,
/// conditional's `if`/`then`/`else`) — the argument names a resource, it does not carry one.
const XSD_ANY_URI: &str = "http://www.w3.org/2001/XMLSchema#anyURI";

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
            .input(
                ArgSpec::new("in")
                    .summary("the text to upper-case")
                    .class(XSD_STRING),
            )
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
            .input(
                ArgSpec::new("in")
                    .summary("newline-separated items")
                    .class(XSD_STRING),
            )
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
/// (`source urn:demo:split "a, b, c" .. urn:iki:fn:toUpper`). The newline list is the
/// same convention [`reverse_list`] reads.
pub fn split() -> FnEndpoint {
    FnEndpoint::new("split", split_impl).with_description(
        Description::new("split")
            .title("Split")
            .summary("Splits the `in` argument on commas into newline-separated items.")
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("in")
                    .summary("comma-separated items")
                    .class(XSD_STRING),
            )
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
            .input(
                ArgSpec::new("text")
                    .summary("the text to wrap")
                    .class(XSD_STRING),
            )
            .output(TEXT_PLAIN_UTF8),
    )
}

fn greet_impl(inv: &Invocation<'_>) -> Result<Representation> {
    let greeting = inv.inline_str("greeting")?;
    let name = inv.inline_str("name")?;
    Ok(Representation::new(
        text_plain_utf8(),
        format!("{greeting}, {name}").into_bytes(),
    )
    .cacheable())
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
            .input(
                ArgSpec::new("greeting")
                    .summary("the salutation, e.g. Hello")
                    .class(XSD_STRING),
            )
            .input(
                ArgSpec::new("name")
                    .summary("who to greet")
                    .class(XSD_STRING),
            )
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
                    .class(XSD_STRING)
                    .binding(),
            )
            .output(TEXT_PLAIN_UTF8),
    )
}

// --- compose: a request-template language over `$a{…}`, `$r{…}` and `$h{…}` ----

/// Maximum `$a{}` expansion depth — a backstop against a shape that transcludes
/// itself, directly or through a cycle.
const COMPOSE_MAX_DEPTH: usize = 32;

/// `compose`: a request-template language — NetKernel's TRL, resolved through the
/// kernel.
///
/// Sources the template named by the `src` argument and replaces every marker in its
/// (UTF-8 text) representation with the resource the marker names. There are four
/// markers, one for each way of splicing what comes back:
///
/// | marker | splices | the spliced text's own markers |
/// |---|---|---|
/// | `$a{<iri>}` | as it is — trusted markup | **expanded**: a template including a template |
/// | `$r{<iri>}` | as it is — trusted markup | left alone (terminated) |
/// | `$h{<iri>}` | as TEXT, HTML-escaped | left alone (terminated) |
/// | `$j{<src> <program>}` | a jq selection of `src`, as `$h` does | left alone (terminated) |
///
/// **`$j`** is sugar for `$h{urn:json:query?src=<src>&q=<program>&as=text/plain}`
/// ([`JQ_QUERY`], served by ikigai-json, which this crate does not depend on: the marker only
/// names it). The source is the body up to its first whitespace, and the program is the rest,
/// trimmed, as written: it runs to the marker's balanced `}` (braces outside a `"…"` string
/// nest, and a `}` inside one is text), so any jq program fits unquoted:
/// `$j{urn:example:people map(select(.age > 30)) | .[0].name}`. The source may name template
/// arguments (`{name}`, percent-encoded, as in any marker's IRI); the program is template text
/// only — `{…}` there is jq's object, and no argument reaches it. The program travels as the
/// request's `q` argument, not in the IRI, so it needs no percent-encoding. `$j` takes no
/// fallback (`||` belongs to the program), and with nothing bound at [`JQ_QUERY`] it fails as
/// an unresolved `$h` does.
///
/// **Short names.** A marker's IRI need not be a `urn:`: `$h{fn:toUpper?in=x}`,
/// `$j{page:people .[0].name}` and an [`each`]'s `src` and `template` resolve as whatever the
/// request's resolution chain makes of them. The ecosystem's way to give a composition short
/// names is a prefix table declared as data and bound into a dynamic space around it
/// ([`prefix_space`], and the [`prefix`] module): inside, `json:query` resolves to
/// `urn:json:query` under the caller's capability; outside, the name does not exist.
///
/// A marker may carry inline arguments (`$a{urn:iki:fn:toUpper?in="resource oriented
/// computing"}`); a literal `$` is written `$$`, so `$$h{…}` is the literal text `$h{…}`.
/// [`escape_markers`] writes any text that way, and [`MARKER_LETTERS`] lists the letters.
///
/// **Escaping.** `$h` escapes with [`html::html_text`]: `&`, `<`, `>`, `"` and `'` become
/// `&amp;`, `&lt;`, `&gt;`, `&quot;` and `&#39;` (and a C0 control other than tab, line feed
/// and carriage return becomes U+FFFD), so its output is safe in element text and inside an
/// attribute value quoted with either quote. It is not safe in an UNQUOTED attribute value
/// (a space ends one), and no escaping makes a URL attribute safe from a `javascript:`
/// value: quote every attribute, and put values, not whole URLs, in `href` (a whole URL
/// from data goes through [`html::safe_href`]).
///
/// **Terminate.** Only `$a` expands what it splices. Content read from an atom — user
/// data, a peer, a file — goes in through `$h` (or `$r`, for markup you trust), so a
/// stored `$a{urn:example:secret}` renders as those characters and resolves nothing. `$a`
/// is how a template opts in to recursion, and it re-expands EVERYTHING it splices —
/// including the output of a nested `compose`, whose `$$` escapes have already
/// collapsed to `$`. Splice a composed view with `$r`.
///
/// **Template arguments.** Every argument of the request other than `src` — and, for
/// [`compose_over`], every variable its name captured, which wins over an argument of
/// the same name — is a template argument, and a marker names one as `{name}`:
///
/// - in the IRI its value is percent-encoded with [`html::url_segment`], as RFC 6570
///   expands a simple variable, so an argument can never add a path segment, a query
///   or an argument:
///   `$h{urn:example:cell:{x}:{y}}`;
/// - in the IRI, `{+name}` is RFC 6570's RESERVED expansion ([`html::url_reserved`]): a
///   reserved character (`: / ? # [ ] @ ! $ & ' ( ) * + , ; =`) and a well-formed `%XX`
///   pass through, everything else is encoded, so a name with `:` in it reaches its target
///   whole: `$r{urn:example:fragment:{+page}}` with `page=item:default:5` sources
///   `urn:example:fragment:item:default:5` (ledger #2132). ⚠ **Use it only for a value the
///   HOST fixed** — a variable a [`compose_over`] binding captured from a name the host
///   bound, under a prefix the host owns. Never for a request argument: `/`, `?`, `#` and
///   `:` survive, so the value can pick any resource under the prefix. The template's own
///   text before its first argument must hold the scheme and its `:`, so a reserved
///   argument never chooses the scheme (`$r{{+x}}` is refused when parsed); `{+name}` in an
///   argument value is refused too, since `{name}` there is already verbatim;
/// - in an unquoted argument value it IS the value, verbatim, and still one value:
///   `$r{urn:iki:fn:toUpper?in={name}}`;
/// - in a quoted value it is literal text: `in="{name}"` passes six characters.
///
/// A marker whose body is only an argument splices the value and resolves nothing:
/// `id="square-$h{{x}}"`. An argument is a value and never a template, so `$a{{x}}` is
/// refused. An argument the request does not carry is [`Error::MissingArgument`].
///
/// **Per-marker errors.** By default a marker that fails fails the whole compose, with
/// the marker's own typed error (a `NotFound` stays a `NotFound`). A marker may instead
/// name fallbacks, tried left to right until one succeeds — `$h{urn:example:title ||
/// urn:example:untitled}`, or `$h{{title} || urn:example:untitled}` for a missing
/// argument — and the last alternative's error is the one reported. A fallback covers
/// everything that can go wrong with its marker (resolution, a missing argument, a `$a`
/// whose own markers fail); a marker that cannot be parsed fails the whole compose. The
/// kernel decides what a fallback is worth caching: one standing in for an absent
/// resource is cached and cut when the resource appears, and one standing in for a
/// denial or any other failure makes the composite uncacheable.
///
/// **Authority.** Compose is its caller's deputy. The template named by `src`, and every
/// marker in it at every depth, resolve under the capability the compose request was
/// issued with — never more: nothing a caller cannot source directly is reachable through
/// a template, by any marker, fallback, nested compose or view. A template therefore acts
/// with the authority of whoever FILLS it, not of whoever wrote it, which is what lets a
/// view read the viewer's own data. The other side of that rule is the caller's to hold:
/// before filling a template it does not trust — one a peer or a user could have written,
/// or a `src` forwarded from outside — narrow the capability first
/// (`Capability::attenuate`, or `Invocation::issue_attenuated` from inside an endpoint);
/// the narrowing reaches every marker. `tests/authority.rs` pins both halves (ledger
/// #609).
///
/// The `a` is for *asynchronous*: the first alternatives of a level's markers are forked
/// and joined, so a kernel driven on a concurrent executor resolves them simultaneously,
/// while a single-threaded executor (the browser, for now) resolves them in turn.
///
/// The output mirrors the source's media type. It declares itself cacheable, so the
/// kernel keeps it cacheable only while every part it read is — one volatile constituent
/// makes the whole composite volatile, automatically.
///
/// ```
/// use std::sync::Arc;
/// use futures::executor::block_on;
/// use ikigai_core::{
///     ArgRef, Capability, EndpointSpace, Exact, FnEndpoint, Invocation, Iri, Kernel,
///     ReprType, Representation, Request, Verb,
/// };
///
/// let html = |body: &'static str| {
///     FnEndpoint::new("fixed", move |_: &Invocation<'_>| {
///         Ok(Representation::new(ReprType::new("text/html"), body.as_bytes().to_vec())
///             .cacheable())
///     })
/// };
/// let space = EndpointSpace::new()
///     .bind(Exact::new("urn:iki:fn:compose"), ikigai_fn::compose())
///     .bind(Exact::new("urn:example:mark"), html(r#"<i>"&'</i>$a{urn:example:secret}"#))
///     .bind(
///         Exact::new("urn:example:page"),
///         html(r#"<b title="$h{urn:example:mark}">$h{urn:example:mark}</b>"#),
///     );
/// let kernel = Kernel::new(Arc::new(space));
/// let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:compose").unwrap())
///     .with_arg("src", ArgRef::Inline(b"urn:example:page".to_vec()));
/// let page = block_on(kernel.issue(request, &Capability::root())).unwrap();
/// // Escaped for text and for either quote, and the marker in the value is not
/// // expanded: `urn:example:secret` is not even bound.
/// let mark = "&lt;i&gt;&quot;&amp;&#39;&lt;/i&gt;$a{urn:example:secret}";
/// assert_eq!(
///     String::from_utf8(page.bytes).unwrap(),
///     format!(r#"<b title="{mark}">{mark}</b>"#)
/// );
/// ```
pub struct Compose;

#[async_trait]
impl Endpoint for Compose {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        let src = inv.inline_str("src")?;
        let iri = Iri::parse(src).map_err(|e| Error::InvalidArgument {
            name: "src".to_string(),
            detail: format!("not an IRI: {e}"),
        })?;
        compose_template(inv, &iri).await
    }

    fn name(&self) -> &str {
        "compose"
    }

    fn describe(&self) -> Description {
        Description::new("compose")
            .title("Compose")
            .summary(
                "Fills the template named by the `src` argument: `$a{<iri>}` splices a \
                 resource and expands its own markers, `$r{<iri>}` splices it as it is, \
                 `$h{<iri>}` splices it as HTML-escaped text, and `$j{<src> <program>}` \
                 splices a jq selection of `src` (urn:json:query) as `$h` does. `{name}` in a marker is \
                 a template argument — any other argument of this request — and \
                 `$h{<iri> || <fallback>}` renders the fallback when the first fails. A \
                 literal `$` is written `$$`. The output mirrors the source's media type \
                 and stays cacheable only while every part it read is.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("src")
                    .summary("the IRI of the template to fill")
                    .class(XSD_ANY_URI),
            )
            .open_arguments("each fills a {name} marker in the body")
        // No `.output(…)`. `invoke` returns the SHAPE's `repr_type` unchanged, so the
        // served type is the source's and not compose's to declare: over an HTML shape
        // it serves HTML, over a Turtle one Turtle. It declared `text/html` through
        // 0.2.1 — true of the shapes we happened to write, false as a contract, and a
        // declaration that is merely what today's fixture produced is a lie that passes
        // every check. `outputs` is a closed list in core's `Description` with no
        // pass-through spelling (core PENDING §20); until there is one, announcing
        // nothing is the only honest option, and `tests/conformance.rs` waives
        // `Check::Outputs` here and pins the pass-through by hand.
        //
        // Template arguments form an open set: the template determines their names.
    }
}

/// `compose`: fill the template named by `src`. See [`Compose`].
pub fn compose() -> Compose {
    Compose
}

/// The name every [`ComposeOver`] answers until it is [`named`](ComposeOver::named): the
/// KIND of endpoint it is.
pub const COMPOSE_OVER: &str = "composeOver";

/// `composeOver`: a template bound at a NAME — [`Compose`] with its `src` fixed.
///
/// Bind it under a URI template and the variables the name captures are the template's
/// arguments, so a parameterized view is a resource like any other:
/// `urn:example:view:square:{x}:{y}` over `urn:example:template:square` answers
/// `urn:example:view:square:0:2` with the template filled for `x=0`, `y=2`. That name is
/// what a composer that takes an IRI (`conditional`'s `then`, another template's marker)
/// can point at, and what the cache and a golden thread key on. The template is sourced
/// through the kernel, so editing it recomputes every view over it.
///
/// **One endpoint per template, so one name per template.** Every instance answers
/// [`name`](Endpoint::name) with `composeOver` unless it is [`named`](Self::named), and a
/// name is how the ecosystem tells endpoints apart: a declared space binds a door to an
/// endpoint BY NAME (`ik:endpointName`), and core's `Registry` refuses a second, different
/// endpoint under a name already taken. So an application with several views names each
/// one, and the name is also its [`describe`](Endpoint::describe) id — see there.
pub struct ComposeOver {
    src: Iri,
    name: String,
}

impl ComposeOver {
    /// Give this instance its own name (builder) — what [`name`](Endpoint::name) answers,
    /// what a declared door binds it by, and its description id. Without it the name is
    /// [`COMPOSE_OVER`].
    ///
    /// The name is published: it becomes the catalog node `urn:ikigai:endpoint:{name}`
    /// and an MCP tool name, so give it the shape of an endpoint id — short, IRI-safe,
    /// `kebab-case` by convention (`ttt-view-board`). It is not checked here; core's
    /// `Description::validate` reports an id that is not IRI-safe.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

#[async_trait]
impl Endpoint for ComposeOver {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        compose_template(inv, &self.src).await
    }

    fn name(&self) -> &str {
        &self.name
    }

    /// The description's id is the instance's NAME, and its title is the KIND.
    ///
    /// The id is what the ecosystem keys an endpoint on: the catalog subject
    /// `urn:ikigai:endpoint:{id}`, the action IRI `urn:kernel:validate` looks an endpoint
    /// up by, the MCP tool name, and the identity guard type-driven selection checks a
    /// template door against. Five views all describing themselves as `composeOver` would
    /// be ONE catalog node carrying five summaries, one validate target (whichever was
    /// found first), and one MCP tool fronting five different templates behind a
    /// synthesized selector argument, summarized as whichever template came first. So a
    /// named instance is described under its name, and each view is its own entry
    /// everywhere. The kind is not lost: the title stays "Compose over a template" for
    /// every instance, and the summary names the template. An unnamed instance describes
    /// itself exactly as it did before `named` existed.
    fn describe(&self) -> Description {
        Description::new(self.name.as_str())
            .title("Compose over a template")
            .summary(format!(
                "Fills the template `{}` as compose does, with the variables this name \
                 captured (and any other arguments) as its template arguments.",
                self.src.as_str()
            ))
            .verb(Verb::Source)
            .verb(Verb::Meta)
        // No inputs: the captured variables are the binding's, not this endpoint's, and
        // the rest are compose's open set (see `Compose::describe`). No output, for
        // compose's reason: the served type is the template's.
    }
}

/// `composeOver`: the template `src`, filled — bind it at the name of a view. See
/// [`ComposeOver`].
///
/// An application with more than one view names each one with
/// [`named`](ComposeOver::named), because a name is how a declared space finds its
/// endpoint. The name answers [`name`](Endpoint::name) and is the description's id:
///
/// ```
/// use std::sync::Arc;
/// use futures::executor::block_on;
/// use ikigai_core::{
///     Capability, Endpoint, EndpointSpace, Exact, FnEndpoint, Iri, Kernel, ReprType,
///     Representation, Request, Verb,
/// };
///
/// let board = ikigai_fn::compose_over(Iri::parse("urn:example:template:board").unwrap())
///     .named("board-view");
/// assert_eq!(board.name(), "board-view");
/// assert_eq!(board.describe().id, "board-view");
/// assert_eq!(board.describe().title, "Compose over a template");
///
/// // Unnamed, it is the kind — the name every instance shares.
/// let plain = ikigai_fn::compose_over(Iri::parse("urn:example:template:board").unwrap());
/// assert_eq!(plain.name(), ikigai_fn::COMPOSE_OVER);
/// assert_eq!(plain.describe().id, "composeOver");
///
/// // A name changes what the endpoint is called, never what it answers.
/// let template = FnEndpoint::new("board", |_| {
///     Ok(Representation::new(ReprType::new("text/html"), b"<p>$h{{who}}</p>".to_vec()))
/// });
/// let space = EndpointSpace::new()
///     .bind(Exact::new("urn:example:template:board"), template)
///     .bind(Exact::new("urn:example:view:board"), board);
/// let kernel = Kernel::new(Arc::new(space));
/// let request = Request::new(Verb::Source, Iri::parse("urn:example:view:board").unwrap())
///     .with_arg("who", ikigai_core::ArgRef::Inline(b"<X>".to_vec()));
/// let view = block_on(kernel.issue(request, &Capability::root())).unwrap();
/// assert_eq!(view.bytes, b"<p>&lt;X&gt;</p>");
/// ```
pub fn compose_over(src: Iri) -> ComposeOver {
    ComposeOver {
        src,
        name: COMPOSE_OVER.to_string(),
    }
}

/// Source the template `src` and fill it: the one body of [`Compose`] and
/// [`ComposeOver`].
async fn compose_template(inv: &Invocation<'_>, src: &Iri) -> Result<Representation> {
    let shape = inv.source(src).await?;
    let Representation {
        repr_type, bytes, ..
    } = shape;
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::Endpoint(format!("compose: `{}` is not UTF-8 text", src.as_str())))?;
    let expanded = expand(inv, Args::Request, text, 0).await?;
    Ok(Representation::new(repr_type, expanded.into_bytes()).cacheable())
}

/// `conditional`: the **lazy** sibling of [`Compose`]. Sources the `if` resource,
/// reads it as a boolean, and sources — and returns — **only** `then` (true) or the
/// optional `else` (false). The untaken branch is never invoked, so neither its
/// work nor its golden threads enter the result. `if` is always evaluated; a false
/// condition with no `else` yields an empty representation. Because each branch is
/// taken via `inv.source`, dependencies propagate: if `if`'s value later flips (its
/// thread is cut), the conditional recomputes and can take the other branch.
///
/// With `equals`, the condition is "`if`'s text, trimmed, is exactly `equals`" instead
/// of a boolean — so a template can branch on a VALUE (`if` a cell `equals=-`, it is
/// empty) with no predicate resource written for it.
///
/// The contract is checked before the branch is chosen: `then` is required whatever
/// `if` says, and `if`/`then`/`else` must each be an IRI whether or not they end up
/// sourced. Only the SOURCING is lazy.
pub struct Conditional;

#[async_trait]
impl Endpoint for Conditional {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        // The whole contract is held BEFORE the branch is chosen: `then` is required
        // and every IRI-classed input must parse as one, whichever side ends up taken.
        // Laziness is about what gets SOURCED, not about which arguments are read — a
        // `then` demanded only when `if` is true is "declared required, actually
        // optional", and an `else` parsed only when taken is a class enforced on one
        // path (ikigai-conformance PENDING #49, #118).
        let cond_iri = parse_iri(inv.inline_str("if")?, "if")?;
        let then_iri = parse_iri(inv.inline_str("then")?, "then")?;
        let else_iri = match inv.inline_str("else") {
            Ok(uri) => Some(parse_iri(uri, "else")?),
            Err(Error::MissingArgument(_)) => None,
            Err(e) => return Err(e),
        };
        let equals = match inv.inline_str("equals") {
            Ok(value) => Some(value),
            Err(Error::MissingArgument(_)) => None,
            Err(e) => return Err(e),
        };
        let verdict = inv.source(&cond_iri).await?;
        let taken = match equals {
            Some(value) => as_text(&verdict.bytes, &cond_iri)?.trim() == value,
            None => as_bool(&verdict.bytes, &cond_iri)?,
        };
        if taken {
            inv.source(&then_iri).await
        } else {
            match else_iri {
                Some(iri) => inv.source(&iri).await,
                // A false condition with no `else` is a no-op. The empty result is a
                // function of `if` alone, so it is marked cacheable: the kernel folds
                // `if`'s expiry and threads in, and a cut of `if` recomputes it — the
                // same rule as the taken branches, no less.
                None => Ok(Representation::new(text_plain_utf8(), Vec::new()).cacheable()),
            }
        }
    }

    fn name(&self) -> &str {
        "conditional"
    }

    fn describe(&self) -> Description {
        Description::new("conditional")
            .title("Conditional")
            .summary(
                "Sources `if` and reads it as a boolean (true/false/1/0/yes/no/on/off) — or, \
                 with `equals`, tests whether its trimmed text is exactly that value — then \
                 sources and returns ONLY `then` (when true) or the optional `else` (when \
                 false); the untaken branch is never invoked. A false condition with no \
                 `else` returns nothing. The lazy counterpart to compose's eager fan-out.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("if")
                    .summary(
                        "IRI of a resource whose value is a boolean (or is compared to `equals`)",
                    )
                    .class(XSD_ANY_URI),
            )
            .input(
                ArgSpec::new("then")
                    .summary("IRI to source and return when `if` is true")
                    .class(XSD_ANY_URI),
            )
            .input(
                ArgSpec::new("else")
                    .summary("IRI to source and return when `if` is false (optional)")
                    .class(XSD_ANY_URI)
                    .optional(),
            )
            .input(
                ArgSpec::new("equals")
                    .summary(
                        "compare `if`'s trimmed text to this value instead of reading it as \
                         a boolean (optional)",
                    )
                    .class(XSD_STRING)
                    .optional(),
            )
        // No `.output(…)`, for compose's reason: a taken branch is returned from
        // `inv.source` unchanged, so the served type is the branch's. The one
        // representation this endpoint authors itself — a false condition with no
        // `else` — is `text/plain; charset=utf-8`, and declaring THAT would silence
        // `Check::Outputs` (it matches what the walk's fixture happens to serve) while
        // telling a consumer that a Turtle `then` comes back as plain text. Waived and
        // pinned by hand in `tests/conformance.rs`; core PENDING §20 is the condition
        // for replacing both waivers with a real pass-through spelling.
    }
}

/// `conditional`: branch on a boolean resource, invoking only the taken side. See
/// [`Conditional`].
pub fn conditional() -> Conditional {
    Conditional
}

// --- each: one template fill per element of a JSON array ---------------------

/// The resource a `$j{<src> <program>}` marker names: ikigai-json's jq selection. The
/// marker is sugar for `$h{urn:json:query?src=<src>&q=<program>&as=text/plain}`, always
/// this full name, so it works with or without a prefix table.
pub const JQ_QUERY: &str = "urn:json:query";

/// The most elements [`each`] fills a template for: 10,000. More is refused, naming the
/// bound, before any template is filled.
pub const EACH_MAX_ELEMENTS: usize = 10_000;

/// The most fields one object element may carry: 256.
pub const EACH_MAX_FIELDS: usize = 256;

/// The most bytes [`each`] answers: 8 MiB. A longer answer is refused, naming the bound.
pub const EACH_MAX_OUTPUT: usize = 8 << 20;

/// `each`: render a collection — fill the template `template` once per element of the
/// JSON array `src` answers, and concatenate the fills in order.
///
/// - An OBJECT element's top-level fields are the template's `{arguments}`, filled exactly
///   as [`Compose`] fills request arguments: `$h{{name}}` splices one as escaped text, and
///   `{name}` in a marker's IRI is percent-encoded as one segment.
/// - A SCALAR element is `{value}`. A string is its text, a number its shortest decimal
///   form, a boolean `true` or `false`.
/// - `{index}` is the element's position, from 0. It is always the position: a field
///   named `index` does not replace it.
/// - A `null` field, or a `null` element's `value`, is ABSENT, so a fallback covers it:
///   `$h{{title} || {name}}`.
/// - A NESTED value (an object or array inside an element, or an array element) is
///   refused, naming the element. Select the flat shape first (`$j`, or `urn:json:query`).
///
/// Optional: `empty`, a template filled (with no arguments) when the array is empty —
/// without it an empty array answers nothing — and `separator`, text put between two
/// fills, HTML-escaped as `$h` escapes.
///
/// **Element data is DATA.** It reaches the output only through an escaping marker
/// (`$h`, `$j`): a template filled per element that splices an argument as it is
/// (`$r{{name}}`, or `$r{…}` over a request that names one) is refused, at every depth of
/// `$a`, and so is `{+name}`, so a field never chooses more than one percent-encoded
/// segment of a name the template fixed. The request's own arguments are not template
/// arguments here: only the element's are. Nothing in an element is ever scanned for
/// markers, so a field holding `$a{urn:example:secret}` renders as those characters.
///
/// **Authority** is compose's: `src`, the templates and every marker resolve under the
/// caller's capability.
///
/// **Cacheable** exactly when what it read is: it declares itself cacheable, and the
/// kernel keeps it so only while `src`, the template and every marker's answer are,
/// golden-threaded from each, so a write to `src` or to the template recomputes it.
///
/// **Bounds:** [`EACH_MAX_ELEMENTS`], [`EACH_MAX_FIELDS`], [`EACH_MAX_OUTPUT`]. The array
/// is read by a typed deserializer that refuses a nested value at its first bracket, so
/// it never descends below an element's fields: depth is bounded by the shape, with no
/// pre-scan needed.
///
/// The output's media type is the template's.
///
/// ```
/// use std::sync::Arc;
/// use futures::executor::block_on;
/// use ikigai_core::{
///     ArgRef, Capability, Exact, FnEndpoint, Iri, Kernel, ReprType, Representation, Request,
///     Verb,
/// };
///
/// let fixed = |media: &'static str, body: &'static str| {
///     FnEndpoint::new("fixed", move |_| {
///         Ok(Representation::new(ReprType::new(media), body.as_bytes().to_vec()).cacheable())
///     })
/// };
/// let space = ikigai_fn::space()
///     .bind(
///         Exact::new("urn:example:people"),
///         fixed("application/json", r#"[{"name":"Ada"},{"name":"<Grace>"}]"#),
///     )
///     .bind(Exact::new("urn:example:row"), fixed("text/html", "<li>$h{{index}}: $h{{name}}</li>"));
/// let kernel = Kernel::new(Arc::new(space));
/// let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:each").unwrap())
///     .with_arg("src", ArgRef::Inline(b"urn:example:people".to_vec()))
///     .with_arg("template", ArgRef::Inline(b"urn:example:row".to_vec()));
/// let list = block_on(kernel.issue(request, &Capability::root())).unwrap();
/// assert_eq!(list.bytes, b"<li>0: Ada</li><li>1: &lt;Grace&gt;</li>");
/// ```
pub struct Each;

#[async_trait]
impl Endpoint for Each {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        let src = parse_iri(inv.inline_str("src")?, "src")?;
        let template = parse_iri(inv.inline_str("template")?, "template")?;
        let empty = match inv.inline_str("empty") {
            Ok(iri) => Some(parse_iri(iri, "empty")?),
            Err(Error::MissingArgument(_)) => None,
            Err(e) => return Err(e),
        };
        let separator = match inv.inline_str("separator") {
            Ok(text) => html::html_text(text),
            Err(Error::MissingArgument(_)) => String::new(),
            Err(e) => return Err(e),
        };
        let collection = inv.source(&src).await?;
        let elements = parse_elements(&collection.bytes, &src)?;
        let shape = inv.source(&template).await?;
        if elements.is_empty() {
            if let Some(empty) = empty {
                let shape = inv.source(&empty).await?;
                let text = template_text(shape.bytes, &empty)?;
                let none = Fields::new();
                let filled = expand(inv, Args::Element(&none), text, 0).await?;
                return Ok(Representation::new(shape.repr_type, filled.into_bytes()).cacheable());
            }
            return Ok(Representation::new(shape.repr_type, Vec::new()).cacheable());
        }
        let text = template_text(shape.bytes, &template)?;
        let fills = try_join_all(
            elements
                .iter()
                .map(|fields| expand(inv, Args::Element(fields), text.clone(), 0)),
        )
        .await?;
        let mut out = String::new();
        for (i, fill) in fills.iter().enumerate() {
            if i > 0 {
                out.push_str(&separator);
            }
            out.push_str(fill);
            if out.len() > EACH_MAX_OUTPUT {
                return Err(Error::Endpoint(format!(
                    "each: the answer passed {EACH_MAX_OUTPUT} bytes (EACH_MAX_OUTPUT) at \
                     element {i} of {}",
                    fills.len()
                )));
            }
        }
        Ok(Representation::new(shape.repr_type, out.into_bytes()).cacheable())
    }

    fn name(&self) -> &str {
        "each"
    }

    fn describe(&self) -> Description {
        Description::new("each")
            .title("Each")
            .summary(
                "Fills the template `template` once per element of the JSON array `src` \
                 answers, in order: an object's top-level fields are its `{arguments}`, a \
                 scalar is `{value}`, and `{index}` is the position. Element data reaches \
                 the output only escaped (`$h`, `$j`); a nested value is refused. Optional \
                 `empty` (a template for an empty array) and `separator` (escaped text \
                 between fills). The output is the template's media type, cacheable while \
                 everything it read is.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(
                ArgSpec::new("src")
                    .summary("the IRI of a resource whose answer is a JSON array")
                    .class(XSD_ANY_URI),
            )
            .input(
                ArgSpec::new("template")
                    .summary("the IRI of the template filled once per element")
                    .class(XSD_ANY_URI),
            )
            .input(
                ArgSpec::new("empty")
                    .summary("the IRI of a template filled when the array is empty (optional)")
                    .class(XSD_ANY_URI)
                    .optional(),
            )
            .input(
                ArgSpec::new("separator")
                    .summary("text put between two fills, HTML-escaped (optional)")
                    .class(XSD_STRING)
                    .optional(),
            )
        // No `.output(…)`, for compose's reason: the served type is the template's.
    }
}

/// `each`: one template fill per element of a JSON array. See [`Each`].
pub fn each() -> Each {
    Each
}

/// A template's bytes as UTF-8 text.
fn template_text(bytes: Vec<u8>, iri: &Iri) -> Result<String> {
    String::from_utf8(bytes)
        .map_err(|_| Error::Endpoint(format!("each: `{}` is not UTF-8 text", iri.as_str())))
}

/// The elements of the JSON array `bytes`, each as its template arguments.
fn parse_elements(bytes: &[u8], src: &Iri) -> Result<Vec<Fields>> {
    use serde::de::DeserializeSeed;
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let elements = ElementsSeed
        .deserialize(&mut de)
        .and_then(|elements| de.end().map(|()| elements))
        .map_err(|e| {
            Error::Endpoint(format!(
                "each: `{}` is not a JSON array of flat objects or scalars: {e}",
                src.as_str()
            ))
        })?;
    Ok(elements)
}

/// The array: at most [`EACH_MAX_ELEMENTS`] elements, each an object or a scalar.
struct ElementsSeed;

impl<'de> serde::de::DeserializeSeed<'de> for ElementsSeed {
    type Value = Vec<Fields>;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        de: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        de.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for ElementsSeed {
    type Value = Vec<Fields>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a JSON array")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut elements = Vec::new();
        while let Some(mut fields) = seq.next_element_seed(ElementSeed(elements.len()))? {
            if elements.len() == EACH_MAX_ELEMENTS {
                return Err(serde::de::Error::custom(format!(
                    "more than {EACH_MAX_ELEMENTS} elements (EACH_MAX_ELEMENTS)"
                )));
            }
            fields.insert("index".to_string(), elements.len().to_string());
            elements.push(fields);
        }
        Ok(elements)
    }
}

/// One element, at its position: an object's fields, or a scalar as `value`.
struct ElementSeed(usize);

impl<'de> serde::de::DeserializeSeed<'de> for ElementSeed {
    type Value = Fields;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        de: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        de.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for ElementSeed {
    type Value = Fields;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "element {}: an object of scalars, or a scalar", self.0)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut fields = Fields::new();
        let mut count = 0usize;
        while let Some(key) = map.next_key::<String>()? {
            count += 1;
            if count > EACH_MAX_FIELDS {
                return Err(serde::de::Error::custom(format!(
                    "element {}: more than {EACH_MAX_FIELDS} fields (EACH_MAX_FIELDS)",
                    self.0
                )));
            }
            match map.next_value_seed(ScalarSeed { element: self.0 })? {
                Some(value) => {
                    fields.insert(key, value);
                }
                None => {
                    fields.remove(&key);
                }
            }
        }
        Ok(fields)
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        _: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        Err(nested(self.0))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
        Ok(scalar(Some(v.to_string())))
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Self::Value, E> {
        Ok(scalar(Some(v.to_string())))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
        Ok(scalar(Some(v.to_string())))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
        Ok(scalar(Some(v.to_string())))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
        Ok(scalar(Some(v.to_string())))
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(scalar(None))
    }
}

/// A scalar element's fields: `value`, when it is not `null`.
fn scalar(value: Option<String>) -> Fields {
    value
        .map(|v| Fields::from([("value".to_string(), v)]))
        .unwrap_or_default()
}

/// The refusal of a nested value, at its first bracket.
fn nested<E: serde::de::Error>(element: usize) -> E {
    E::custom(format!(
        "element {element}: a nested object or array — `each` fills a template from flat \
         elements; select the flat shape first (`$j`, or `urn:json:query`)"
    ))
}

/// One field's value: its text, or `None` for `null`; a nested value is refused.
struct ScalarSeed {
    element: usize,
}

impl<'de> serde::de::DeserializeSeed<'de> for ScalarSeed {
    type Value = Option<String>;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        de: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        de.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for ScalarSeed {
    type Value = Option<String>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "element {}: a scalar field value", self.element)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        _: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        Err(nested(self.element))
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        _: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        Err(nested(self.element))
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(None)
    }
}

/// Parse an argument that must be an IRI, reporting which argument if it isn't.
fn parse_iri(s: &str, arg: &str) -> Result<Iri> {
    Iri::parse(s).map_err(|e| Error::InvalidArgument {
        name: arg.to_string(),
        detail: format!("not an IRI: {e}"),
    })
}

/// A condition's bytes as UTF-8 text.
fn as_text<'b>(bytes: &'b [u8], iri: &Iri) -> Result<&'b str> {
    std::str::from_utf8(bytes)
        .map_err(|_| Error::Endpoint(format!("conditional: `{}` is not UTF-8 text", iri.as_str())))
}

/// Interpret a resource's bytes as a boolean — lenient on common spellings, strict
/// on anything else so a malformed condition can't silently mis-branch.
fn as_bool(bytes: &[u8], iri: &Iri) -> Result<bool> {
    let s = as_text(bytes, iri)?.trim().to_ascii_lowercase();
    match s.as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" | "" => Ok(false),
        other => Err(Error::Endpoint(format!(
            "conditional: `{}` returned {other:?}, not a boolean (true/false/1/0/yes/no/on/off)",
            iri.as_str()
        ))),
    }
}

/// How a marker splices what it resolves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// `$a{…}`: as it is, and then its own markers expanded.
    Transclude,
    /// `$r{…}`: as it is, terminated.
    Raw,
    /// `$h{…}`: as text, HTML-escaped, terminated.
    Html,
    /// `$j{<src> <program>}`: a jq selection of `src` ([`JQ_QUERY`]), spliced as `$h`
    /// splices: as text, HTML-escaped, terminated.
    Jq,
}

impl Mode {
    /// Every mode, in the order compose learned its letter. The ONE list: the scanner
    /// reads letters through it and [`MARKER_LETTERS`] is built from it, so a mode left
    /// out is a marker nothing recognizes (and its own tests fail), never a marker the
    /// parser reads and the escape misses.
    const ALL: [Mode; 4] = [Mode::Transclude, Mode::Raw, Mode::Html, Mode::Jq];

    /// The letter that spells this mode after `$`.
    const fn letter(self) -> u8 {
        match self {
            Mode::Transclude => b'a',
            Mode::Raw => b'r',
            Mode::Html => b'h',
            Mode::Jq => b'j',
        }
    }

    /// Whether what this mode splices is escaped as text — the only way element data
    /// ([`each`]) may reach the output.
    fn escapes(self) -> bool {
        matches!(self, Mode::Html | Mode::Jq)
    }

    /// The mode a marker letter spells, if it spells one.
    fn from_letter(letter: u8) -> Option<Mode> {
        Mode::ALL.into_iter().find(|mode| mode.letter() == letter)
    }
}

/// The letters [`Mode::ALL`] spells, as `char`s for [`MARKER_LETTERS`].
const LETTERS: [char; Mode::ALL.len()] = {
    let mut letters = ['\0'; Mode::ALL.len()];
    let mut i = 0;
    while i < letters.len() {
        letters[i] = Mode::ALL[i].letter() as char;
        i += 1;
    }
    letters
};

/// The letters that open a [`compose`] marker — `$a{…}`, `$r{…}`, `$h{…}`, `$j{…}` — in
/// the order compose learned them (`a` in 0.2, `r` and `h` in 0.3, `j` in 0.6).
///
/// Built from the list compose's own scanner reads, so it cannot drift from it. A host
/// that escapes text for a template should call [`escape_markers`] rather than keep a copy
/// of this list: a copy escapes only the letters it knew when it was made, and the next
/// letter compose learns passes through it live (ledger #646). It is a slice so a new
/// letter is not a change to its type.
///
/// ```
/// assert_eq!(ikigai_fn::MARKER_LETTERS, &['a', 'r', 'h', 'j']);
/// ```
pub const MARKER_LETTERS: &[char] = &LETTERS;

/// The marker `$` at byte `i` of `b` opens, if it opens one: a marker letter, then `{`.
/// Whether it is ever CLOSED is the scanner's business; [`escape_markers`] escapes an
/// unclosed one too, which costs nothing and stays literal.
fn opens_marker(b: &[u8], i: usize) -> Option<Mode> {
    let mode = b.get(i + 1).and_then(|letter| Mode::from_letter(*letter))?;
    (b.get(i + 2) == Some(&b'{')).then_some(mode)
}

/// `text` written so that [`compose`] renders it exactly as it is: no marker in it
/// resolves, and every `$` survives.
///
/// The one thing compose reads outside a marker is `$`: `$$` is a literal `$`, and `$`
/// before a marker letter and `{` opens a marker. So a run of `$` is doubled whenever
/// compose would otherwise read it differently — a run of two or more, or a single `$`
/// that opens a marker — and a lone `$` before anything else is left as it is, which keeps
/// ordinary text (`costs $5`) readable. Compose of the result is `text`, byte for byte.
///
/// This is an escape, not a normalization: it is not idempotent, and must not be, because
/// `$$h{…}` in the input is text that says `$$h{…}` (say, documentation of the escape
/// itself), and escaping it again must keep both `$`. Escape a value once, where it enters
/// a template.
///
/// ```
/// use ikigai_fn::escape_markers;
///
/// assert_eq!(escape_markers("costs $5"), "costs $5");
/// assert_eq!(
///     escape_markers("write $h{<iri>} for text and $$ for a dollar"),
///     "write $$h{<iri>} for text and $$$$ for a dollar"
/// );
/// // Already-escaped text is text too: both of its `$` survive a compose.
/// assert_eq!(escape_markers("$$a{<iri>}"), "$$$$a{<iri>}");
/// ```
pub fn escape_markers(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(offset) = text[i..].find('$') {
        let start = i + offset;
        let end = start + b[start..].iter().take_while(|c| **c == b'$').count();
        // `$` is ASCII, so every index here is a char boundary.
        out.push_str(&text[i..end]);
        if end - start > 1 || opens_marker(b, start).is_some() {
            out.push_str(&text[start..end]);
        }
        i = end;
    }
    out.push_str(&text[i..]);
    out
}

/// One piece of a scanned template: literal text, or a marker to resolve.
enum Segment {
    Lit(String),
    Marker(Mode, String),
}

/// One alternative of a marker body.
enum Alt {
    /// `{name}`: a template argument's value; resolves nothing.
    Arg(String),
    /// `<iri>[?k=v&…]`: a SOURCE request. The IRI and the unquoted values may name
    /// template arguments.
    Request {
        iri: String,
        args: Vec<(String, Value)>,
    },
    /// `$j{<src> <program>}`: a SOURCE of [`JQ_QUERY`] with `src` (whose text may name
    /// template arguments, as an IRI does) and `q`, the program, which is template text
    /// only: it names no argument and nothing fills it.
    Jq { src: String, program: String },
}

/// Where a marker's `{name}` arguments come from.
#[derive(Clone, Copy)]
enum Args<'e> {
    /// The request's: a variable the name captured, else an inline argument
    /// ([`compose`], [`compose_over`]).
    Request,
    /// One element of an [`each`] collection, and nothing else. Element data is DATA: it
    /// reaches the output only through an escaping marker (`$h`, `$j`), and is never
    /// reserved-expanded (`{+name}`) into an IRI.
    Element(&'e Fields),
}

/// One element's template arguments: its top-level fields (or `value`), and `index`.
type Fields = std::collections::BTreeMap<String, String>;

/// A marker argument's value: quoted (`"…"`, literal) or not (may name arguments).
struct Value {
    text: String,
    literal: bool,
}

/// A parsed marker: how it splices, its body as written, and its alternatives in order.
struct Marker {
    mode: Mode,
    body: String,
    alts: Vec<Alt>,
}

impl Marker {
    /// Whether any alternative reads a template argument: is one, or names one in its
    /// IRI or an unquoted value.
    fn reads_arguments(&self) -> bool {
        self.alts.iter().any(|alt| match alt {
            Alt::Arg(_) => true,
            Alt::Request { iri, args } => {
                iri.contains('{') || args.iter().any(|(_, v)| !v.literal && v.text.contains('{'))
            }
            Alt::Jq { src, .. } => src.contains('{'),
        })
    }
}

/// Split `text` into ordered literal/marker segments. `$$` collapses to a literal
/// `$` (so `$$a{…}` becomes the literal text `$a{…}`); `$a{ … }`, `$r{ … }` and
/// `$h{ … }` become markers holding their trimmed body; an unterminated marker stays
/// literal.
fn scan(text: &str) -> Vec<Segment> {
    let b = text.as_bytes();
    let mut segments = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            // `$$` is a literal `$` — the escape for every marker.
            if b.get(i + 1) == Some(&b'$') {
                lit.push('$');
                i += 2;
                continue;
            }
            if let Some(mode) = opens_marker(b, i) {
                let brace = i + 2;
                if let Some(close) = find_marker_end(text, brace) {
                    if !lit.is_empty() {
                        segments.push(Segment::Lit(std::mem::take(&mut lit)));
                    }
                    let body = text[brace + 1..close].trim().to_string();
                    segments.push(Segment::Marker(mode, body));
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

/// Expand every marker in `text`. The `a` is for *asynchronous*: the first alternative
/// of each of a level's markers is forked via [`Invocation::fan_out`] and joined, so a
/// scheduled kernel resolves them **concurrently** (each spawned onto the pool,
/// parking on the join) while a single-threaded kernel resolves them in turn. A `$a`
/// result is itself expanded — a transcluded template's own markers recurse, and the
/// sub-expansions run concurrently too. A fallback is tried only after its marker's
/// first alternative failed, in turn. Boxed for the async recursion.
fn expand<'a>(
    inv: &'a Invocation<'_>,
    args: Args<'a>,
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
        // This level's markers, in document order. One that cannot be parsed is the
        // template's fault, and fails the whole compose whatever fallbacks it names.
        let markers = segments
            .iter()
            .filter_map(|segment| match segment {
                Segment::Marker(mode, body) => Some(parse_marker(*mode, body)),
                Segment::Lit(_) => None,
            })
            .collect::<Result<Vec<Marker>>>()?;
        if let Args::Element(_) = args {
            // Element data reaches the output only escaped: a marker that reads an
            // argument and splices as it is (`$r`) is refused, at every depth.
            if let Some(marker) = markers
                .iter()
                .find(|marker| !marker.mode.escapes() && marker.reads_arguments())
            {
                return Err(Error::Endpoint(format!(
                    "each: marker `{}` reads element data and splices it as it is — element \
                     fields reach the output only through `$h` (or `$j`)",
                    marker.body
                )));
            }
        }
        // Fork: every first alternative that is a request resolves concurrently. An
        // argument is answered here, and a request that cannot be built (a missing
        // argument, a bad IRI) is that marker's failure — its fallback's to cover.
        let mut requests = Vec::new();
        let firsts: Vec<Option<Result<String>>> = markers
            .iter()
            .map(|marker| match &marker.alts[0] {
                Alt::Arg(name) => Some(template_arg(inv, args, name)),
                alt => match alt_request(inv, args, &marker.body, alt) {
                    Ok(request) => {
                        requests.push(request);
                        None
                    }
                    Err(e) => Some(Err(e)),
                },
            })
            .collect();
        let mut resolved = inv.fan_out(requests).await.into_iter();
        let outcomes: Vec<First> = firsts
            .into_iter()
            .map(|first| match first {
                Some(local) => First::Local(local),
                None => First::Resolved(resolved.next().expect("one result per request")),
            })
            .collect();
        let expansions = markers
            .iter()
            .zip(outcomes)
            .map(|(marker, first)| async move {
                let mut result = match first {
                    First::Local(value) => value.map(|value| splice_value(marker.mode, value)),
                    First::Resolved(Ok(repr)) => {
                        splice(inv, args, marker.mode, &marker.body, repr, depth).await
                    }
                    First::Resolved(Err(e)) => Err(e),
                };
                for alt in &marker.alts[1..] {
                    if result.is_ok() {
                        break;
                    }
                    result = evaluate(inv, args, marker, alt, depth).await;
                }
                result
            });
        let mut expanded = try_join_all(expansions).await?.into_iter();
        // Reassemble in document order.
        let mut out = String::with_capacity(text.len());
        for segment in &segments {
            match segment {
                Segment::Lit(t) => out.push_str(t),
                Segment::Marker(..) => {
                    out.push_str(&expanded.next().expect("one expansion per marker"))
                }
            }
        }
        Ok(out)
    })
}

/// A marker's first alternative, once the level's fork has joined.
enum First {
    /// Answered without the kernel: an argument's value, or the failure to build the
    /// request.
    Local(Result<String>),
    /// What the kernel resolved the request to.
    Resolved(Result<Representation>),
}

/// Resolve and splice one (fallback) alternative of `marker`.
async fn evaluate(
    inv: &Invocation<'_>,
    args: Args<'_>,
    marker: &Marker,
    alt: &Alt,
    depth: usize,
) -> Result<String> {
    match alt {
        Alt::Arg(name) => Ok(splice_value(marker.mode, template_arg(inv, args, name)?)),
        alt => {
            let request = alt_request(inv, args, &marker.body, alt)?;
            let repr = inv.issue(request).await?;
            splice(inv, args, marker.mode, &marker.body, repr, depth).await
        }
    }
}

/// What a marker splices for the resource it resolved.
async fn splice(
    inv: &Invocation<'_>,
    args: Args<'_>,
    mode: Mode,
    body: &str,
    repr: Representation,
    depth: usize,
) -> Result<String> {
    match (mode, String::from_utf8(repr.bytes)) {
        (Mode::Transclude, Ok(text)) => expand(inv, args, text, depth + 1).await,
        (Mode::Raw, Ok(text)) => Ok(text),
        (Mode::Html | Mode::Jq, Ok(text)) => Ok(html::html_text(&text)),
        // A value spliced into HTML must be text: a placeholder here would be escaped
        // into visible garbage, or land inside an attribute.
        (Mode::Html | Mode::Jq, Err(e)) => Err(Error::Endpoint(format!(
            "compose: `{body}` is non-text ({} bytes), and `${}` splices text",
            e.into_bytes().len(),
            mode.letter() as char
        ))),
        (_, Err(e)) => Ok(format!(
            "<!-- compose: `{body}` is non-text ({} bytes), not inlined -->",
            e.into_bytes().len()
        )),
    }
}

/// What a marker splices for an argument's value. (`$a` over an argument is refused
/// when the marker is parsed, so a value is never expanded.)
fn splice_value(mode: Mode, value: String) -> String {
    match mode {
        Mode::Html | Mode::Jq => html::html_text(&value),
        Mode::Raw | Mode::Transclude => value,
    }
}

/// The value of the template argument `name`: the variable the name captured, else the
/// request's inline argument — or, for an [`each`] element, the element's field;
/// [`Error::MissingArgument`] if it is not there.
fn template_arg(inv: &Invocation<'_>, args: Args<'_>, name: &str) -> Result<String> {
    if let Args::Element(fields) = args {
        return fields
            .get(name)
            .cloned()
            .ok_or_else(|| Error::MissingArgument(name.to_string()));
    }
    if let Some(value) = inv.bindings.get(name) {
        return Ok(value.to_string());
    }
    inv.inline_str(name).map(str::to_string)
}

/// The index of the `}` closing the marker whose `{` is at `brace`, skipping any
/// `}` inside a `"…"` span (where `\"` and `\\` are escapes) and any that closes a
/// `{name}` argument inside the body. `None` if unterminated.
fn find_marker_end(text: &str, brace: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = brace + 1;
    let mut in_quote = false;
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'\\' if in_quote => i += 2,
            b'"' => {
                in_quote = !in_quote;
                i += 1;
            }
            b'{' if !in_quote => {
                depth += 1;
                i += 1;
            }
            b'}' if !in_quote => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
                i += 1;
            }
            c => i += utf8_len(c),
        }
    }
    None
}

/// Split `s` on every `sep` outside a `"…"` span (where `\"` and `\\` are escapes).
fn split_outside_quotes<'s>(s: &'s str, sep: &str) -> Vec<&'s str> {
    let b = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut in_quote = false;
    while i < b.len() {
        match b[i] {
            b'\\' if in_quote => i += 2,
            b'"' => {
                in_quote = !in_quote;
                i += 1;
            }
            // `sep` is ASCII, so a match starts on a character boundary.
            _ if !in_quote && b[i..].starts_with(sep.as_bytes()) => {
                parts.push(&s[start..i]);
                i += sep.len();
                start = i;
            }
            _ => i += 1,
        }
    }
    parts.push(&s[start.min(s.len())..]);
    parts
}

/// Parse a marker body: `alternative [|| alternative]…`, each an argument `{name}` or a
/// request `<iri>[?k=v&…]`. Every `{…}` must name an argument; `$a` over an argument is
/// refused.
fn parse_marker(mode: Mode, body: &str) -> Result<Marker> {
    let refuse = |detail: String| Error::Endpoint(format!("compose: marker `{body}`: {detail}"));
    if mode == Mode::Jq {
        // `<src> <program>`: the source runs to the first whitespace, and the program is
        // everything after it, as written. No `||` split and no `{name}` in the program:
        // jq's own `{…}` is an object, and a program is template text only.
        let (src, program) = body
            .split_once(char::is_whitespace)
            .map(|(src, program)| (src, program.trim()))
            .unwrap_or((body, ""));
        if program.is_empty() {
            return Err(refuse(
                "`$j{<src> <program>}` needs a source IRI, whitespace, and a jq program"
                    .to_string(),
            ));
        }
        check_placeholders(src, true).map_err(refuse)?;
        return Ok(Marker {
            mode,
            body: body.to_string(),
            alts: vec![Alt::Jq {
                src: src.to_string(),
                program: program.to_string(),
            }],
        });
    }
    let mut alts = Vec::new();
    for alt in split_outside_quotes(body, "||") {
        let alt = alt.trim();
        if alt.is_empty() {
            return Err(refuse("an empty alternative".to_string()));
        }
        if let Some(name) = alt
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        {
            if is_arg_name(name) {
                if mode == Mode::Transclude {
                    return Err(refuse(format!(
                        "`{{{name}}}` is an argument, a value and never a template — \
                         splice it with `$h` or `$r`, not `$a`"
                    )));
                }
                alts.push(Alt::Arg(name.to_string()));
                continue;
            }
        }
        let (iri, query) = match alt.split_once('?') {
            Some((iri, query)) => (iri.trim(), Some(query)),
            None => (alt, None),
        };
        check_placeholders(iri, true).map_err(refuse)?;
        let mut args = Vec::new();
        if let Some(query) = query {
            for pair in split_outside_quotes(query, "&") {
                let pair = pair.trim();
                if pair.is_empty() {
                    continue;
                }
                let (key, value) = pair.split_once('=').ok_or_else(|| {
                    Error::Endpoint(format!(
                        "compose: marker argument `{pair}` is not key=value"
                    ))
                })?;
                let value = value.trim();
                let value = match unquote(value) {
                    Some(text) => Value {
                        text,
                        literal: true,
                    },
                    None => {
                        check_placeholders(value, false).map_err(refuse)?;
                        Value {
                            text: value.to_string(),
                            literal: false,
                        }
                    }
                };
                args.push((key.trim().to_string(), value));
            }
        }
        alts.push(Alt::Request {
            iri: iri.to_string(),
            args,
        });
    }
    Ok(Marker {
        mode,
        body: body.to_string(),
        alts,
    })
}

/// Build the SOURCE request an alternative names, its arguments filled in. (An argument
/// alternative names no request; it is answered before this is reached.)
fn alt_request(inv: &Invocation<'_>, from: Args<'_>, body: &str, alt: &Alt) -> Result<Request> {
    let bad_iri = |e: ikigai_core::IriError| {
        Error::Endpoint(format!("compose: bad IRI in marker `{body}`: {e}"))
    };
    match alt {
        Alt::Arg(name) => unreachable!("argument `{name}` is answered without a request"),
        Alt::Request { iri, args } => {
            let filled = fill_arguments(inv, from, iri, true)?;
            let mut request = Request::new(Verb::Source, Iri::parse(&filled).map_err(bad_iri)?);
            for (key, value) in args {
                let text = if value.literal {
                    value.text.clone()
                } else {
                    fill_arguments(inv, from, &value.text, false)?
                };
                request = request.with_arg(key.clone(), ArgRef::Inline(text.into_bytes()));
            }
            Ok(request)
        }
        Alt::Jq { src, program } => {
            // The source is named like any marker's IRI (its arguments percent-encoded),
            // and checked as one here, so a bad one is this marker's error rather than
            // the jq endpoint's. The program goes in as it was written.
            let src = fill_arguments(inv, from, src, true)?;
            Iri::parse(&src).map_err(bad_iri)?;
            Ok(
                Request::new(Verb::Source, Iri::parse(JQ_QUERY).expect("a valid IRI"))
                    .with_arg("src", ArgRef::Inline(src.into_bytes()))
                    .with_arg("q", ArgRef::Inline(program.clone().into_bytes()))
                    .with_arg("as", ArgRef::Inline(b"text/plain".to_vec())),
            )
        }
    }
}

/// Every `{name}` in `template` replaced by that argument's value — percent-encoded
/// when `encode` (in an IRI), verbatim otherwise (as one argument value) — and, in an IRI,
/// every `{+name}` by its value under RFC 6570's reserved expansion. The placeholders were
/// checked when the marker was parsed.
fn fill_arguments(
    inv: &Invocation<'_>,
    args: Args<'_>,
    template: &str,
    encode: bool,
) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .expect("placeholders are checked when parsed");
        let inner = &after[..close];
        match inner.strip_prefix('+') {
            Some(name) => {
                debug_assert!(encode, "`{{+name}}` is refused outside an IRI when parsed");
                if let Args::Element(_) = args {
                    return Err(Error::Endpoint(format!(
                        "each: `{{+{name}}}` would let element data choose a path, a query or \
                         a name under the prefix — element fields are reserved-expanded never; \
                         write `{{{name}}}`"
                    )));
                }
                html::push_url_reserved(&template_arg(inv, args, name)?, &mut out);
            }
            None if encode => html::push_url_segment(&template_arg(inv, args, inner)?, &mut out),
            None => out.push_str(&template_arg(inv, args, inner)?),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Refuse a `{` that does not open a `{name}` argument — or, in an IRI (`in_iri`), a
/// `{+name}` one, whose reserved expansion lets `:` through, so it must come after the
/// scheme: the template's own text before its FIRST placeholder holds a `:` (an argument
/// before that would choose the scheme). Outside an IRI `{+name}` is
/// refused, since an unquoted value already takes `{name}` verbatim.
fn check_placeholders(template: &str, in_iri: bool) -> std::result::Result<(), String> {
    let scheme_fixed = template[..template.find('{').unwrap_or(template.len())].contains(':');
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .ok_or_else(|| "a `{` is never closed".to_string())?;
        let inner = &after[..close];
        match inner.strip_prefix('+') {
            Some(name) if is_arg_name(name) => {
                if !in_iri {
                    return Err(format!(
                        "`{{{inner}}}` (reserved expansion) belongs only in the IRI; in an \
                         unquoted value `{{{name}}}` is already the value, verbatim"
                    ));
                }
                if !scheme_fixed {
                    return Err(format!(
                        "`{{{inner}}}` would let an argument choose the IRI's scheme — the \
                         template's text before its first argument must hold the scheme and \
                         its `:`"
                    ));
                }
            }
            _ if is_arg_name(inner) => {}
            _ => return Err(format!("`{{{inner}}}` is not an argument name")),
        }
        rest = &after[close + 1..];
    }
    Ok(())
}

/// An argument name: a letter or `_`, then letters, digits, `_` and `-`.
fn is_arg_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The text of a double-quoted marker argument value, `\"` and `\\` unescaped, or
/// `None` if the value is not quoted.
fn unquote(value: &str) -> Option<String> {
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
        Some(out)
    } else {
        None
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

/// The name [`space`] claims: `urn:iki:space:fn`.
pub const SPACE_ID: &str = "urn:iki:space:fn";

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
///
/// The space is configuration-free (no parameters, nothing read while building
/// it), so it names itself [`SPACE_ID`]. A host that extends it with `.bind`
/// gets an anonymous space back (core 0.1.89 drops the name on a new door), and
/// names its own composition if it wants one.
///
/// ```
/// use ikigai_core::{Exact, Space};
///
/// assert_eq!(
///     ikigai_fn::space().id().map(|iri| iri.as_str().to_owned()).as_deref(),
///     Some(ikigai_fn::SPACE_ID)
/// );
/// let extended = ikigai_fn::space().bind(Exact::new("urn:host:page"), ikigai_fn::echo());
/// assert!(extended.id().is_none(), "different doors, no inherited name");
/// ```
pub fn space() -> EndpointSpace {
    let echo_template = UriTemplate::parse("urn:demo:echo/{message}").expect("valid template");
    EndpointSpace::new()
        .bind(Exact::new("urn:iki:fn:toUpper"), to_upper())
        .bind(Exact::new("urn:iki:fn:reverseList"), reverse_list())
        .bind(Exact::new("urn:iki:fn:compose"), compose())
        .bind(Exact::new("urn:iki:fn:conditional"), conditional())
        .bind(Exact::new("urn:iki:fn:each"), each())
        .bind(Exact::new("urn:demo:wrap"), wrap())
        .bind(Exact::new("urn:demo:split"), split())
        .bind(Exact::new("urn:demo:greet"), greet())
        .bind(echo_template, echo())
        .named(space_iri("fn"))
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

    // ---- conditional -------------------------------------------------------

    /// A resource that returns a fixed body — a stand-in boolean or branch value.
    fn constant(name: &'static str, body: &'static str) -> FnEndpoint {
        FnEndpoint::new(name, move |_inv: &Invocation<'_>| {
            Ok(Representation::new(
                text_plain_utf8(),
                body.as_bytes().to_vec(),
            ))
        })
    }

    fn cond_kernel() -> Kernel {
        let space = space()
            .bind(Exact::new("urn:test:true"), constant("t", "true"))
            .bind(Exact::new("urn:test:false"), constant("f", "false"))
            .bind(Exact::new("urn:test:maybe"), constant("m", "maybe"))
            .bind(Exact::new("urn:test:A"), constant("a", "branch-A"))
            .bind(Exact::new("urn:test:B"), constant("b", "branch-B"));
        Kernel::new(Arc::new(space))
    }

    fn cond(
        kernel: &Kernel,
        if_u: &str,
        then_u: &str,
        else_u: Option<&str>,
    ) -> Result<Representation> {
        let mut req = Request::new(Verb::Source, Iri::parse("urn:iki:fn:conditional").unwrap())
            .with_arg("if", ArgRef::Inline(if_u.as_bytes().to_vec()))
            .with_arg("then", ArgRef::Inline(then_u.as_bytes().to_vec()));
        if let Some(e) = else_u {
            req = req.with_arg("else", ArgRef::Inline(e.as_bytes().to_vec()));
        }
        block_on(kernel.issue(req, &Capability::root()))
    }

    #[test]
    fn true_takes_the_then_branch() {
        let out = cond(
            &cond_kernel(),
            "urn:test:true",
            "urn:test:A",
            Some("urn:test:B"),
        )
        .unwrap();
        assert_eq!(out.bytes, b"branch-A");
    }

    #[test]
    fn false_takes_the_else_branch() {
        let out = cond(
            &cond_kernel(),
            "urn:test:false",
            "urn:test:A",
            Some("urn:test:B"),
        )
        .unwrap();
        assert_eq!(out.bytes, b"branch-B");
    }

    #[test]
    fn false_without_else_is_empty() {
        let out = cond(&cond_kernel(), "urn:test:false", "urn:test:A", None).unwrap();
        assert!(out.bytes.is_empty());
    }

    #[test]
    fn the_untaken_branch_is_never_sourced() {
        // `else` names a nonexistent resource; a true condition must not touch it
        // (if it did, the kernel would fail to resolve it).
        let out = cond(
            &cond_kernel(),
            "urn:test:true",
            "urn:test:A",
            Some("urn:does:not:exist"),
        )
        .unwrap();
        assert_eq!(out.bytes, b"branch-A");
    }

    #[test]
    fn a_non_boolean_condition_errors() {
        assert!(cond(&cond_kernel(), "urn:test:maybe", "urn:test:A", None).is_err());
    }

    /// The spellings the summary and the refusal list are exactly the ones `as_bool`
    /// accepts: the list once omitted `on`/`off`, and a Deno port copied it (ledger #627).
    #[test]
    fn the_stated_boolean_spellings_are_the_accepted_ones() {
        const STATED: &str = "(true/false/1/0/yes/no/on/off)";
        let iri = Iri::parse("urn:test:x").unwrap();
        let refusal = as_bool(b"maybe", &iri).unwrap_err().to_string();
        assert!(refusal.contains(STATED), "refusal: {refusal}");
        assert!(
            conditional().describe().summary.contains(STATED),
            "summary lists the accepted spellings"
        );
        let words = STATED.trim_matches(|c| c == '(' || c == ')').split('/');
        for (i, word) in words.enumerate() {
            assert_eq!(
                as_bool(word.as_bytes(), &iri).unwrap(),
                i % 2 == 0,
                "{word}"
            );
        }
    }

    #[test]
    fn to_upper_upper_cases_the_in_argument() {
        assert_eq!(
            source(&kernel(), "urn:iki:fn:toUpper", &[("in", b"hi there")]).bytes,
            b"HI THERE"
        );
    }

    #[test]
    fn reverse_list_reverses_newline_items() {
        assert_eq!(
            source(&kernel(), "urn:iki:fn:reverseList", &[("in", b"a\nb\nc")]).bytes,
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

    // ---- self-description ---------------------------------------------------

    /// Every input declared in `space()` carries a class. `select_action` and
    /// `urn:kernel:actions types=` match on `ArgSpec::class` alone, so an input without one
    /// is not "untyped" — it makes its endpoint invisible to type-driven selection (and a
    /// REQUIRED input without one makes the endpoint un-inferable). Walks the live space
    /// through the kernel rather than a hand-kept list, so a new binding is covered the
    /// moment it is bound, and covers per-verb `ActionSpec` inputs for the day one appears.
    /// Enumerates THIS space's bindings, not the kernel's: `Kernel::entries` also lists the
    /// kernel's own `urn:kernel:*` builtins, which are core's to type, not this crate's.
    #[test]
    fn every_declared_input_has_a_class() {
        use ikigai_core::Space;
        let space = space();
        let entries = space.entries().expect("space() is enumerable");
        let kernel = Kernel::new(Arc::new(space));
        assert!(!entries.is_empty());
        let mut untyped = Vec::new();
        for entry in &entries {
            let description = kernel
                .describe_pattern(&entry.pattern)
                .unwrap_or_else(|| panic!("{} describes itself", entry.pattern));
            let flat = description.inputs.iter();
            let per_action = description.actions.iter().flat_map(|a| a.inputs.iter());
            for input in flat.chain(per_action) {
                if input.class.is_none() {
                    untyped.push(format!("{}#{}", entry.pattern, input.name));
                }
            }
        }
        assert!(untyped.is_empty(), "inputs without a class: {untyped:?}");
    }

    /// The point of declaring the classes: type-driven selection now OFFERS these endpoints.
    /// "I hold a string — what can I do with it?" answers with the text functions; "I hold
    /// an IRI" answers with the two that take one. Before, every answer was empty.
    #[test]
    fn typed_selection_offers_the_text_and_iri_functions() {
        use ikigai_core::select_action;
        let space = space();
        let names = |present: &[&str]| -> Vec<String> {
            select_action(&space, present)
                .into_iter()
                .map(|m| m.endpoint)
                .collect()
        };
        // `ActionMatch::endpoint` is the bound IRI — the thing a caller would invoke.
        let strings = names(&[XSD_STRING]);
        for expected in [
            "urn:iki:fn:toUpper",
            "urn:iki:fn:reverseList",
            "urn:demo:split",
            "urn:demo:wrap",
            "urn:demo:greet",
            "urn:demo:echo/{message}",
        ] {
            assert!(
                strings.contains(&expected.to_string()),
                "{expected} in {strings:?}"
            );
        }
        let iris = names(&[XSD_ANY_URI]);
        for expected in ["urn:iki:fn:compose", "urn:iki:fn:conditional"] {
            assert!(
                iris.contains(&expected.to_string()),
                "{expected} in {iris:?}"
            );
        }
        assert!(
            !iris.contains(&"urn:iki:fn:toUpper".to_string()),
            "{iris:?}"
        );
    }
}

#[cfg(test)]
mod compose_tests {
    use super::*;
    use futures::executor::block_on;
    use ikigai_core::{Capability, Expiry, Kernel};
    use std::sync::Arc;

    /// A shape resource: returns a fixed body (which may carry markers) under the
    /// given media type. The type is a parameter because `compose` passes it
    /// through — a shape fixed at `text/html` cannot tell "mirrors the source" apart
    /// from "always emits HTML".
    fn shape(media_type: &'static str, body: &'static str) -> FnEndpoint {
        FnEndpoint::new("shape", move |_inv: &Invocation<'_>| {
            Ok(
                Representation::new(ReprType::new(media_type), body.as_bytes().to_vec())
                    .cacheable(),
            )
        })
    }

    /// A kernel binding `compose`, `toUpper`, and a `urn:data:page` shape.
    fn kernel(page: &'static str) -> Kernel {
        typed_kernel("text/html", page)
    }

    /// The same, with the shape's media type chosen by the caller.
    fn typed_kernel(media_type: &'static str, page: &'static str) -> Kernel {
        let space = EndpointSpace::new()
            .bind(Exact::new("urn:iki:fn:compose"), compose())
            .bind(Exact::new("urn:iki:fn:toUpper"), to_upper())
            .bind(Exact::new("urn:data:page"), shape(media_type, page));
        Kernel::new(Arc::new(space))
    }

    fn compose_page(kernel: &Kernel) -> Representation {
        block_on(
            kernel.issue(
                Request::new(Verb::Source, Iri::parse("urn:iki:fn:compose").unwrap())
                    .with_arg("src", ArgRef::Inline(b"urn:data:page".to_vec())),
                &Capability::root(),
            ),
        )
        .unwrap()
    }

    #[test]
    fn expands_a_marker_with_a_quoted_argument() {
        let rep = compose_page(&kernel(r#"<h1>$a{urn:iki:fn:toUpper?in="hi there"}</h1>"#));
        assert_eq!(rep.bytes, b"<h1>HI THERE</h1>".to_vec());
    }

    /// Whatever the shape is labelled, the composite carries that label: the type is
    /// the source's, which is why `describe` declares none. Before 0.2.2 this test
    /// ran over a shape hard-coded to `text/html` and so could not have failed if
    /// compose had emitted a constant `text/html` — the very thing it claims to rule
    /// out, and the thing `describe` then asserted.
    #[test]
    fn preserves_the_source_media_type() {
        for media_type in ["text/html", "text/turtle", "application/json"] {
            let kernel = typed_kernel(media_type, "<p>$a{urn:iki:fn:toUpper?in=x}</p>");
            let rep = compose_page(&kernel);
            assert_eq!(rep.repr_type.media_type, media_type);
            assert_eq!(rep.bytes, b"<p>X</p>".to_vec(), "{media_type}");
        }
    }

    #[test]
    fn a_double_dollar_keeps_a_marker_literal() {
        let rep = compose_page(&kernel("show $$a{urn:iki:fn:toUpper?in=x} verbatim"));
        assert_eq!(
            rep.bytes,
            b"show $a{urn:iki:fn:toUpper?in=x} verbatim".to_vec()
        );
    }

    #[test]
    fn recurses_into_transcluded_shapes() {
        let space = EndpointSpace::new()
            .bind(Exact::new("urn:iki:fn:compose"), compose())
            .bind(Exact::new("urn:iki:fn:toUpper"), to_upper())
            .bind(
                Exact::new("urn:data:page"),
                shape("text/html", "[$a{urn:data:inner}]"),
            )
            .bind(
                Exact::new("urn:data:inner"),
                shape("text/html", "$a{urn:iki:fn:toUpper?in=hi}"),
            );
        let kernel = Kernel::new(Arc::new(space));
        let rep = compose_page(&kernel);
        assert_eq!(rep.bytes, b"[HI]".to_vec());
    }

    #[test]
    fn a_composite_of_cacheable_parts_is_cacheable() {
        let rep = compose_page(&kernel("<p>$a{urn:iki:fn:toUpper?in=hi}</p>"));
        assert_eq!(rep.expiry, Expiry::Never);
    }

    #[test]
    fn text_around_and_between_markers_is_preserved() {
        let rep = compose_page(&kernel(
            "a $a{urn:iki:fn:toUpper?in=b} c $a{urn:iki:fn:toUpper?in=d} e",
        ));
        assert_eq!(rep.bytes, b"a B c D e".to_vec());
    }
}
