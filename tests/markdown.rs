//! The rules of `ikigai_fn::markdown`, one test each, with hostile inputs. The expected
//! fragments are written by hand: they are the contract.

use futures::executor::block_on;
use ikigai_core::{
    ArgRef, Capability, Error, Exact, Expiry, FnEndpoint, Invocation, Iri, Kernel, ReprType,
    Representation, Request, Verb,
};
use ikigai_fn::markdown::{
    map_text_nodes, render_untrusted, try_render_untrusted, Options, Refused, MAX_INPUT_BYTES,
    MAX_NESTING,
};
use std::sync::Arc;

fn render(text: &str) -> String {
    render_untrusted(text, &Options::default())
}

fn assert_well_formed(fragment: &str) {
    let document = format!("<root>{fragment}</root>");
    if let Err(error) = roxmltree::Document::parse(&document) {
        panic!("not well-formed XML ({error}): {fragment:?}");
    }
}

// --- the dialect ----------------------------------------------------------------------------

#[test]
fn tables_strikethrough_and_task_lists_are_on() {
    assert_eq!(
        render("|a|b|c|\n|:-|:-:|--|\n|1|2|3|"),
        "<table>\n<thead>\n<tr><th scope=\"col\" style=\"text-align: left\">a</th>\
         <th scope=\"col\" style=\"text-align: center\">b</th><th scope=\"col\">c</th></tr>\n\
         </thead>\n<tbody>\n<tr><td style=\"text-align: left\">1</td>\
         <td style=\"text-align: center\">2</td><td>3</td></tr>\n</tbody>\n</table>"
    );
    assert_eq!(render("~~gone~~"), "<p><del>gone</del></p>");
    assert_eq!(
        render("- [x] done\n- [ ] open"),
        "<ul>\n<li><input type=\"checkbox\" disabled=\"disabled\" checked=\"checked\" \
         aria-label=\"task\"/> done</li>\n<li><input type=\"checkbox\" disabled=\"disabled\" \
         aria-label=\"task\"/> open</li>\n</ul>"
    );
}

#[test]
fn a_table_without_a_body_has_no_tbody() {
    assert_eq!(
        render("|a|\n|-|"),
        "<table>\n<thead>\n<tr><th scope=\"col\">a</th></tr>\n</thead>\n</table>"
    );
}

#[test]
fn extensions_left_off_stay_text() {
    assert_eq!(render("a[^1]"), "<p>a[^1]</p>");
    // Without footnotes this is a link reference definition, to a target that is refused.
    assert_eq!(render("a[^1]\n\n[^1]: note"), "<p>a^1</p>");
    assert_eq!(render("# t {#id .c}"), "<h3>t {#id .c}</h3>");
}

// --- raw HTML -------------------------------------------------------------------------------

#[test]
fn raw_html_is_text_never_markup() {
    for (input, expected) in [
        (
            "<script>alert(1)</script>",
            "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>",
        ),
        (
            "<img src=x onerror=alert(1)>",
            "<p>&lt;img src=x onerror=alert(1)&gt;</p>",
        ),
        (
            "a <img src=x onerror=alert(1)> b",
            "<p>a &lt;img src=x onerror=alert(1)&gt; b</p>",
        ),
        (
            "hi <script>alert(1)</script>",
            "<p>hi &lt;script&gt;alert(1)&lt;/script&gt;</p>",
        ),
        ("<!-- x -->", "<p>&lt;!-- x --&gt;</p>"),
        ("<![CDATA[ <b> ]]>", "<p>&lt;![CDATA[ &lt;b&gt; ]]&gt;</p>"),
        ("<?php echo 1 ?>", "<p>&lt;?php echo 1 ?&gt;</p>"),
        (
            "<div onclick=\"x\">\ntwo lines\n</div>",
            "<p>&lt;div onclick=&quot;x&quot;&gt;\ntwo lines\n&lt;/div&gt;</p>",
        ),
    ] {
        assert_eq!(render(input), expected, "{input:?}");
    }
}

