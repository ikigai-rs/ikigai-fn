//! Markdown from people the host does not trust, rendered as an XHTML fragment: ONE walker
//! for every door. [`render_untrusted`] serves a host that renders without a kernel round
//! trip; [`crate::markdown()`] serves the same walker at `urn:iki:fn:markdown`.
//!
//! The dialect is CommonMark plus tables, strikethrough and task lists. The fragment is built
//! element by element from `pulldown-cmark` events, and every piece of text or attribute goes
//! through [`crate::html`]:
//!
//! | Input | Output |
//! |---|---|
//! | an HTML block or inline tag | escaped text, never markup |
//! | a link to `http`, `https`, `mailto`, `/…`, `?…` or `#…` | `<a>`, with `rel="noopener noreferrer"` unless it stays on this origin |
//! | a link to anything else | its text only |
//! | an image | a link to its URL under the rules above, labeled with the alt text; never an `<img>` |
//! | `#` | `<h{1 + heading_offset}>`, capped at `<h6>` |
//! | a fenced code block | `<pre><code>`, with `class="language-…"` only for a name in `[A-Za-z0-9_+-]+` |
//!
//! The fragment is well-formed XML, so an XSLT pipeline can copy it through, and the same input
//! and options always give the same bytes.
//!
//! ## Bounds
//!
//! An input over [`MAX_INPUT_BYTES`] or nested past [`MAX_NESTING`] levels is refused before it
//! is parsed, by one pass that neither recurses nor allocates (the ecosystem's rule for a
//! decoder over request input, ledger #1326). That pass cannot be exact without being a parser,
//! so the walker counts the same levels again and refuses the shapes the pass misread. Either
//! way nothing partial comes back.
//!
//! ## Ledger and IRI links are the host's
//!
//! This module links nothing that the author did not write as a link. A host with its own
//! linkify runs it over the fragment's text nodes with [`map_text_nodes`], which is how gonk
//! applies its linkify.

use pulldown_cmark::{
    Alignment, CodeBlockKind, CowStr, Event, LinkType, Options as ParseOptions, Parser, Tag, TagEnd,
};

use crate::html;

/// The largest input accepted, in bytes (1 MiB). Past it [`try_render_untrusted`] refuses
/// without parsing.
pub const MAX_INPUT_BYTES: usize = 1 << 20;

/// The deepest nesting accepted, in levels. A block quote, a list and each of emphasis, strong
/// emphasis, strikethrough and a link is one level.
pub const MAX_NESTING: usize = 32;

/// How a body is placed in its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Added to every heading's level, so a body never outranks its page's headings. The
    /// default, 2, makes `#` an `<h3>`. A level past 6 is capped at `<h6>`.
    pub heading_offset: u8,
}

impl Default for Options {
    fn default() -> Self {
        Options { heading_offset: 2 }
    }
}

/// Why an input was refused. Its message names the bound and fits after a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The input is longer than [`MAX_INPUT_BYTES`].
    TooLarge { bytes: usize },
    /// The input nests deeper than [`MAX_NESTING`].
    TooDeep,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { bytes } => write!(
                f,
                "the text is {bytes} bytes, over the limit of {MAX_INPUT_BYTES} bytes"
            ),
            Self::TooDeep => write!(f, "the text nests deeper than {MAX_NESTING} levels"),
        }
    }
}

impl std::error::Error for Refused {}

/// `text` as an XHTML fragment, or a one-paragraph notice that it was refused.
///
/// For a host that puts the answer straight on a page, where a refused body should say so and
/// the rest of the page should still render. [`try_render_untrusted`] answers the refusal as a
/// value instead.
///
/// ```
/// use ikigai_fn::markdown::{render_untrusted, Options};
/// assert_eq!(
///     render_untrusted("# Hi <b>there</b>", &Options::default()),
///     "<h3>Hi &lt;b&gt;there&lt;/b&gt;</h3>"
/// );
/// assert_eq!(
///     render_untrusted("[run](javascript:alert(1))", &Options::default()),
///     "<p>run</p>"
/// );
/// ```
pub fn render_untrusted(text: &str, options: &Options) -> String {
    try_render_untrusted(text, options).unwrap_or_else(|refused| {
        let reason = html::html_text(&refused.to_string());
        format!("<p>This text was not rendered: {reason}.</p>")
    })
}

