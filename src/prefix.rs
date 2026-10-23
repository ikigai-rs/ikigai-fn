//! **Prefixes as scoped names**: `json:query` for `urn:json:query`, inside a dynamic space
//! that wraps a composition, and nowhere else.
//!
//! A prefix is not a template-syntax trick. It is a real name, declared as DATA: a prefix
//! table is an `ik:Alias` whose rules rewrite `json:` to `urn:json:`, each resolving from
//! the root under the caller's capability (`ik:resolveAt "root"`, core 0.1.110), over an
//! empty space. The table is a resource the host serves, and a [`DynamicSpace`] built from
//! it ([`prefix_space`]) is the place the short names exist. The host injects that space as
//! a corridor around the requests that compose (`Scope::empty().with(prefixes)` on
//! `Kernel::issue_in`), and every sub-request those requests give rise to, at any depth,
//! inherits the corridor: a marker's `json:query`, and the `data:people` that the jq
//! endpoint is asked to read, both resolve. A request issued without it finds no such
//! name.
//!
//! ```turtle
//! @prefix ik: <https://ikigai-rs.dev/ns#> .
//! @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
//!
//! <urn:example:prefixes> a ik:Alias ;
//!     ik:maxHops 8 ;
//!     ik:rewrites <urn:example:prefixes:rule:1> ;
//!     ik:space <urn:example:prefixes:space> .
//!
//! <urn:example:prefixes:rule:1> a ik:RewriteRule ;
//!     ik:ruleKind "prefix" ;
//!     ik:logical "json:" ;
//!     ik:canonical "urn:json:" ;
//!     ik:resolveAt "root" .
//!
//! <urn:example:prefixes:space> a ik:EndpointSpace ;
//!     ik:doors rdf:nil .
//! ```
//!
//! # Why a corridor and not a `Level`
//!
//! A `Level` scopes the sub-requests of the endpoints found INSIDE it. The jq endpoint is
//! the host's, found outside, so the source it reads (`data:people`) would not resolve
//! there. A corridor is inherited by every sub-request of the request it was injected for,
//! wherever the endpoint answering it lives.
//!
//! # What the table may say ([`prefix_check`])
//!
//! A corridor is consulted ahead of the root, so a table that could match a name the host
//! answers would stand in for it. The check every [`prefix_space`] runs refuses, before the
//! table binds:
//!
//! - anything but one `ik:Alias` over an empty `ik:EndpointSpace` (no doors, no limits, no
//!   mounts: a table can rewrite names and do nothing else);
//! - a rule that does not resolve from the root (a child rule over an empty space resolves
//!   nothing, so it can only be a mistake);
//! - a rule whose name is under a scheme the kernel and the web use
//!   ([`RESERVED_SCHEMES`]: `urn`, `http`, `https`, `file` and the rest). A host's names
//!   live under those, so a prefix can never match one.
//!
//! A refused table keeps the doors in force and is reported in the space's status.
//!
//! ⚠ That is the whole guarantee: a host that serves names under some OTHER scheme (its own
//! `page:` names, say) and declares a prefix for that same scheme is shadowed inside the
//! corridor. Core has no "answer only if the root misses" placement for a corridor, so the
//! scheme list is the line. Every ikigai host's names are `urn:`.
//!
//! # What a prefix grants: nothing
//!
//! A rewritten name is issued from the root under the caller's own capability, so a caller
//! who may not read the target through its full name may not read it through the short one.
//! And the table is read only from its declaration resource, under the capability the host
//! hands `DynamicSpace::refresh`: no request argument reaches it.
//!
//! # Changing the table
//!
//! A write to the declaration is a cut of its golden thread. The host's loop
//! (`DynamicSpace::drive`, behind core's `declare` feature) re-reads and rebinds it, and
//! a successful bind cuts the space's own thread, from which every answer served through a
//! prefix hangs: what was composed with the old table recomputes.

use std::sync::Arc;

use ikigai_core::{DynamicSpace, Iri, Registry, ResolveAt, SpaceCheck, SpaceKind, Topology};

/// The schemes a prefix may not be declared under: the kernel's own (`urn`) and the ones
/// the web and a host's files are named with. A prefix under one of them could match a
/// name the host already answers, and the corridor is consulted first.
pub const RESERVED_SCHEMES: &[&str] = &[
    "urn", "http", "https", "file", "ftp", "ws", "wss", "data", "mailto", "tag", "did", "about",
    "blob",
];