#[test]
fn a_tag_is_never_written_from_the_input_however_it_is_spelled() {
    for input in [
        "&lt;script&gt;",
        "&#60;script&#62;",
        "\\<script>",
        "`<script>`",
        "```\n<script>\n```",
        "*<script>*",
        "[<script>](/a)",
        "![<script>](/a)",
        "|<script>|\n|-|",
    ] {
        let output = render(input);
        assert!(!output.contains("<script"), "{input:?}: {output}");
        assert!(output.contains("&lt;script"), "{input:?}: {output}");
    }
}

// --- links ----------------------------------------------------------------------------------

#[test]
fn the_allowed_links_become_links() {
    for (input, expected) in [
        (
            "[a](https://e.org/x?y=1&z=2)",
            "<a href=\"https://e.org/x?y=1&amp;z=2\" rel=\"noopener noreferrer\">a</a>",
        ),
        (
            "[a](HTTP://E.ORG/)",
            "<a href=\"HTTP://E.ORG/\" rel=\"noopener noreferrer\">a</a>",
        ),
        (
            "[a](mailto:me@e.org)",
            "<a href=\"mailto:me@e.org\" rel=\"noopener noreferrer\">a</a>",
        ),
        (
            "[a](MailTo:me@e.org)",
            "<a href=\"MailTo:me@e.org\" rel=\"noopener noreferrer\">a</a>",
        ),
        (
            "<me@e.org>",
            "<a href=\"mailto:me@e.org\" rel=\"noopener noreferrer\">me@e.org</a>",
        ),
        (
            "<https://e.org/>",
            "<a href=\"https://e.org/\" rel=\"noopener noreferrer\">https://e.org/</a>",
        ),
        ("[a](/x/y)", "<a href=\"/x/y\">a</a>"),
        ("[a](?q=1)", "<a href=\"?q=1\">a</a>"),
        ("[a](#top)", "<a href=\"#top\">a</a>"),
        ("[*a*](/x)", "<a href=\"/x\"><em>a</em></a>"),
        ("[a][r]\n\n[r]: /x", "<a href=\"/x\">a</a>"),
    ] {
        assert_eq!(render(input), format!("<p>{expected}</p>"), "{input:?}");
    }
}

#[test]
fn every_other_link_renders_only_its_words() {
    for input in [
        "[run](javascript:alert(1))",
        "[run](JaVaScRiPt:alert(1))",
        "[run](java&#115;cript:alert(1))",
        "[run](&#106;avascript:alert(1))",
        "[run](java&#9;script:alert(1))",
        "[run](data:text/html,x)",
        "[run](vbscript:msgbox(1))",
        "[run](file:///etc/passwd)",
        "[run](//evil.example/)",
        "[run](/\\evil.example/)",
        "[run](&#47;/evil.example/)",
        "[run](/&#92;evil.example/)",
        "[run](ftp://e.org/)",
        "[run](relative/path)",
        "[run](mailto:a&#9;b@e.org)",
        "[run](?a&#10;b)",
        "[run](#a&#127;b)",
        "[run][r]\n\n[r]: javascript:alert(1)",
    ] {
        assert_eq!(render(input), "<p>run</p>", "{input:?}");
    }
}

#[test]
fn a_refused_autolink_keeps_its_words_as_text() {
    assert_eq!(
        render("<javascript:alert(1)>"),
        "<p>javascript:alert(1)</p>"
    );
}

#[test]
fn a_refused_link_keeps_its_formatting() {
    assert_eq!(
        render("[**run**](javascript:x)"),
        "<p><strong>run</strong></p>"
    );
}