/// `text` as an XHTML fragment, or why it was refused.
///
/// ```
/// use ikigai_fn::markdown::{try_render_untrusted, Options, Refused, MAX_NESTING};
/// let deep = ">".repeat(MAX_NESTING + 1);
/// assert_eq!(try_render_untrusted(&deep, &Options::default()), Err(Refused::TooDeep));
/// ```
pub fn try_render_untrusted(text: &str, options: &Options) -> Result<String, Refused> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Refused::TooLarge { bytes: text.len() });
    }
    if deepest_nesting(text) > MAX_NESTING {
        return Err(Refused::TooDeep);
    }
    let dialect = ParseOptions::ENABLE_TABLES
        | ParseOptions::ENABLE_STRIKETHROUGH
        | ParseOptions::ENABLE_TASKLISTS;
    let mut walker = Walker::new(text.len(), options.heading_offset);
    for event in Parser::new_ext(text, dialect) {
        walker.event(event)?;
    }
    Ok(walker.finish())
}

/// `fragment` with each text node replaced by what `linkify` answers for it.
///
/// `fragment` must be the output of this module, where `<` and `>` only ever delimit tags.
/// `linkify` sees a text node as it stands in the fragment, so still escaped (`&amp;`), and its
/// answer is spliced as markup: anything it adds from data it must escape itself. Text inside
/// `<a>` and `<code>` is left alone, since a link cannot hold a link and code is not prose.
///
/// ```
/// use ikigai_fn::markdown::{map_text_nodes, render_untrusted, Options};
/// let page = render_untrusted("see #12 and `#13`", &Options::default());
/// let linked = map_text_nodes(&page, |text| text.replace("#12", "<a href=\"/item/12\">#12</a>"));
/// assert_eq!(linked, "<p>see <a href=\"/item/12\">#12</a> and <code>#13</code></p>");
/// ```
pub fn map_text_nodes(fragment: &str, mut linkify: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut protected = 0usize;
    let mut rest = fragment;
    while let Some(open) = rest.find('<') {
        let (text, tail) = rest.split_at(open);
        push_text_node(&mut out, text, protected, &mut linkify);
        let Some(close) = tail.find('>') else {
            rest = tail;
            break;
        };
        let (tag, after) = tail.split_at(close + 1);
        out.push_str(tag);
        match tag_name(tag) {
            ("a" | "code", false) => protected += 1,
            ("a" | "code", true) => protected = protected.saturating_sub(1),
            _ => {}
        }
        rest = after;
    }
    push_text_node(&mut out, rest, protected, &mut linkify);
    out
}

fn push_text_node(
    out: &mut String,
    text: &str,
    protected: usize,
    linkify: &mut impl FnMut(&str) -> String,
) {
    if protected == 0 && !text.is_empty() {
        out.push_str(&linkify(text));
    } else {
        out.push_str(text);
    }
}

/// The name of `tag` (`<a href="…">`, `</a>`), and whether it is a closing tag.
fn tag_name(tag: &str) -> (&str, bool) {
    let inner = tag.trim_start_matches('<');
    let (inner, closing) = match inner.strip_prefix('/') {
        Some(inner) => (inner, true),
        None => (inner, false),
    };
    let end = inner
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(inner.len());
    (&inner[..end], closing)
}

// --- the pre-parse bound ----------------------------------------------------------------------

