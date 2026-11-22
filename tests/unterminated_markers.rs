//! Scanning an unterminated marker used to rescan the rest of the template from every
//! `$h{`, so a fill was quadratic in the template's length; the node budget never saw
//! it, because the whole template is one literal node (ledger #4002).
//!
//! This is a wall-clock bound with wide margin (a linear scan of 300 KB takes
//! milliseconds even in a debug build; the quadratic one took 5.5 s in release). The
//! exact proof is the step-counting unit test `marker_end_scan_is_linear` in
//! `src/lib.rs`, beside the differential test against the old scanner.
mod common;
use common::*;
use ikigai_core::{Exact, FnEndpoint, Invocation, Kernel, Representation};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fill(template: String) -> (String, Duration) {
    let k = Kernel::new(Arc::new(ikigai_fn::space().bind(
        Exact::new("urn:example:template"),
        FnEndpoint::new("t", move |_: &Invocation<'_>| {
            Ok(Representation::new(
                text_html(),
                template.as_bytes().to_vec(),
            ))
        }),
    )));
    let start = Instant::now();
    let out = compose(&k, "urn:example:template", &[]).unwrap();
    (out, start.elapsed())
}

#[test]
fn unterminated_markers_scan_in_linear_time() {
    // 100k unterminated `$h{` (300 KB) against a template of the same length with no `$`.
    let template = "$h{".repeat(100_000);
    let (out, markers) = fill(template.clone());
    let (_, plain) = fill("abc".repeat(100_000));
    println!("unterminated: {markers:?}, plain: {plain:?}");
    // Unterminated markers stay literal.
    assert_eq!(out, template);
    assert!(
        markers < Duration::from_secs(1),
        "a 300 KB template took {markers:?} to fill"
    );
}

#[test]
fn unterminated_markers_in_quotes_and_braces_scan_in_linear_time() {
    // Openers whose scans each cross nested braces and quoted spans before running off
    // the end.
    for unit in ["$r{{", "$h{\"", "$h{\"\\\"{", "{$h{"] {
        let template = unit.repeat(60_000);
        let (out, took) = fill(template.clone());
        assert_eq!(out, template, "{unit:?}");
        assert!(
            took < Duration::from_secs(1),
            "{unit:?} x 60k took {took:?} to fill"
        );
    }
}
