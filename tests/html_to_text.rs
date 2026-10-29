//! HTML as text: [`ikigai_fn::html::to_text`] and the `html-to-text` endpoint (ledger
//! #2528).
//!
//! An RSS description or `content:encoded` holds HTML as TEXT (escaped, or in CDATA), so
//! XSLT 1.0 sees `<strong>` as five letters and a word, and a search over the feed
//! matches markup: `strong`, `img`, `href`. These tests pin that the text face holds the
//! words a reader sees, and none of the markup around them.

use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Error, Iri, Kernel, Request, Verb};
use ikigai_fn::html::to_text;
use std::sync::Arc;

/// The census's three search misses: a tag name, a picture, a link. None of `strong`,
/// `img`, `src` or `href` is a word a reader sees.
#[test]
fn markup_is_not_text() {
    let description = r#"<p>Read <strong>this</strong> post.</p>
<p><img src="https://example.org/cat.jpg" alt="a cat"> and
<a href="https://example.org/more">more</a></p>"#;
    let text = to_text(description);
    assert_eq!(text, "Read this post. and more");
    for markup in ["strong", "img", "src", "href", "<", ">", "example.org"] {
        assert!(!text.contains(markup), "{markup:?} in {text:?}");
    }
}

/// Character references are decoded, so an escaped `&amp;` reads `&` and a search for
/// `Tom & Jerry` finds it; and `&lt;b&gt;` is text that says `<b>`, not a tag.
#[test]
fn references_are_decoded_once() {
    assert_eq!(
        to_text("Tom &amp; Jerry&#39;s &quot;caf&eacute;&quot;"),
        "Tom & Jerry's \"café\""
    );
    assert_eq!(to_text("&lt;b&gt;bold&lt;/b&gt;"), "<b>bold</b>");
    assert_eq!(to_text("&#x1F600; &nbsp;x"), "\u{1F600} \u{A0}x");
}

/// A block boundary and `<br>` separate words; inline elements do not. Whitespace runs
/// collapse to one space and the ends are trimmed.
#[test]
fn blocks_separate_words_and_inline_elements_do_not() {
    assert_eq!(to_text("<p>one</p><p>two</p>"), "one two");
    assert_eq!(to_text("line<br>break<br/>again"), "line break again");
    assert_eq!(to_text("<li>a</li><li>b</li>"), "a b");
    assert_eq!(to_text("un<em>bel</em>ievable"), "unbelievable");
    assert_eq!(to_text("  a \n\t  b  "), "a b");
    assert_eq!(
        to_text("<div><h2>T</h2><table><tr><td>x</td><td>y</td></tr></table></div>"),
        "T x y"
    );
}

/// Code and inert content is not text: script, style and template bodies are dropped,
/// and a `</p>` inside a script does not end it (the tokenizer is in script state, as a
/// browser is). Comments, doctypes and CDATA-as-bogus-comment are dropped too.
#[test]
fn code_and_comments_are_not_text() {
    assert_eq!(
        to_text("a<script>if (1 < 2) { x = '</p>'; }</script>b<style>p { color: red }</style>c"),
        "abc"
    );
    assert_eq!(to_text("a<template><p>hidden</p></template>b"), "ab");
    assert_eq!(to_text("<!DOCTYPE html>a<!-- not <b>this</b> -->b"), "ab");
    assert_eq!(to_text("a<iframe><p>fallback</p></iframe>b"), "ab");
    // Nested template: the outer close is the one that ends it.
    assert_eq!(
        to_text("a<template><template>x</template>y</template>b"),
        "ab"
    );
}

/// Raw-text elements keep their markup-looking text as text, the way a browser shows it.
#[test]
fn rcdata_is_text_not_markup() {
    assert_eq!(to_text("<title>a <b> c</title>"), "a <b> c");
    assert_eq!(
        to_text("<textarea>x &amp; <i>y</i></textarea>"),
        "x & <i>y</i>"
    );
}

/// Broken markup still yields its words: the tokenizer recovers as a browser does.
#[test]
fn broken_markup_yields_its_words() {
    assert_eq!(to_text("<p>unclosed <b>bold"), "unclosed bold");
    assert_eq!(to_text("a < b and c > d"), "a < b and c > d");
    assert_eq!(to_text("x <a href=\"y"), "x");
    assert_eq!(to_text("AT&T &bogus; 5 &lt 6"), "AT&T &bogus; 5 < 6");
    assert_eq!(to_text(""), "");
    assert_eq!(to_text("a\0b"), "ab");
}

/// The text face is plain text: safe in a page only through `html_text`, like any text.
#[test]
fn the_text_face_round_trips_through_html_text() {
    let html = "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>";
    let text = to_text(html);
    assert_eq!(text, "<script>alert(1)</script>");
    assert_eq!(
        ikigai_fn::html::html_text(&text),
        "&lt;script&gt;alert(1)&lt;/script&gt;"
    );
}

fn source(iri: &str, input: &[u8]) -> ikigai_core::Result<ikigai_core::Representation> {
    let kernel = Kernel::new(Arc::new(ikigai_fn::space()));
    let request = Request::new(Verb::Source, Iri::parse(iri).unwrap())
        .with_arg("in", ArgRef::Inline(input.to_vec()));
    block_on(kernel.issue(request, &Capability::root()))
}

/// `space()` serves it at `urn:iki:fn:html-to-text`, as cacheable UTF-8 text.
#[test]
fn the_endpoint_serves_the_text_face() {
    let rep = source(
        "urn:iki:fn:html-to-text",
        b"<p>Read <strong>this</strong> &amp; that</p>",
    )
    .unwrap();
    assert_eq!(rep.bytes, b"Read this & that");
    assert_eq!(rep.repr_type.media_type, "text/plain");
}

/// Non-UTF-8 input is refused, naming the input, as every `xsd:string` input is.
#[test]
fn non_utf8_input_is_refused() {
    match source("urn:iki:fn:html-to-text", b"\xff<p>x</p>") {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "in"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}