#[test]
fn a_target_or_title_cannot_leave_its_attribute() {
    assert_eq!(
        render("[a](</x\"onclick=\"y>)"),
        "<p><a href=\"/x&quot;onclick=&quot;y\">a</a></p>"
    );
    assert_eq!(
        render("[a](/x '\"><script>')"),
        "<p><a href=\"/x\" title=\"&quot;&gt;&lt;script&gt;\">a</a></p>"
    );
}

#[test]
fn only_a_link_off_this_origin_carries_rel() {
    assert!(render("[a](https://e.org/)").contains("rel=\"noopener noreferrer\""));
    for same_origin in ["[a](/x)", "[a](?x)", "[a](#x)"] {
        assert!(!render(same_origin).contains("rel="), "{same_origin}");
    }
}

#[test]
fn a_link_with_no_words_is_labeled_with_its_target() {
    assert_eq!(render("[](/x)"), "<p><a href=\"/x\">/x</a></p>");
    assert_eq!(render("![](/x)"), "<p><a href=\"/x\">/x</a></p>");
    assert_eq!(render("[![](/i)](/x)"), "<p><a href=\"/x\">/x</a></p>");
}

// --- images ---------------------------------------------------------------------------------

#[test]
fn an_image_is_a_link_with_its_alt_text() {
    assert_eq!(
        render("![a cat](/cat.png)"),
        "<p><a href=\"/cat.png\">a cat</a></p>"
    );
    assert_eq!(
        render("![a](https://e.org/i.png \"t\")"),
        "<p><a href=\"https://e.org/i.png\" rel=\"noopener noreferrer\" title=\"t\">a</a></p>"
    );
}

#[test]
fn an_image_with_a_refused_target_is_its_alt_text() {
    assert_eq!(render("![cat](javascript:x)"), "<p>cat</p>");
    assert_eq!(render("![cat](data:image/png;base64,AA)"), "<p>cat</p>");
}

#[test]
fn an_image_never_becomes_an_img() {
    for input in [
        "![a](/x)",
        "![a](https://e.org/i.png)",
        "[![a](/i)](/x)",
        "![a][r]\n\n[r]: /x",
        "<img src=/x>",
    ] {
        let output = render(input);
        assert!(!output.contains("<img"), "{input:?}: {output}");
    }
}

#[test]
fn alt_text_is_flat_so_links_do_not_nest() {
    assert_eq!(
        render("![a *b* [c](/d) ![e](/f) `g`](/h)"),
        "<p><a href=\"/h\">a b c e g</a></p>"
    );
    assert_eq!(
        render("[![in](/img)](/link)"),
        "<p><a href=\"/link\">in</a></p>"
    );
}

// --- headings -------------------------------------------------------------------------------

#[test]
fn headings_start_below_the_page() {
    assert_eq!(render("# a"), "<h3>a</h3>");
    assert_eq!(render("## a"), "<h4>a</h4>");
    assert_eq!(render("a\n="), "<h3>a</h3>");
}

#[test]
fn heading_offset_moves_them_and_h6_is_the_cap() {
    let with = |offset, input| {
        render_untrusted(
            input,
            &Options {
                heading_offset: offset,
            },
        )
    };
    assert_eq!(with(0, "# a"), "<h1>a</h1>");
    assert_eq!(with(4, "## a"), "<h6>a</h6>");
    assert_eq!(with(2, "###### a"), "<h6>a</h6>");
    assert_eq!(with(255, "###### a"), "<h6>a</h6>");
}

// --- code -----------------------------------------------------------------------------------

#[test]
fn a_code_block_names_a_valid_language() {
    assert_eq!(
        render("```c++ extra\nx\n```"),
        "<pre><code class=\"language-c++\">x\n</code></pre>"
    );
    assert_eq!(
        render("~~~A_b-9\nx\n~~~"),
        "<pre><code class=\"language-A_b-9\">x\n</code></pre>"
    );
    assert_eq!(render("    x\n"), "<pre><code>x\n</code></pre>");
    assert_eq!(render("```\nx\n```"), "<pre><code>x\n</code></pre>");
}

