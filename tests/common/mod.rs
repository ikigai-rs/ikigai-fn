//! Fixtures shared by the template tests: fixed resources, an in-memory atom, and the
//! few calls every test makes.

#![allow(dead_code)] // each test binary uses its own subset of these fixtures

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use ikigai_core::{
    ArgRef, Capability, Error, FnEndpoint, Invocation, Iri, Kernel, ReprType, Representation,
    Request, Result, Verb,
};

pub fn iri(name: &str) -> Iri {
    Iri::parse(name).expect("a valid IRI")
}

/// `text/html; charset=utf-8`, the type every HTML fixture serves.
pub fn text_html() -> ReprType {
    ReprType::new("text/html").with_param("charset", "utf-8")
}

/// A resource that answers `body` as cacheable HTML.
pub fn html(body: &'static str) -> FnEndpoint {
    FnEndpoint::new("fixed", move |_: &Invocation<'_>| {
        Ok(Representation::new(text_html(), body.as_bytes().to_vec()).cacheable())
    })
}

/// A resource that answers `body` and counts how often it was asked.
pub fn counted(body: &'static str, reads: Arc<AtomicUsize>) -> FnEndpoint {
    FnEndpoint::new("counted", move |_: &Invocation<'_>| {
        reads.fetch_add(1, Ordering::SeqCst);
        Ok(Representation::new(text_html(), body.as_bytes().to_vec()).cacheable())
    })
}

/// A resource that always refuses with `error`.
pub fn failing(error: fn() -> Error) -> FnEndpoint {
    FnEndpoint::new("failing", move |_: &Invocation<'_>| Err(error()))
}

/// The ATOM: text held in memory under the name's `{key}`. `Source` answers it,
/// cacheable, or `NotFound` if nothing was written; `Sink` writes `content`. It declares
/// no golden thread: the kernel hangs a cacheable read from the thread named after the
/// resource, and cuts that thread after a successful `Sink` to the same name.
pub fn store(held: Arc<Mutex<BTreeMap<String, String>>>) -> FnEndpoint {
    FnEndpoint::new("store", move |inv: &Invocation<'_>| {
        let key = inv
            .bindings
            .get("key")
            .ok_or_else(|| Error::MissingArgument("key".to_string()))?
            .to_string();
        let mut held = held.lock().expect("the store's lock");
        match inv.request.verb {
            Verb::Sink => {
                held.insert(key, inv.inline_str("content")?.to_string());
                Ok(Representation::new(text_html(), b"ok".to_vec()))
            }
            _ => match held.get(&key) {
                Some(text) => {
                    Ok(Representation::new(text_html(), text.clone().into_bytes()).cacheable())
                }
                None => Err(Error::NotFound(format!("nothing is stored at `{key}`"))),
            },
        }
    })
}

pub fn issue(kernel: &Kernel, request: Request) -> Result<Representation> {
    block_on(kernel.issue(request, &Capability::root()))
}

/// SOURCE `name` with inline `args`.
pub fn source_request(name: &str, args: &[(&str, &str)]) -> Request {
    args.iter()
        .fold(Request::new(Verb::Source, iri(name)), |request, (k, v)| {
            request.with_arg(*k, ArgRef::Inline(v.as_bytes().to_vec()))
        })
}

/// SOURCE `name` and read the answer as text.
pub fn text(kernel: &Kernel, name: &str) -> Result<String> {
    let repr = issue(kernel, source_request(name, &[]))?;
    Ok(String::from_utf8(repr.bytes).expect("UTF-8"))
}

/// The compose request over the template `src`, with template arguments.
pub fn compose_request(src: &str, args: &[(&str, &str)]) -> Request {
    let mut all = vec![("src", src)];
    all.extend_from_slice(args);
    source_request("urn:iki:fn:compose", &all)
}

/// Compose `src` with template arguments and read the answer as text.
pub fn compose(kernel: &Kernel, src: &str, args: &[(&str, &str)]) -> Result<String> {
    let repr = issue(kernel, compose_request(src, args))?;
    Ok(String::from_utf8(repr.bytes).expect("UTF-8"))
}

/// SINK `content` to `name`.
pub fn sink(kernel: &Kernel, name: &str, content: &str) -> Result<Representation> {
    issue(
        kernel,
        Request::new(Verb::Sink, iri(name))
            .with_arg("content", ArgRef::Inline(content.as_bytes().to_vec())),
    )
}

pub fn cached(kernel: &Kernel, request: &Request) -> bool {
    kernel.is_cached(request, &Capability::root())
}
