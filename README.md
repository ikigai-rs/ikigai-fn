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
| `compose()` | `urn:iki:fn:compose` | recursive `$a{<iri>}` resource transclusion |
| `conditional()` | `urn:iki:fn:conditional` | lazy `if`/`then`/`else`; only the taken branch runs |
| `split()` | `urn:demo:split` | splits `in` on commas into a newline list |
| `wrap()` | `urn:demo:wrap` | surrounds the `text` argument with `[ ]` |
| `greet()` | `urn:demo:greet` | combines `greeting` and `name` |
| `echo()` | `urn:demo:echo/{message}` | returns the grammar-captured `message` |

Each `snake_case` constructor builds an endpoint whose `lowerCamelCase`
identifier matches its name.

Every input declares its datatype (`xsd:string` for text, `xsd:anyURI` for the
IRI-valued `src`/`if`/`then`/`else`), so type-driven selection — `select_action`,
`urn:kernel:actions types=` — offers these endpoints for the values you hold. A
test pins that no input in `space()` is left unclassed.

## Usage

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