#[test]
fn a_hostile_language_is_dropped() {
    for info in [
        "\"><script>",
        "a\"b",
        "a'b",
        "a&b",
        "a<b",
        "rust,ignore",
        "é",
        "&quot;",
    ] {
        let input = format!("```{info}\nx\n```");
        assert_eq!(render(&input), "<pre><code>x\n</code></pre>", "{info}");
    }
}

#[test]
fn code_is_escaped_and_not_highlighted() {
    assert_eq!(
        render("```rust\nlet a = \"<b>\" & 'c';\n```"),
        "<pre><code class=\"language-rust\">let a = &quot;&lt;b&gt;&quot; &amp; &#39;c&#39;;\n\
         </code></pre>"
    );
    assert_eq!(render("`<b>`"), "<p><code>&lt;b&gt;</code></p>");
}

// --- well-formed, deterministic, bounded ------------------------------------------------------

/// Everything hostile in one list, for the properties that hold of every output.
const HOSTILE: &[&str] = &[
    "<script>alert(1)</script>",
    "<img src=x onerror=alert(1)>",
    "<a href='x",
    "</p></div>",
    "&",
    "&amp;",
    "&#0;",
    "&#1;",
    "&#xD800;",
    "&#xFFFE;",
    "&#xFFFF;",
    "&#x10FFFF;",
    "a\u{0}b\u{1}c\u{8}d\u{B}e\u{C}f\u{1F}g\u{7F}h",
    "\u{FFFE}\u{FFFF}",
    "[a](/x&#xFFFE; 't&#xFFFF;')",
    "![a&#xFFFE;](/x)",
    "```&#xFFFE;\nx\u{FFFF}\n```",
    "[a](</x\"y'z>)",
    "[a](/x '<\">')",
    "![[a](/b)](/c)",
    "[![a](/b)](/c)",
    "[a](/x)[b](/y)",
    "[[a](/x)](/y)",
    "*a **b* c**",
    "~~a *b~~ c*",
    "> - a\n>   1. b\n>      > c",
    "|a|\n|-|\n|<b>|\n|[x](javascript:y)|",
    "- [x] <b>\n- [ ] &",
    "# <h1>\n\n<h1>\n\n</h1>",
    "```\nunclosed <b>",
    "`unclosed <b>",
    "a\r\nb\r\r\n",
];

#[test]
fn every_output_is_well_formed_xml() {
    for input in HOSTILE {
        assert_well_formed(&render(input));
    }
}

#[test]
fn every_output_is_deterministic() {
    for input in HOSTILE {
        assert_eq!(render(input), render(input), "{input:?}");
    }
}

#[test]
fn nothing_xml_cannot_carry_survives() {
    assert_eq!(render("a&#xFFFE;b&#xFFFF;c"), "<p>a\u{FFFD}b\u{FFFD}c</p>");
    assert_eq!(
        render("[a](/x&#xFFFF;)"),
        "<p><a href=\"/x\u{FFFD}\">a</a></p>"
    );
}

#[test]
fn an_input_at_the_size_limit_renders_and_one_past_it_is_refused() {
    let exact = "a".repeat(MAX_INPUT_BYTES);
    assert_eq!(
        try_render_untrusted(&exact, &Options::default()).unwrap(),
        format!("<p>{exact}</p>")
    );
    let over = "a".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        try_render_untrusted(&over, &Options::default()),
        Err(Refused::TooLarge {
            bytes: MAX_INPUT_BYTES + 1
        })
    );
}

fn nested_quotes(levels: usize) -> String {
    format!("{} x", ">".repeat(levels))
}

fn nested_lists(levels: usize) -> String {
    (0..levels)
        .map(|level| format!("{}- x\n", "  ".repeat(level)))
        .collect()
}

fn nested_emphasis(levels: usize) -> String {
    format!("{}x{}", "*a ".repeat(levels), " b*".repeat(levels))
}