/// An estimate of how deeply `text` nests, from one pass over its lines.
///
/// A line's depth is its block markers (`>`, a list marker), plus a level per two columns of
/// indentation before them, plus the emphasis delimiters still open in its paragraph. Fenced
/// and indented code nests nothing, so `*p` in C does not count.
fn deepest_nesting(text: &str) -> usize {
    let mut deepest = 0;
    let mut open = OpenDelimiters::default();
    let mut fence = None;
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let prefix = block_prefix(line);
        if let Some(marker) = fence {
            if fence_marker(prefix.rest) == Some(marker) {
                fence = None;
            }
            continue;
        }
        if prefix.rest.is_empty() {
            open = OpenDelimiters::default();
        }
        if let Some(marker) = fence_marker(prefix.rest) {
            fence = Some(marker);
        } else if prefix.markers > 0 || prefix.indent < 4 {
            open.scan(prefix.rest);
            let containers = match prefix.markers {
                0 => 0,
                markers => markers + prefix.indent / 2,
            };
            deepest = deepest.max(containers + open.peak);
        }
    }
    deepest
}

struct BlockPrefix<'a> {
    /// Block quote and list markers at the start of the line.
    markers: usize,
    /// Columns of indentation before the first marker.
    indent: usize,
    /// The line after them.
    rest: &'a str,
}

fn block_prefix(line: &str) -> BlockPrefix<'_> {
    let start = line.trim_start_matches([' ', '\t']);
    let indent = line[..line.len() - start.len()]
        .bytes()
        .map(|b| if b == b'\t' { 4 } else { 1 })
        .sum();
    let mut markers = 0;
    let mut rest = start;
    while let Some(after) = rest.strip_prefix('>').or_else(|| list_marker(rest)) {
        markers += 1;
        rest = after.trim_start_matches([' ', '\t']);
    }
    BlockPrefix {
        markers,
        indent,
        rest,
    }
}

/// `line` after a list marker it starts with: a bullet, or up to nine digits and `.` or `)`,
/// followed by white space or the end of the line.
fn list_marker(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    let after = match digits {
        0 => line.strip_prefix(['-', '+', '*'])?,
        1..=9 => line[digits..].strip_prefix(['.', ')'])?,
        _ => return None,
    };
    (after.is_empty() || after.starts_with([' ', '\t'])).then_some(after)
}

/// The fence character if `rest` starts with a code fence.
fn fence_marker(rest: &str) -> Option<u8> {
    ["```", "~~~"]
        .into_iter()
        .find(|fence| rest.starts_with(fence))
        .map(|fence| fence.as_bytes()[0])
}

/// The `*`, `_` and `~` delimiter characters open so far in a paragraph.
#[derive(Default)]
struct OpenDelimiters {
    open: [usize; 3],
    peak: usize,
}

impl OpenDelimiters {
    fn scan(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let start = i;
            let c = bytes[i];
            let slot = match c {
                b'*' => 0,
                b'_' => 1,
                b'~' => 2,
                _ => {
                    i += 1;
                    continue;
                }
            };
            while bytes.get(i) == Some(&c) {
                i += 1;
            }
            let run = i - start;
            let can_open = bytes.get(i).is_some_and(|b| !b.is_ascii_whitespace());
            let can_close = start > 0 && !bytes[start - 1].is_ascii_whitespace();
            let closes = match (can_open, can_close) {
                (true, false) => false,
                (false, true) => true,
                // Inside a word, or beside punctuation. `_` neither opens nor closes in a word,
                // and the others do whichever they can.
                (true, true) if c != b'_' => self.open[slot] > 0,
                _ => continue,
            };
            if closes {
                self.open[slot] = self.open[slot].saturating_sub(run);
            } else {
                self.open[slot] += run;
            }
            self.peak = self.peak.max(self.open.iter().sum());
        }
    }
}

// --- the walker -------------------------------------------------------------------------------

/// The `<a>` currently open, for its closing tag and its label.
struct OpenLink {
    /// The target, as the label when the author gave no text.
    href: String,
    /// `out.len()` just after the opening tag.
    body_start: usize,
    /// Whether an image opened it, so that image's end closes it.
    from_image: bool,
}

