# Accessibility

What `ikigai-fn` emits that people read, the rules that output follows, how it is tested, and
how to report a barrier. It is a library of function endpoints: it fills templates other people
write, and gives every HTML door one way to escape text and check links, but it writes no page of
its own. It is not a conformance claim. Adapted from the [ACCESSIBILITY.md
template](https://github.com/mgifford/ACCESSIBILITY.md) (MIT).

| Field | Value |
|---|---|
| Target | WCAG 2.2 Level AA, for any page built with this output; this repository writes no page itself |
| Status | Evaluation in progress. No conformance claim. |
| Owner | the maintainers of this repository |
| Report a barrier | [a GitHub issue](https://github.com/ikigai-rs/ikigai-fn/issues/new?labels=accessibility) with the `accessibility` label; anything security-sensitive through [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-fn/security/advisories/new) |
| Last reviewed | 2026-10-31 |

## What it emits that people read

- **Filled templates** (`src/lib.rs`): `compose`, `compose_over`, `each` and `conditional`
  splice resources into a template and answer the result. The markup (its `lang`, headings,
  landmarks, labels, colors) is the template author's; this crate adds only the spliced values,
  escaped by `$h` and `$j`, or as they are by `$a` and `$r`, and `each`'s `separator`, escaped.
- **Escaped text and checked links** (`src/html.rs`, `ikigai_fn::html`): `html_text` and `attr`
  for element text and attributes, `url_segment` and `url_reserved` for names, `safe_href` for
  a link target from data (`None` means render the label as text, not a link), and `json_attr`
  for `hx-vals`. Every HTML door that uses them gets the same output.
- **The text face of HTML** (`to_text`, served as `urn:iki:fn:html-to-text`): the words a page
  shows, as one line of plain text.
- **Plain-text answers** of the small functions (`toUpper`, `reverseList`, `split`, `wrap`,
  `greet`, `echo`).
- **Error messages** (`src/lib.rs`, `src/prefix.rs`): a marker that does not parse or does not
  resolve, a template or source that is not text, an `each` source that is not a flat JSON array,
  a `conditional` answer that is not a boolean, a refused prefix table.

Not this repository's: every template's markup, and the page a host serves it in.

## The rules that output follows

Verified by reading the source on 2026-10-31:

- Text from data reaches a page escaped (`&<>"'`), safe in element text and in either-quoted
  attributes; a C0 control other than tab, line feed and carriage return becomes U+FFFD rather
  than reaching the page raw.
- A link target from data is kept only for `http://`, `https://` or a single-`/` path; anything else
  is rendered as its label in text, so the words stay even when the link is refused.
- Element data in `each` reaches the output only escaped; a template that splices it as it is is
  refused, naming the marker.
- An error names what it is about: the endpoint (`compose:`, `each:`, `conditional:`), and the
  marker, argument or IRI.
- No color, terminal escape or markup of this crate's own is written.

## How it is tested

- `tests/html_vectors.rs` runs `tests/vectors/html_vectors.json` against `html_text`, `attr`,
  `compose`'s `$h`, `url_segment`, `safe_href` and `json_attr`
  (`html_text_and_attr_match_the_escape_vectors`, `compose_h_escapes_by_the_same_vectors`,
  `safe_href_matches_the_vectors`, `json_attr_matches_the_vectors` and others).
- `tests/html_to_text.rs` pins what `to_text` keeps and drops (`markup_is_not_text`,
  `blocks_separate_words_and_inline_elements_do_not`, `code_and_comments_are_not_text` and
  others).
- `tests/escape.rs` pins that escaped template text renders exactly as written.
- `tests/each.rs` pins that element data reaches the output only escaped
  (`element_data_is_never_spliced_as_it_is`, `the_separator_goes_between_fills_escaped`).
- **No automated accessibility checker, browser or assistive technology has been used on this
  output.**

## Known limitations

Found by reading the source on 2026-10-31, not by testing:

- `to_text` drops every attribute value, `alt` included, so an image's text alternative is not
  in the text face: HTML whose only content is an image becomes empty text. `markup_is_not_text`
  pins this as intended.
- Whether a filled template is accessible depends entirely on the template; this crate checks
  nothing about the markup it fills (no `lang`, heading or label check).
- `conditional`'s refusal prints the answer it could not read in Rust's debug form (`{:?}`).
- Messages wrap names in backticks (`` `name` ``), Markdown punctuation that a screen reader may
  read aloud in a plain-text face.

## Contributing

Read this file before changing any output listed above: an escaping function, `to_text`, the
splicing rules or an error message. A change to escaping changes `tests/vectors/html_vectors.json`
on purpose, and the ports that copy it follow. Keep meaning in words, and say in the pull request
what was tested and what was not.

## Reporting a barrier

[Open an issue](https://github.com/ikigai-rs/ikigai-fn/issues/new?labels=accessibility) with the
`accessibility` label (the link sets it when GitHub lets you add labels; otherwise a maintainer
adds it): what you were doing, what happened, and your browser or terminal and any assistive
technology, if you are comfortable sharing them. Anything security-sensitive goes through
[private vulnerability reporting](https://github.com/ikigai-rs/ikigai-fn/security/advisories/new).
**We acknowledge a report within 7 days**, and promise nothing further.

## Change history

| Date | Change |
|---|---|
| 2026-10-31 | First version. |