#[test]
fn nesting_at_the_limit_renders() {
    let options = Options::default();
    for (input, open_tag) in [
        (nested_quotes(MAX_NESTING), "<blockquote>"),
        (nested_lists(MAX_NESTING), "<ul>"),
        (nested_emphasis(MAX_NESTING), "<em>"),
    ] {
        let output = try_render_untrusted(&input, &options).unwrap();
        assert_eq!(output.matches(open_tag).count(), MAX_NESTING, "{open_tag}");
        assert_well_formed(&output);
    }
}

#[test]
fn nesting_past_the_limit_is_refused_cleanly() {
    let options = Options::default();
    for input in [
        nested_quotes(MAX_NESTING + 1),
        nested_lists(MAX_NESTING + 1),
        nested_emphasis(MAX_NESTING + 1),
        "> ".repeat(MAX_INPUT_BYTES / 2),
        ">".repeat(MAX_INPUT_BYTES),
        "- ".repeat(MAX_INPUT_BYTES / 2),
        "*a ".repeat(MAX_INPUT_BYTES / 3),
    ] {
        assert_eq!(
            try_render_untrusted(&input, &options),
            Err(Refused::TooDeep),
            "{} bytes",
            input.len()
        );
    }
}

/// `_` beside punctuation can open emphasis, which the pre-parse pass does not count. The
/// walker counts it, so this shape is refused too.
#[test]
fn a_shape_the_pre_scan_misreads_is_refused_by_the_walker() {
    let input = format!(
        "{}x{}",
        "(_a ".repeat(MAX_NESTING + 1),
        " b_)".repeat(MAX_NESTING + 1)
    );
    assert_eq!(
        try_render_untrusted(&input, &Options::default()),
        Err(Refused::TooDeep)
    );
}

#[test]
fn ordinary_text_is_not_mistaken_for_deep_nesting() {
    let c_pointers = "int *p;\n".repeat(MAX_NESTING * 2);
    let documents = [
        format!("```c\n{c_pointers}```"),
        format!("~~~\n{}~~~", "> > >\n".repeat(MAX_NESTING)),
        "    int *p;\n".repeat(MAX_NESTING * 2),
        "*a* ".repeat(MAX_NESTING * 2),
        "**a** ".repeat(MAX_NESTING * 2),
        "~~a~~ ".repeat(MAX_NESTING * 2),
        "snake_case_name ".repeat(MAX_NESTING * 2),
        "2*3 ".repeat(MAX_NESTING * 2),
        "- a\n".repeat(MAX_NESTING * 2),
        "> a\n".repeat(MAX_NESTING * 2),
        format!("{}\n\n", "*a").repeat(MAX_NESTING * 2),
        "[".repeat(MAX_NESTING * 2),
        "![a](/b)".repeat(MAX_NESTING * 2),
    ];
    for document in documents {
        assert!(
            try_render_untrusted(&document, &Options::default()).is_ok(),
            "{document:?}"
        );
    }
}

#[test]
fn hostile_brackets_at_the_size_limit_render_well_formed() {
    for input in [
        "[".repeat(MAX_INPUT_BYTES),
        "![".repeat(MAX_INPUT_BYTES / 2),
    ] {
        let output = render(&input);
        assert!(output.starts_with("<p>"), "{}", &output[..20]);
        assert_well_formed(&output);
    }
}

#[test]
fn a_host_rendering_to_a_page_gets_a_notice_for_a_refused_input() {
    assert_eq!(
        render(&nested_quotes(MAX_NESTING + 1)),
        "<p>This text was not rendered: the text nests deeper than 32 levels.</p>"
    );
    assert_eq!(
        render(&"a".repeat(MAX_INPUT_BYTES + 1)),
        "<p>This text was not rendered: the text is 1048577 bytes, over the limit of 1048576 \
         bytes.</p>"
    );
}

// --- ordinary documents, byte for byte ----------------------------------------------------------