struct Walker {
    out: String,
    heading_offset: u8,
    /// Open levels, as [`MAX_NESTING`] counts them.
    depth: usize,
    link: Option<OpenLink>,
    /// More than zero inside an image's alt text, which renders as plain text.
    image_depth: usize,
    alignments: Vec<Alignment>,
    cell: usize,
    in_table_head: bool,
    in_table_body: bool,
}

impl Walker {
    fn new(input_len: usize, heading_offset: u8) -> Self {
        Walker {
            out: String::with_capacity(input_len + input_len / 2),
            heading_offset,
            depth: 0,
            link: None,
            image_depth: 0,
            alignments: Vec::new(),
            cell: 0,
            in_table_head: false,
            in_table_body: false,
        }
    }

    fn event(&mut self, event: Event<'_>) -> Result<(), Refused> {
        if self.image_depth > 0 {
            self.alt_text_event(event);
            return Ok(());
        }
        match event {
            Event::Start(tag) => self.start(tag)?,
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => self.text(&text),
            Event::Code(text) => {
                self.out.push_str("<code>");
                self.text(&text);
                self.out.push_str("</code>");
            }
            Event::SoftBreak => self.out.push('\n'),
            Event::HardBreak => self.out.push_str("<br/>\n"),
            Event::Rule => self.out.push_str("<hr/>\n"),
            Event::TaskListMarker(done) => self.out.push_str(if done {
                "<input type=\"checkbox\" disabled=\"disabled\" checked=\"checked\" aria-label=\"task\"/> "
            } else {
                "<input type=\"checkbox\" disabled=\"disabled\" aria-label=\"task\"/> "
            }),
            // Only with parser options this module does not enable; the words still render.
            Event::InlineMath(text) | Event::DisplayMath(text) | Event::FootnoteReference(text) => {
                self.text(&text)
            }
        }
        Ok(())
    }

