# ikigai-fn

A library of reusable [ikigai](https://github.com/ikigai-rs) **function
endpoints** — and the first ikigai *module crate*.

Rather than the kernel shipping behaviour itself, a host links in module crates
like this one and mounts their resources. `ikigai-fn` depends only on the
published `ikigai-core` kernel, uses no OS or platform APIs, and compiles to
`wasm32-unknown-unknown` — so the same module links into a native CLI and into
an in-browser WebAssembly host alike (no WASI required).

## Endpoints

| constructor | conventional IRI | what it does |
|-------------|------------------|--------------|
| `to_upper()` | `urn:iki:fn:toUpper` | upper-cases the `in` argument |
| `reverse_list()` | `urn:iki:fn:reverseList` | reverses newline-separated items in `in` |
| `compose()` | `urn:iki:fn:compose` | fills a template: `$a{…}`, `$r{…}`, `$h{…}` and `$j{…}` markers, see below |
| `compose_over(src)` | none — bind it at a view's name | `compose` with the template fixed; the name's captured variables are its arguments; `.named(…)` gives each view its own endpoint name |
| `conditional()` | `urn:iki:fn:conditional` | lazy `if`/`then`/`else`; only the taken branch runs; `equals=` branches on a value |
| `each()` | `urn:iki:fn:each` | fills `template` once per element of the JSON array `src`; optional `empty` and `separator` |
| `html_to_text()` | `urn:iki:fn:html-to-text` | the words the HTML in `in` shows a reader, as one line of text (`html::to_text`) |
| `split()` | `urn:demo:split` | splits `in` on commas into a newline list |
| `wrap()` | `urn:demo:wrap` | surrounds the `text` argument with `[ ]` |
| `greet()` | `urn:demo:greet` | combines `greeting` and `name` |
| `echo()` | `urn:demo:echo/{message}` | returns the grammar-captured `message` |

Each `snake_case` constructor builds an endpoint whose `lowerCamelCase`
identifier matches its name.

Every input declares its datatype (`xsd:string` for text, `xsd:anyURI` for the
IRI-valued `src`/`template`/`empty`/`if`/`then`/`else`), so type-driven selection — `select_action`,
`urn:kernel:actions types=` — offers these endpoints for the values you hold. A
test pins that no input in `space()` is left unclassed.

## Templates: views as pure composition

`compose` is a request-template language (NetKernel's TRL, resolved through the
kernel). A template is any text resource; a marker in it names a resource, and
`compose` resolves it, splices it in, and hangs the result from every golden
thread it read — so the filled view is cached and a write to anything it read
recomputes it.

| marker | splices | the spliced text's own markers |
|---|---|---|
| `$a{<iri>}` | as it is — trusted markup | **expanded**: a template including a template |
| `$r{<iri>}` | as it is — trusted markup | left alone (terminated) |
| `$h{<iri>}` | as text, HTML-escaped (`'` → `&#39;`) | left alone (terminated) |
| `$j{<src> <program>}` | a jq selection of `src`, as `$h` does | left alone (terminated) |

- **`$j`, a jq selection.** `$j{urn:example:people .[0].name}` is sugar for
  `$h{urn:json:query?src=urn:example:people&q=.[0].name&as=text/plain}`: the
  source runs to the first whitespace and the program is the rest, as written, to
  the marker's balanced `}` — so `{a: .b}`, `"}"`, `|` and spaces need no quoting.
  The source may name template arguments; the program never does (`{…}` there is
  jq's), so no request argument can change it. It always names the full
  `urn:json:query` (ikigai-json's, which this crate does not depend on), so it works
  with or without a prefix table, and with nothing bound there it fails as an
  unresolved `$h` does.

- **Terminate.** Only `$a` expands what it splices, so content from an atom —
  user data, a peer, a file — goes in through `$h` (or `$r`) and a marker in it
  is text. `$a` re-expands everything it splices, including a nested compose's
  output: splice a composed view with `$r`.
- **Escaping.** `$h` output is safe in element text and in an attribute quoted
  with either quote. Quote every attribute.
- **Literal text.** `$$` is a literal `$`. To put arbitrary text into a template
  so that it renders exactly as written, call `escape_markers(text)`;
  `MARKER_LETTERS` lists the letters that open a marker. Both are built from the
  list compose's own scanner reads, so a host that uses them instead of a copy
  cannot fall behind when compose learns a letter.
- **Reference arguments.** An unquoted `<iri>` value, such as
  `$r{urn:iki:fn:toUpper?in=<urn:example:data>}`, is issued as a reference;
  the kernel settles it by the target contract. `{name}` inside `<…>` is
  percent-encoded. Quote an angle value to keep it inline: `in="<i>text</i>"`.
- **Template arguments.** Every argument of the request but `src` is a template
  argument, and a marker names one as `{name}`: percent-encoded in an IRI
  (`$h{urn:example:cell:{x}:{y}}`), verbatim as an unquoted value, literal
  inside a quoted one. `$h{{x}}` splices the value itself. In an IRI,
  `{+name}` is RFC 6570's reserved expansion: `:`, `/`, `?`, `#` and the other
  reserved characters pass through, so `$r{urn:example:fragment:{+page}}` with
  `page=item:default:5` reaches `urn:example:fragment:item:default:5`. Use it
  only for a value the host fixed, never a request argument, which could then
  name any resource under the prefix; the template's text before its first
  argument must hold the scheme, so `$r{{+x}}` is refused.
- **Per-marker errors.** A failed marker fails the whole compose with its own
  typed error, unless it names fallbacks: `$h{urn:example:title ||
  urn:example:untitled}`. A fallback for an absent resource is cached and cut
  when the resource appears; one for a denial makes the view uncacheable.
- **Authority.** A template, and every marker in it, resolves under the
  capability of whoever fills it — the caller's, never more, so nothing the
  caller cannot source directly is reachable through compose. That is what lets
  a view read the viewer's own data; it also means a template acts with the
  viewer's authority, not its author's. Before filling a template you do not
  trust (a `src` from outside, a template a peer or a user could write), narrow
  the capability first — `Capability::attenuate`, or
  `Invocation::issue_attenuated` inside an endpoint — and the narrowing reaches
  every marker. `tests/authority.rs` pins both halves.
- **Views at names.** `compose_over(src)` bound under a URI template makes a
  parameterized view a resource: `urn:example:view:square:{x}:{y}`. That is what
  `conditional` can branch to, and what the cache keys on.
- **One name per view.** Every `compose_over` instance is called `composeOver`
  until it is named, and a name is how the rest of the ecosystem tells endpoints
  apart: a declared space (core 0.1.83, `ikigai_core::build`) binds each door to
  a registered endpoint BY NAME, and the `Registry` refuses two different
  endpoints under one name. So an application with several views names each one:

  ```rust,ignore
  let board = ikigai_fn::compose_over(template_iri).named("ttt-view-board");
  ```

  The name is also the view's description id, so the catalog
  (`urn:ikigai:endpoint:ttt-view-board`), `urn:kernel:validate` and the MCP
  projection each see one entry per view rather than one `composeOver` fronting
  them all; the title, "Compose over a template", still says what kind of
  endpoint it is. Unnamed, a view describes itself exactly as it did in 0.3.0.
  `tests/declared_views.rs` registers five named views, declares the space they
  are bound in, builds it with core's builder, and reads each door back.

`tests/tic_tac_toe_board.rs` is the proof: the tutorial's tic-tac-toe board,
which Rust code fills from the cells and the winner, as six templates and names
over the same cell space — byte for byte the Rust version's output in every
state, recomputed by a write to one cell.

`$a{…}` behaves exactly as it did in 0.2.x, and its tests are unchanged.

## Collections: `each`

`urn:iki:fn:each` renders a collection: it reads the JSON array `src` answers and
fills `template` once per element, in order.

```html
<!-- urn:example:ui:row -->
<tr><td>$h{{index}}</td><td>$h{{name}}</td></tr>
```

```text
$r{urn:iki:fn:each?src=urn:example:people&template=urn:example:ui:row&empty=urn:example:ui:no-rows}
```

- An object element's top-level fields are the template's `{arguments}`; a scalar
  element is `{value}`; `{index}` is the position, from 0. A `null` is absent, so
  a fallback covers it (`$h{{title} || {name}}`).
- A nested value is refused, naming the element: select the flat shape first with
  jq. The array is read by a typed deserializer that refuses a nested value at its
  first bracket, so a deeply nested document cannot recurse.
- **Element data is data.** It reaches the output only through `$h` or `$j`; a
  template that splices it as it is (`$r{{name}}`, or `$r` over a request that
  names a field) is refused at every depth, and `{+name}` is refused, so a field
  is never more than one percent-encoded segment of a name the template fixed.
  The request's own arguments are not template arguments here, only the
  element's. `separator` is HTML-escaped.
- Bounds: 10,000 elements, 256 fields an element, 8 MiB of output.
- Cacheable exactly when `src`, the templates and every marker are, and
  golden-threaded from each.

## Short names: a prefix table in a dynamic space

A composition can write `json:query` for `urn:json:query`, `fn:each` for
`urn:iki:fn:each` and `ui:row` for `urn:example:ui:row`. A prefix is not text
substitution: it is a real name that exists only inside a dynamic space around the
composition. The table is DATA, an `ik:Alias` of `ik:RewriteRule`s with
`ik:resolveAt "root"` (core 0.1.110) over an empty space, served by the host at an
IRI of its choosing:

```turtle
<urn:example:prefixes:rule:1> a ik:RewriteRule ;
    ik:ruleKind "prefix" ; ik:logical "json:" ; ik:canonical "urn:json:" ;
    ik:resolveAt "root" .
```

The host builds the space once and injects it where it composes:

```rust,ignore
let prefixes = Arc::new(ikigai_fn::prefix_space("pages", table_iri));
spawn(prefixes.clone().drive(&kernel, &cap));          // core's `declare` feature
kernel.issue_in(request, &cap, Scope::empty().with(prefixes.clone()))
```

Inside, `$j{page:people .[0].name}` and
`$r{fn:each?src=page:rows&template=ui:row}` resolve, and so does the `page:people`
the jq endpoint is asked to read; outside, the names do not exist. A write to the
table recomputes what was composed with it. `prefix_check` (which `prefix_space`
runs on every candidate) refuses a table that is anything but rewrite rules, a
rule that does not resolve from the root, and a prefix under a scheme the host
names things with (`urn`, `http`, `https`, `file`, `data` and the rest of
`RESERVED_SCHEMES`). A prefix grants nothing: the rewritten name is issued under the
caller's own capability. `tests/prefixes.rs` holds the whole example.

## Escaping and links: `ikigai_fn::html`

One rule for every HTML door that builds markup from data (ledger #1813), so a page
built in Rust, a `compose` template and a port in another language agree:

| Where the value goes | Function |
|---|---|
| element text | `html_text` (`&<>"'` escaped; a C0 control other than tab, LF, CR becomes U+FFFD) |
| a quoted attribute, either quote | `attr` (the same output) |
| one path segment or query value | `url_segment` (RFC 3986 unreserved kept, every other byte `%XX`) |
| a host-fixed name whose `:` and `/` must survive | `url_reserved` (RFC 6570 `{+var}`: unreserved, reserved and `%XX` kept) |
| a whole link target from data | `safe_href`, then `attr`: `Some` only for `http://`, `https://` (any case) or a single-`/` path; `None` means render text |
| htmx `hx-vals` | `json_attr` (a JSON object of strings, escaped for an attribute) |

The other direction, HTML that arrived as text (an RSS `description` or
`content:encoded`) turned into the words it shows, is `to_text`, served as
`urn:iki:fn:html-to-text`. It tokenizes with html5ever (Servo's WHATWG tokenizer) and keeps
only text: tags, attribute values, comments, and `script`/`style`/`template` bodies are
dropped, references decoded, a block boundary or `<br>` separates words, whitespace
collapses to one line. The answer is text, so it goes back into a page through `html_text`.
A lift can store it beside the HTML, so a search matches words and not `strong` or `href`.

`compose`'s `$h{…}` escapes with `html_text` and percent-encodes IRI arguments with
`url_segment`, so there is one implementation. The contract is the shared vectors file
[`tests/vectors/html_vectors.json`](tests/vectors/html_vectors.json): hand-written inputs
and outputs that `tests/html_vectors.rs` runs, and that a Python, Deno or XSLT port keeps
a verbatim copy of and runs the same way. The two URI-template forms, `{name}` and
`{+name}`, have their own file,
[`tests/vectors/uri_template_vectors.json`](tests/vectors/uri_template_vectors.json): RFC
6570's examples for those forms plus a few of ours, run by `tests/uri_template_vectors.rs`
against both functions and against compose.

## Conformance

`space()` passes [`ikigai-conformance`](https://github.com/ikigai-rs/ikigai-conformance)
(`tests/conformance.rs`): every input classed, every result cacheable and held to
it, no capability enforced that is not declared. The seven text functions are pure
functions of their arguments (an empty golden-thread set, by design); `compose`,
`conditional` and `each` are exactly as cacheable as the resources they resolve — a shape
served under a golden thread makes the composite cacheable under that thread and a
cut recomputes it, a shape served live makes it uncacheable. The test pins both.
The two `lowerCamelCase` ids (`toUpper`, `reverseList`) are the known exception to
the kebab-case naming convention: they are live MCP tool names, and move only in
the coordinated rename wave, so the test records them as the exact `NAMES` set.

One check is waived, and only for the three resolvers: `OUTPUTS` asks an endpoint
to declare the media type it serves, and none of `compose`, `conditional` and
`each` has one of its own — each returns the resource it resolved, so the served
type is the shape's, the taken branch's or the template's. `outputs` is a closed list with no pass-through
spelling, so the declaration cannot be made true; the waiver is per CHECK
(`Suite::opt_out_check`), never per endpoint, and what it gives up is asserted by
hand over three different source types. `compose` declared `text/html` through
0.2.1 — true of the shapes we happened to write, false as a contract, and it
passed the walk because the fixture was HTML.

## Usage

`space()` names itself `urn:iki:space:fn` (`ikigai_fn::SPACE_ID`); a space you extend with `.bind` is anonymous until you name it.

Mount the whole library at its conventional IRIs and chain your own bindings on
top (`EndpointSpace::bind` is a builder):

```rust
use ikigai_core::{Exact, Kernel};
use std::sync::Arc;

let space = ikigai_fn::space()
    .bind(Exact::new("urn:data:page"), my_page_shape())
    .bind(Exact::new("urn:host:info"), my_host_info());
let kernel = Kernel::new(Arc::new(space));
```

Or pull the individual constructors and bind them at IRIs of your choosing —
binding authority (which IRIs a module's resources live at) is a host concern.

## Namespace migration (0.2.0)

As of **0.2.0** this library's own resources are bound under `urn:iki:fn:` —
`urn:fn:toUpper` is now `urn:iki:fn:toUpper`, and likewise for `reverseList`,
`compose` and `conditional`. The ecosystem is consolidating on a single
`urn:iki:` namespace, because these graphs leave the process (signature graphs,
PROV-O log entries, federation, MCP projection, layer servers) and land in
datasets beside names like `urn:isbn:` that someone else owns.

**Renaming a resource is a breaking change to this crate's real public
interface**, even though its Rust API is untouched — hence the minor bump under
0.x, so no host adopts it by a `cargo update` alone.

A host adopting 0.2.0 should install the prefix alias in the *same* release, so
every name it has already published keeps resolving:

```rust,ignore
use ikigai_core::{AliasTable, Kernel};
use std::sync::Arc;

let table = AliasTable::new().prefix("urn:fn:", "urn:iki:fn:");
let kernel = Kernel::new(Arc::new(ikigai_fn::space())).with_aliases(Arc::new(table));
```

(`Kernel::with_aliases` needs `ikigai-core` 0.1.63 or later.) The alias wraps the
root space, so a nested resolution gets it too — a stored `compose` shape holding
`$a{urn:fn:toUpper?in=x}` still expands. The catalog deliberately advertises only
the **new** name, so catalog-driven consumers migrate themselves while old-name
holders keep working.

The other namespaces this library binds — `urn:demo:` (`wrap`, `split`, `greet`,
`echo`) and the `urn:data:`/`urn:host:` names in the examples below — are **not**
renamed, and must not be renamed here. They are bound in several repos at once,
and a per-prefix alias cannot describe a half-moved shared prefix: aliasing
`urn:demo:` while only this crate has moved would rewrite another repo's
`urn:demo:*` to a name nothing binds. A shared namespace moves atomically across
every binder or not at all.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