#[test]
fn an_ordinary_document() {
    let input = "\
# Release notes

The **0.11** line adds *Markdown*, with `code` and a [guide](https://e.org/g \"Guide\").
It keeps a soft break,  
and a hard one.

## Upgrade

1. Read this
2. Bump the crate

- one

- two

> Quote with a list:
>
> - item

---

```toml
pulldown-cmark = \"0.13\"
```
";
    assert_eq!(
        render(input),
        "<h3>Release notes</h3>\n\
         <p>The <strong>0.11</strong> line adds <em>Markdown</em>, with <code>code</code> and a \
         <a href=\"https://e.org/g\" rel=\"noopener noreferrer\" title=\"Guide\">guide</a>.\n\
         It keeps a soft break,<br/>\nand a hard one.</p>\n\
         <h4>Upgrade</h4>\n\
         <ol>\n<li>Read this</li>\n<li>Bump the crate</li>\n</ol>\n\
         <ul>\n<li><p>one</p>\n</li>\n<li><p>two</p>\n</li>\n</ul>\n\
         <blockquote>\n<p>Quote with a list:</p>\n<ul>\n<li>item</li>\n</ul>\n</blockquote>\n\
         <hr/>\n\
         <pre><code class=\"language-toml\">pulldown-cmark = &quot;0.13&quot;\n</code></pre>"
    );
}

#[test]
fn a_nested_list_and_a_numbered_start() {
    assert_eq!(
        render("3. c\n4. d\n   - e\n"),
        "<ol start=\"3\">\n<li>c</li>\n<li>d<ul>\n<li>e</li>\n</ul>\n</li>\n</ol>"
    );
}

#[test]
fn empty_and_blank_text_render_nothing() {
    assert_eq!(render(""), "");
    assert_eq!(render(" \n\n\t\n"), "");
}

#[test]
fn unicode_passes_through() {
    assert_eq!(render("café ✓ 日本語 𝄞"), "<p>café ✓ 日本語 𝄞</p>");
}

// --- text nodes -------------------------------------------------------------------------------

#[test]
fn map_text_nodes_visits_only_the_text_outside_links_and_code() {
    let fragment = render("x *y* [z](/a) `w`\n\n```\nv\n```");
    let mut seen = Vec::new();
    let mapped = map_text_nodes(&fragment, |text| {
        seen.push(text.to_string());
        text.to_uppercase()
    });
    assert_eq!(seen, ["x ", "y", " ", " ", "\n"]);
    assert_eq!(
        mapped,
        "<p>X <em>Y</em> <a href=\"/a\">z</a> <code>w</code></p>\n<pre><code>v\n</code></pre>"
    );
}

#[test]
fn map_text_nodes_sees_escaped_text_and_leaves_tags_and_attributes() {
    let fragment = render("[a](/x '1 > 2') 1 > 2 & <b>");
    let mapped = map_text_nodes(&fragment, |text| {
        assert!(!text.contains('<') && !text.contains('>'), "{text:?}");
        text.replace("&amp;", "AND")
    });
    assert_eq!(
        mapped,
        "<p><a href=\"/x\" title=\"1 &gt; 2\">a</a> 1 &gt; 2 AND &lt;b&gt;</p>"
    );
}

#[test]
fn map_text_nodes_with_the_identity_changes_nothing() {
    for input in HOSTILE {
        let fragment = render(input);
        assert_eq!(
            map_text_nodes(&fragment, str::to_string),
            fragment,
            "{input:?}"
        );
    }
}

// --- the resource -----------------------------------------------------------------------------

fn request(args: &[(&str, &[u8])]) -> Request {
    args.iter().fold(
        Request::new(Verb::Source, Iri::parse("urn:iki:fn:markdown").unwrap()),
        |request, (name, value)| request.with_arg(*name, ArgRef::Inline(value.to_vec())),
    )
}

fn kernel() -> Kernel {
    Kernel::new(Arc::new(ikigai_fn::space()))
}

fn issue(args: &[(&str, &[u8])]) -> ikigai_core::Result<Representation> {
    block_on(kernel().issue(request(args), &Capability::root()))
}

#[test]
fn the_resource_is_the_function() {
    for input in HOSTILE {
        let output = issue(&[("content", input.as_bytes())]).unwrap();
        assert_eq!(output.bytes, render(input).into_bytes(), "{input:?}");
    }
}

#[test]
fn the_resource_is_cacheable_html() {
    let kernel = kernel();
    let request = request(&[("content", b"# a")]);
    let output = block_on(kernel.issue(request.clone(), &Capability::root())).unwrap();
    assert_eq!(output.bytes, b"<h3>a</h3>");
    assert_eq!(output.expiry, Expiry::Never);
    assert_eq!(output.repr_type.media_type, "text/html");
    assert_eq!(output.repr_type.params.get("charset").unwrap(), "utf-8");
    assert!(kernel.is_cached(&request, &Capability::root()));
}

#[test]
fn the_resource_reads_heading_offset() {
    let output = issue(&[("content", b"# a"), ("heading_offset", b"0")]).unwrap();
    assert_eq!(output.bytes, b"<h1>a</h1>");
    let output = issue(&[("content", b"# a"), ("heading_offset", b"200")]).unwrap();
    assert_eq!(output.bytes, b"<h6>a</h6>");
}

#[test]
fn the_resource_refuses_a_heading_offset_that_is_not_a_small_number() {
    for bad in [&b"-1"[..], b"two", b"256", b"", b"1.5", &[0xff]] {
        assert!(
            matches!(
                issue(&[("content", b"x"), ("heading_offset", bad)]),
                Err(Error::InvalidArgument { name, .. }) if name == "heading_offset"
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn the_resource_answers_missing_and_non_utf8_content_with_typed_errors() {
    assert!(matches!(
        issue(&[]),
        Err(Error::MissingArgument(name)) if name == "content"
    ));
    assert!(matches!(
        issue(&[("content", &[0xff])]),
        Err(Error::InvalidArgument { name, .. }) if name == "content"
    ));
}

#[test]
fn the_resource_refuses_past_its_bounds_as_over_budget() {
    let deep = nested_quotes(MAX_NESTING + 1);
    let huge = "a".repeat(MAX_INPUT_BYTES + 1);
    for (input, words) in [
        (&deep, "markdown: the text nests deeper than 32 levels"),
        (
            &huge,
            "markdown: the text is 1048577 bytes, over the limit of 1048576 bytes",
        ),
    ] {
        match issue(&[("content", input.as_bytes())]) {
            Err(Error::OverBudget { detail }) => assert_eq!(detail, words),
            other => panic!("expected OverBudget, got {other:?}"),
        }
    }
}

#[test]
fn a_template_can_render_a_referenced_body() {
    let fixed = |text: &'static str| {
        FnEndpoint::new("fixed", move |_: &Invocation<'_>| {
            Ok(
                Representation::new(ReprType::new("text/plain"), text.as_bytes().to_vec())
                    .cacheable(),
            )
        })
    };
    let space = ikigai_fn::space()
        .bind(Exact::new("urn:test:body"), fixed("# <b>Hi</b>"))
        .bind(
            Exact::new("urn:test:page"),
            fixed("<main>$r{urn:iki:fn:markdown?content=<urn:test:body>}</main>"),
        );
    let kernel = Kernel::new(Arc::new(space));
    let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:compose").unwrap())
        .with_arg("src", ArgRef::Inline(b"urn:test:page".to_vec()));
    let output = block_on(kernel.issue(request, &Capability::root())).unwrap();
    assert_eq!(
        String::from_utf8(output.bytes.to_vec()).unwrap(),
        "<main><h3>&lt;b&gt;Hi&lt;/b&gt;</h3></main>"
    );
}