    /// Inside an image only its words are kept, so an `<a>` never lands inside the image's `<a>`.
    fn alt_text_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Image { .. }) => self.image_depth += 1,
            Event::End(TagEnd::Image) => {
                self.image_depth -= 1;
                if self.image_depth == 0 && self.link.as_ref().is_some_and(|l| l.from_image) {
                    self.close_link();
                }
            }
            Event::Text(text) | Event::Code(text) | Event::InlineHtml(text) => self.text(&text),
            Event::SoftBreak | Event::HardBreak => self.out.push(' '),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) -> Result<(), Refused> {
        if matches!(
            tag,
            Tag::BlockQuote(_)
                | Tag::List(_)
                | Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Link { .. }
        ) {
            self.depth += 1;
            if self.depth > MAX_NESTING {
                return Err(Refused::TooDeep);
            }
        }
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => self.out.push_str("<p>"),
            Tag::Heading { level, .. } => {
                let level = self.heading_level(level as usize);
                self.out.push_str(&format!("<h{level}>"));
            }
            Tag::BlockQuote(_) => self.out.push_str("<blockquote>\n"),
            Tag::CodeBlock(kind) => self.open_code_block(kind),
            Tag::List(None) => self.out.push_str("<ul>\n"),
            Tag::List(Some(1)) => self.out.push_str("<ol>\n"),
            Tag::List(Some(start)) => self.out.push_str(&format!("<ol start=\"{start}\">\n")),
            Tag::Item => self.out.push_str("<li>"),
            Tag::Table(alignments) => {
                self.alignments = alignments;
                self.out.push_str("<table>\n");
            }
            Tag::TableHead => {
                self.in_table_head = true;
                self.cell = 0;
                self.out.push_str("<thead>\n<tr>");
            }
            Tag::TableRow => {
                if !self.in_table_body {
                    self.in_table_body = true;
                    self.out.push_str("<tbody>\n");
                }
                self.cell = 0;
                self.out.push_str("<tr>");
            }
            Tag::TableCell => self.open_table_cell(),
            Tag::Emphasis => self.out.push_str("<em>"),
            Tag::Strong => self.out.push_str("<strong>"),
            Tag::Strikethrough => self.out.push_str("<del>"),
            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            } => self.open_link(link_type, &dest_url, &title, false),
            Tag::Image {
                link_type,
                dest_url,
                title,
                ..
            } => {
                self.open_link(link_type, &dest_url, &title, true);
                self.image_depth = 1;
            }
            // Only with parser options this module does not enable.
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => {}
        }
        Ok(())
    }

    fn end(&mut self, tag: TagEnd) {
        if matches!(
            tag,
            TagEnd::BlockQuote(_)
                | TagEnd::List(_)
                | TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
        ) {
            self.depth -= 1;
        }
        match tag {
            TagEnd::Paragraph => self.out.push_str("</p>\n"),
            TagEnd::HtmlBlock => {
                self.out.truncate(self.out.trim_end_matches('\n').len());
                self.out.push_str("</p>\n");
            }
            TagEnd::Heading(level) => {
                let level = self.heading_level(level as usize);
                self.out.push_str(&format!("</h{level}>\n"));
            }
            TagEnd::BlockQuote(_) => self.out.push_str("</blockquote>\n"),
            TagEnd::CodeBlock => self.out.push_str("</code></pre>\n"),
            TagEnd::List(true) => self.out.push_str("</ol>\n"),
            TagEnd::List(false) => self.out.push_str("</ul>\n"),
            TagEnd::Item => self.out.push_str("</li>\n"),
            TagEnd::Table => {
                if self.in_table_body {
                    self.in_table_body = false;
                    self.out.push_str("</tbody>\n");
                }
                self.out.push_str("</table>\n");
            }
            TagEnd::TableHead => {
                self.in_table_head = false;
                self.out.push_str("</tr>\n</thead>\n");
            }
            TagEnd::TableRow => self.out.push_str("</tr>\n"),
            TagEnd::TableCell => {
                self.cell += 1;
                self.out
                    .push_str(if self.in_table_head { "</th>" } else { "</td>" });
            }
            TagEnd::Emphasis => self.out.push_str("</em>"),
            TagEnd::Strong => self.out.push_str("</strong>"),
            TagEnd::Strikethrough => self.out.push_str("</del>"),
            TagEnd::Link => self.close_link(),
            // An image's end is handled with its alt text.
            TagEnd::Image
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn text(&mut self, text: &str) {
        self.out.push_str(&html::html_text(text));
    }

    fn heading_level(&self, level: usize) -> usize {
        (level + usize::from(self.heading_offset)).min(6)
    }

    fn open_code_block(&mut self, kind: CodeBlockKind<'_>) {
        let language = match &kind {
            CodeBlockKind::Fenced(info) => info.split_ascii_whitespace().next(),
            CodeBlockKind::Indented => None,
        };
        let language = language.filter(|name| {
            name.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'-'))
        });
        match language {
            Some(name) => self
                .out
                .push_str(&format!("<pre><code class=\"language-{name}\">")),
            None => self.out.push_str("<pre><code>"),
        }
    }

    fn open_table_cell(&mut self) {
        let align = match self.alignments.get(self.cell) {
            Some(Alignment::Left) => " style=\"text-align: left\"",
            Some(Alignment::Center) => " style=\"text-align: center\"",
            Some(Alignment::Right) => " style=\"text-align: right\"",
            Some(Alignment::None) | None => "",
        };
        // `scope` ties each data cell to its header for a screen reader.
        let open = if self.in_table_head {
            format!("<th scope=\"col\"{align}>")
        } else {
            format!("<td{align}>")
        };
        self.out.push_str(&open);
    }

    fn open_link(
        &mut self,
        link_type: LinkType,
        dest: &CowStr<'_>,
        title: &CowStr<'_>,
        from_image: bool,
    ) {
        // A linked image is already inside its link's `<a>`.
        let Some(href) = link_target(link_type, dest).filter(|_| self.link.is_none()) else {
            return;
        };
        self.out.push_str("<a href=\"");
        self.out.push_str(&html::attr(&href));
        self.out.push('"');
        if !href.starts_with(['/', '?', '#']) {
            self.out.push_str(" rel=\"noopener noreferrer\"");
        }
        if !title.is_empty() {
            self.out.push_str(" title=\"");
            self.out.push_str(&html::attr(title));
            self.out.push('"');
        }
        self.out.push('>');
        self.link = Some(OpenLink {
            href,
            body_start: self.out.len(),
            from_image,
        });
    }

    fn close_link(&mut self) {
        let Some(link) = self.link.take() else {
            return;
        };
        // A link with no words has no accessible name: its target is the next best label.
        if self.out.len() == link.body_start {
            self.text(&link.href);
        }
        self.out.push_str("</a>");
    }

    fn finish(self) -> String {
        // XML has no way to write these two, even as references. `html_text` leaves them for
        // its shared vectors, and a numeric reference in the Markdown can produce them.
        self.out
            .trim_end_matches('\n')
            .replace(['\u{FFFE}', '\u{FFFF}'], "\u{FFFD}")
    }
}

