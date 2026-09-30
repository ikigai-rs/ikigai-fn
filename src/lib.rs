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
/// (UTF-8 text) representation with the resource the marker names. There are three
/// markers, one for each way of splicing what comes back:
///
/// | marker | splices | the spliced text's own markers |
/// |---|---|---|
/// | `$a{<iri>}` | as it is — trusted markup | **expanded**: a template including a template |
/// | `$r{<iri>}` | as it is — trusted markup | left alone (terminated) |
/// | `$h{<iri>}` | as TEXT, HTML-escaped | left alone (terminated) |
///
/// A marker may carry inline arguments (`$a{urn:iki:fn:toUpper?in="resource oriented
/// computing"}`); a literal `$` is written `$$`, so `$$h{…}` is the literal text `$h{…}`.
///
/// **Escaping.** `$h` replaces `&`, `<`, `>`, `"` and `'` with `&amp;`, `&lt;`, `&gt;`,
/// `&quot;` and `&#39;`, so its output is safe in element text and inside an attribute
/// value quoted with either quote. It is not safe in an UNQUOTED attribute value (a space
/// ends one), and no escaping makes a URL attribute safe from a `javascript:` value: quote
/// every attribute, and put values, not whole URLs, in `href`.
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
/// - in the IRI its value is percent-encoded, as RFC 6570 expands a simple variable, so
///   an argument can never add a path segment, a query or an argument:
///   `$h{urn:example:cell:{x}:{y}}`;
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
                 and `$h{<iri>}` splices it as HTML-escaped text. `{name}` in a marker is \
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
        // Nor does it declare the template arguments: they are an OPEN set, whatever the
        // template names, and `ArgSpec` has no spelling for "any other argument". So a
        // pre-flight (`urn:kernel:validate`) reports each one as unknown, though the
        // endpoint uses it. Reported to the hub rather than worked around.
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
    let expanded = expand(inv, text, 0).await?;
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
                "Sources `if` and reads it as a boolean (true/false/1/0/yes/no) — or, with \
                 `equals`, tests whether its trimmed text is exactly that value — then \
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
            "conditional: `{}` returned {other:?}, not a boolean (true/false/1/0/yes/no)",
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
}

impl Mode {
    /// The mode a marker letter spells, if it spells one.
    fn from_letter(letter: u8) -> Option<Mode> {
        match letter {
            b'a' => Some(Mode::Transclude),
            b'r' => Some(Mode::Raw),
            b'h' => Some(Mode::Html),
            _ => None,
        }
    }
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
}

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
            let mode = b.get(i + 1).and_then(|letter| Mode::from_letter(*letter));
            if let (Some(mode), Some(b'{')) = (mode, b.get(i + 2)) {
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
        // Fork: every first alternative that is a request resolves concurrently. An
        // argument is answered here, and a request that cannot be built (a missing
        // argument, a bad IRI) is that marker's failure — its fallback's to cover.
        let mut requests = Vec::new();
        let firsts: Vec<Option<Result<String>>> = markers
            .iter()
            .map(|marker| match &marker.alts[0] {
                Alt::Arg(name) => Some(template_arg(inv, name)),
                Alt::Request { iri, args } => match build_request(inv, &marker.body, iri, args) {
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
                        splice(inv, marker.mode, &marker.body, repr, depth).await
                    }
                    First::Resolved(Err(e)) => Err(e),
                };
                for alt in &marker.alts[1..] {
                    if result.is_ok() {
                        break;
                    }
                    result = evaluate(inv, marker, alt, depth).await;
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
    marker: &Marker,
    alt: &Alt,
    depth: usize,
) -> Result<String> {
    match alt {
        Alt::Arg(name) => Ok(splice_value(marker.mode, template_arg(inv, name)?)),
        Alt::Request { iri, args } => {
            let request = build_request(inv, &marker.body, iri, args)?;
            let repr = inv.issue(request).await?;
            splice(inv, marker.mode, &marker.body, repr, depth).await
        }
    }
}

/// What a marker splices for the resource it resolved.
async fn splice(
    inv: &Invocation<'_>,
    mode: Mode,
    body: &str,
    repr: Representation,
    depth: usize,
) -> Result<String> {
    match (mode, String::from_utf8(repr.bytes)) {
        (Mode::Transclude, Ok(text)) => expand(inv, text, depth + 1).await,
        (Mode::Raw, Ok(text)) => Ok(text),
        (Mode::Html, Ok(text)) => Ok(escape_html(&text)),
        // A value spliced into HTML must be text: a placeholder here would be escaped
        // into visible garbage, or land inside an attribute.
        (Mode::Html, Err(e)) => Err(Error::Endpoint(format!(
            "compose: `{body}` is non-text ({} bytes), and `$h` splices text",
            e.into_bytes().len()
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
        Mode::Html => escape_html(&value),
        Mode::Raw | Mode::Transclude => value,
    }
}

/// `text`, safe in HTML element text and in an attribute value quoted with either
/// quote: `&`, `<`, `>`, `"` and `'` become `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The value of the template argument `name`: the variable the name captured, else the
/// request's inline argument; [`Error::MissingArgument`] if neither is there.
fn template_arg(inv: &Invocation<'_>, name: &str) -> Result<String> {
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
        check_placeholders(iri).map_err(refuse)?;
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
                        check_placeholders(value).map_err(refuse)?;
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

/// Build the SOURCE request an alternative names, its arguments filled in.
fn build_request(
    inv: &Invocation<'_>,
    body: &str,
    iri: &str,
    args: &[(String, Value)],
) -> Result<Request> {
    let filled = fill_arguments(inv, iri, true)?;
    let iri = Iri::parse(&filled)
        .map_err(|e| Error::Endpoint(format!("compose: bad IRI in marker `{body}`: {e}")))?;
    let mut request = Request::new(Verb::Source, iri);
    for (key, value) in args {
        let text = if value.literal {
            value.text.clone()
        } else {
            fill_arguments(inv, &value.text, false)?
        };
        request = request.with_arg(key.clone(), ArgRef::Inline(text.into_bytes()));
    }
    Ok(request)
}

/// Every `{name}` in `template` replaced by that argument's value — percent-encoded
/// when `encode` (in an IRI), verbatim otherwise (as one argument value). The
/// placeholders were checked when the marker was parsed.
fn fill_arguments(inv: &Invocation<'_>, template: &str, encode: bool) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .expect("placeholders are checked when parsed");
        let value = template_arg(inv, &after[..close])?;
        if encode {
            percent_encode(&value, &mut out);
        } else {
            out.push_str(&value);
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Refuse a `{` that does not open a `{name}` argument.
fn check_placeholders(template: &str) -> std::result::Result<(), String> {
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = after
            .find('}')
            .ok_or_else(|| "a `{` is never closed".to_string())?;
        let name = &after[..close];
        if !is_arg_name(name) {
            return Err(format!("`{{{name}}}` is not an argument name"));
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

/// Append `value` percent-encoded as RFC 6570 expands a simple variable: every byte
/// outside the unreserved set (`A-Z a-z 0-9 - . _ ~`) as `%XX`.
fn percent_encode(value: &str, out: &mut String) {
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
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
        .bind(Exact::new("urn:iki:fn:toUpper"), to_upper())
        .bind(Exact::new("urn:iki:fn:reverseList"), reverse_list())
        .bind(Exact::new("urn:iki:fn:compose"), compose())
        .bind(Exact::new("urn:iki:fn:conditional"), conditional())
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
