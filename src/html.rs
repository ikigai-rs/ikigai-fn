//! Escaping and URL rules for HTML built from data: ONE implementation for every door.
//!
//! An HTML page built by string concatenation has three places a value can break out of:
//! element text, an attribute value, and a URL. Each gets one function here, and each is
//! pinned by the shared vectors in `tests/vectors/html_vectors.json`, which a Python, Deno or
//! XSLT port consumes verbatim so every language gives the same answer (ledger #1813).
//!
//! | Where the value goes | Function |
//! |---|---|
//! | element text: `<p>{}</p>` | [`html_text`] |
//! | a quoted attribute: `title="{}"` or `title='{}'` | [`attr`] |
//! | one path segment or query value: `/item/{}?q={}` | [`url_segment`] |
//! | a whole link target from data: `href="{}"` | [`safe_href`], then [`attr`] |
//! | htmx's `hx-vals='{}'` | [`json_attr`] |
//!
//! Nothing here makes an UNQUOTED attribute, an event-handler attribute (`onclick=`), or
//! the inside of `<script>` or `<style>` safe: those are code, and no escape turns data into
//! safe code. `compose`'s `$h{…}` marker escapes with [`html_text`], so a template and a
//! hand-built page agree.

/// `text`, safe as HTML (or XML) element text: `&`, `<`, `>`, `"` and `'` become `&amp;`,
/// `&lt;`, `&gt;`, `&quot;` and `&#39;`, and a C0 control character other than tab, line
/// feed and carriage return becomes U+FFFD.
///
/// The quotes are escaped too, so the same text is safe in an attribute ([`attr`] is this
/// function under the name that says where it goes). `'` is `&#39;`, not `&apos;`, which
/// HTML 4 does not define. The controls are replaced, not escaped, because XML 1.0 cannot
/// carry them even as character references: a page that is later read as XHTML, or run
/// through XSLT, fails to parse on one. Every other character, including any Unicode
/// outside ASCII, passes through unchanged. Not idempotent, by design: `&amp;` in the
/// input is text that says `&amp;`.
///
/// ```
/// use ikigai_fn::html::html_text;
/// assert_eq!(html_text("<b>Tom & Jerry's</b>"), "&lt;b&gt;Tom &amp; Jerry&#39;s&lt;/b&gt;");
/// assert_eq!(html_text("say \"hi\""), "say &quot;hi&quot;");
/// assert_eq!(html_text("a\u{1}b\tc"), "a\u{FFFD}b\tc");
/// assert_eq!(html_text("café ✓"), "café ✓");
/// ```
pub fn html_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}

/// `value`, safe inside a QUOTED attribute value, whichever quote the markup uses:
/// `name="{}"` and `name='{}'` are both safe, because both quotes are escaped. The same
/// output as [`html_text`], under the name that says where it goes.
///
/// ⚠ An attribute must still be quoted: an unquoted value ends at a space, and nothing
/// escapes a space. And escaping is not a URL check: a value for `href` or `src` that came
/// from data goes through [`safe_href`] first, since `javascript:alert(1)` needs no escaping
/// to run.
///
/// ```
/// use ikigai_fn::html::attr;
/// let title = attr(r#"5" & 6'"#);
/// assert_eq!(title, "5&quot; &amp; 6&#39;");
/// assert_eq!(format!("<a title=\"{title}\">"), "<a title=\"5&quot; &amp; 6&#39;\">");
/// assert_eq!(format!("<a title='{title}'>"), "<a title='5&quot; &amp; 6&#39;'>");
/// ```
pub fn attr(value: &str) -> String {
    html_text(value)
}

/// `value` percent-encoded as ONE path segment or ONE query value: every byte outside
/// RFC 3986's unreserved set (`A-Z a-z 0-9 - . _ ~`) becomes `%XX`, uppercase hex, over the
/// value's UTF-8 bytes.
///
/// So `/`, `?`, `#`, `&`, `=` and `+` cannot change where the value ends, and `%` is encoded
/// too: `%41` in the input is text that says `%41`, not an `A`. A space is `%20`, which a
/// form decoder reads as a space as well. This is RFC 6570's simple-variable expansion, the
/// rule `compose` uses for an argument spliced into an IRI. The output is ASCII, so it is
/// also safe in an attribute without further escaping.
///
/// ```
/// use ikigai_fn::html::url_segment;
/// assert_eq!(url_segment("what? #1 / 100%"), "what%3F%20%231%20%2F%20100%25");
/// assert_eq!(url_segment("café"), "caf%C3%A9");
/// assert_eq!(url_segment("a+b=c&d"), "a%2Bb%3Dc%26d");
/// assert_eq!(url_segment("A-z_0.9~"), "A-z_0.9~");
/// ```
pub fn url_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    push_url_segment(value, &mut out);
    out
}