/// A dynamic space named `urn:iki:space:dynamic:{name}`, built from the prefix table at
/// `declaration` and checked by [`prefix_check`]. Empty until the host binds the table
/// (`refresh` once, or `drive` to follow its edits; both behind core's `declare` feature).
///
/// It holds no endpoints of its own (its registry is empty), so a table cannot bind one.
///
/// ```
/// use std::sync::Arc;
/// use futures::executor::block_on;
/// use ikigai_core::{
///     ArgRef, Capability, Exact, Iri, Kernel, Request, Scope, Space, Verb,
/// };
///
/// let table = r#"
/// @prefix ik: <https://ikigai-rs.dev/ns#> .
/// @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
/// <urn:example:p> a ik:Alias ; ik:maxHops 8 ;
///     ik:rewrites <urn:example:p:rule:1> ; ik:space <urn:example:p:space> .
/// <urn:example:p:rule:1> a ik:RewriteRule ; ik:ruleKind "prefix" ;
///     ik:logical "fn:" ; ik:canonical "urn:iki:fn:" ; ik:resolveAt "root" .
/// <urn:example:p:space> a ik:EndpointSpace ; ik:doors rdf:nil .
/// "#;
/// let kernel = Kernel::new(Arc::new(ikigai_fn::space()));
/// let prefixes = Arc::new(ikigai_fn::prefix_space("demo", Iri::parse("urn:example:p").unwrap()));
/// block_on(prefixes.install(&kernel, table)).unwrap();
///
/// let upper = || {
///     Request::new(Verb::Source, Iri::parse("fn:toUpper").unwrap())
///         .with_arg("in", ArgRef::Inline(b"roc".to_vec()))
/// };
/// // Inside the space the short name resolves ...
/// let inside = Scope::empty().with(prefixes.clone() as Arc<dyn Space>);
/// let answer = block_on(kernel.issue_in(upper(), &Capability::root(), inside)).unwrap();
/// assert_eq!(answer.bytes, b"ROC");
/// // ... and outside it there is no such name.
/// assert!(block_on(kernel.issue(upper(), &Capability::root())).is_err());
/// ```
pub fn prefix_space(name: &str, declaration: Iri) -> DynamicSpace {
    DynamicSpace::new(name, declaration, Registry::new()).with_check(prefix_check())
}

/// The check a prefix table must pass before it binds: one `ik:Alias` of root-resolving
/// rules, none under a [reserved scheme](RESERVED_SCHEMES), over an empty space. See the
/// [module docs](self).
pub fn prefix_check() -> SpaceCheck {
    Arc::new(|topology: Topology| {
        let verdict = check(&topology);
        Box::pin(async move { verdict })
    })
}

fn check(topology: &Topology) -> std::result::Result<(), String> {
    let SpaceKind::Alias { rules, .. } = &topology.kind else {
        return Err(
            "a prefix table is one ik:Alias over an empty ik:EndpointSpace, and nothing else"
                .to_string(),
        );
    };
    match topology.children.as_slice() {
        [child]
            if child.children.is_empty()
                && matches!(&child.kind, SpaceKind::EndpointSpace { doors } if doors.is_empty()) => {
        }
        _ => {
            return Err(
                "a prefix table's ik:Alias encloses one empty ik:EndpointSpace (no doors): a \
                 table rewrites names and binds nothing"
                    .to_string(),
            )
        }
    }
    for rule in rules {
        if rule.at != ResolveAt::Root {
            return Err(format!(
                "prefix `{}` must resolve from the root (ik:resolveAt \"root\"): over an empty \
                 space a child rule resolves nothing",
                rule.from
            ));
        }
        let scheme = rule
            .from
            .split_once(':')
            .map(|(scheme, _)| scheme)
            .filter(|scheme| is_scheme(scheme))
            .ok_or_else(|| {
                format!(
                    "prefix `{}` does not begin with a scheme and its `:` (`json:`, say)",
                    rule.from
                )
            })?;
        if RESERVED_SCHEMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(scheme))
        {
            return Err(format!(
                "prefix `{}` is under the reserved scheme `{scheme}:` — the host's own names \
                 live there, and a prefix may never stand in for one",
                rule.from
            ));
        }
    }
    Ok(())
}

/// RFC 3986's `scheme`: a letter, then letters, digits, `+`, `-` and `.`.
fn is_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}