/// The link target to use for `dest`, or `None` to render the words without a link.
///
/// [`html::safe_href`] is the authority, but its shared vectors refuse `mailto:`, `?…` and
/// `#…`, which a Markdown body may use. Behind a `/` those read as a path, which leaves only
/// its control-character rule to decide.
fn link_target(link_type: LinkType, dest: &str) -> Option<String> {
    let email;
    let dest = if link_type == LinkType::Email {
        email = format!("mailto:{dest}");
        email.as_str()
    } else {
        dest
    };
    let trimmed = dest.trim_matches(|c: char| c <= ' ');
    let is_mailto = trimmed
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("mailto:"));
    if is_mailto || trimmed.starts_with(['?', '#']) {
        html::safe_href(&format!("/{trimmed}")).map(|_| trimmed.to_string())
    } else {
        html::safe_href(dest).map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The refusal these inputs get in `try_render_untrusted` comes from the pass that runs
    /// before the parser, not from the walker.
    #[test]
    fn the_pre_parse_pass_sees_each_kind_of_nesting() {
        let levels = MAX_NESTING + 1;
        assert_eq!(deepest_nesting(&">".repeat(levels)), levels);
        assert_eq!(deepest_nesting(&"> ".repeat(levels)), levels);
        assert_eq!(deepest_nesting(&"- ".repeat(levels)), levels);
        assert_eq!(deepest_nesting(&"1. ".repeat(levels)), levels);
        assert_eq!(deepest_nesting(&"*a ".repeat(levels)), levels);
        assert_eq!(deepest_nesting(&"~a ".repeat(levels)), levels);
        assert_eq!(
            deepest_nesting(&format!("{}- x", "  ".repeat(levels - 1))),
            levels
        );
        assert_eq!(
            deepest_nesting(&format!("{}- x", "\t".repeat(levels))),
            levels * 2 + 1
        );
    }

    #[test]
    fn the_pre_parse_pass_counts_a_block_marker_with_the_emphasis_inside_it() {
        assert_eq!(deepest_nesting("> *a _b"), 3);
    }

    #[test]
    fn the_pre_parse_pass_closes_what_a_paragraph_opened() {
        assert_eq!(deepest_nesting("*a* *b* _c_ **d** ~~e~~"), 2);
        assert_eq!(deepest_nesting("*a\n\n*b\n\n*c"), 1);
    }

    #[test]
    fn the_pre_parse_pass_skips_code() {
        assert_eq!(deepest_nesting("```\n*a *b > > >\n```\n"), 0);
        assert_eq!(deepest_nesting("~~~c\n- - -\n~~~\n*a"), 1);
        assert_eq!(deepest_nesting("    *a *b"), 0);
    }

    #[test]
    fn crlf_line_endings_read_like_lf() {
        assert_eq!(deepest_nesting("-\r\n  -\r\n"), 2);
        assert_eq!(deepest_nesting("*a\r\n\r\n*b"), 1);
    }
}