/// Append [`url_segment`]`(value)` to `out`.
pub(crate) fn push_url_segment(value: &str, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0xF)]));
        }
    }
}

/// The link target to put in `href` when `href` came from data, or `None` when it must not
/// be a link (render the label as text instead).
///
/// The rule, after cms-web's `safe-link` (ledger #1793): past any leading and trailing
/// space or C0 control character (which a browser strips too), the target must start with
/// `http://` or `https://`, in any ASCII case, or with a single `/` (a path on the page's
/// own origin). `//host` and `/\host` name another host and are refused, and so is every
/// other scheme (`javascript:`, `data:`, `vbscript:`, `file:`, `mailto:`) and every target
/// with no scheme at all. A target that still holds a C0 control or DEL after trimming is
/// refused too: a browser deletes a tab, line feed or carriage return anywhere in a URL
/// before reading it, so `java\tscript:` runs as `javascript:` and `/\t/host` opens
/// another host, and the string the browser reads would not be the one checked.
///
/// The answer is the TRIMMED target, still unescaped: put it in a quoted attribute through
/// [`attr`], since it may hold `&` or `"`.
///
/// ```
/// use ikigai_fn::html::{attr, safe_href};
/// assert_eq!(safe_href("https://example.org/a?b=1"), Some("https://example.org/a?b=1"));
/// assert_eq!(safe_href("HTTP://EXAMPLE.ORG/"), Some("HTTP://EXAMPLE.ORG/"));
/// assert_eq!(safe_href(" /r/urn:cms:tags\n"), Some("/r/urn:cms:tags"));
/// assert_eq!(safe_href("javascript:alert(1)"), None);
/// assert_eq!(safe_href("java\tscript:alert(1)"), None);
/// assert_eq!(safe_href("//evil.example/"), None);
/// assert_eq!(safe_href("/\\evil.example/"), None);
/// assert_eq!(safe_href("data:text/html,x"), None);
///
/// let link = |href: &str, label: &str| match safe_href(href) {
///     Some(h) => format!("<a href=\"{}\">{}</a>", attr(h), ikigai_fn::html::html_text(label)),
///     None => format!("<span>{}</span>", ikigai_fn::html::html_text(label)),
/// };
/// assert_eq!(link("/a?x=1&y=2", "A"), "<a href=\"/a?x=1&amp;y=2\">A</a>");
/// assert_eq!(link("vbscript:msgbox(1)", "B"), "<span>B</span>");
/// ```
pub fn safe_href(href: &str) -> Option<&str> {
    let href = href.trim_matches(|c: char| c <= ' ');
    if href.chars().any(|c| c < ' ' || c == '\u{7F}') {
        return None;
    }
    let starts = |prefix: &str| {
        href.get(..prefix.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(prefix))
    };
    let same_origin_path =
        href.starts_with('/') && !href.starts_with("//") && !href.starts_with("/\\");
    (starts("https://") || starts("http://") || same_origin_path).then_some(href)
}

/// `pairs` as a JSON object of strings, escaped for a quoted attribute: the value of
/// htmx's `hx-vals`, built on the server without concatenating JSON by hand.
///
/// The object's members are in the order `pairs` gives them, keys and values as JSON
/// strings (`"` and `\` escaped, a control character as `\n`, `\r`, `\t`, `\b`, `\f` or
/// `\u00XX`), with no whitespace. The whole object then goes through [`attr`], so it is
/// safe inside either quote; the browser decodes the attribute and htmx reads the JSON.
/// A duplicate key is written as given, and `JSON.parse` keeps the last one.
///
/// ```
/// use ikigai_fn::html::json_attr;
/// assert_eq!(json_attr([("q", "a")]), "{&quot;q&quot;:&quot;a&quot;}");
/// assert_eq!(
///     json_attr([("tag", "c++"), ("note", "say \"hi\"\n")]),
///     "{&quot;tag&quot;:&quot;c++&quot;,&quot;note&quot;:&quot;say \\&quot;hi\\&quot;\\n&quot;}"
/// );
/// assert_eq!(json_attr(Vec::<(String, String)>::new()), "{}");
/// ```
pub fn json_attr<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> String
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut json = String::from("{");
    for (i, (key, value)) in pairs.into_iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        push_json_string(key.as_ref(), &mut json);
        json.push(':');
        push_json_string(value.as_ref(), &mut json);
    }
    json.push('}');
    attr(&json)
}

/// Append `s` to `out` as a JSON string literal (RFC 8259).
fn push_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
