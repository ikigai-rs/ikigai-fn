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
| `to_upper()` | `urn:fn:toUpper` | upper-cases the `in` argument |
| `reverse_list()` | `urn:fn:reverseList` | reverses newline-separated items in `in` |
| `compose()` | `urn:fn:compose` | recursive `$a{<iri>}` resource transclusion |
| `split()` | `urn:demo:split` | splits `in` on commas into a newline list |
| `wrap()` | `urn:demo:wrap` | surrounds the `text` argument with `[ ]` |
| `greet()` | `urn:demo:greet` | combines `greeting` and `name` |
| `echo()` | `urn:demo:echo/{message}` | returns the grammar-captured `message` |

Each `snake_case` constructor builds an endpoint whose `lowerCamelCase`
identifier matches its name.

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

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
