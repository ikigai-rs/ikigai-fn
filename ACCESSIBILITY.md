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
| Last reviewed | 2026-11-13 |

## What it emits that people read

- **Filled templates** (`src/lib.rs`): `compose`, `compose_over`, `each` and `conditional`
  splice resources into a template and answer the result. The markup (its `lang`, headings,
  landmarks, labels, colors) is the template author's; this crate adds only the spliced values,
  escaped by `$h` and `$j`, or as they are by `$a` and `$r`, and `each`'s `separator`, escaped.
- **Escaped text and checked links** (`src/html.rs`, `ikigai_fn::html`): `html_text` and `attr`
  for element text and attributes, `url_segment` and `url_reserved` for names, `safe_href` for
  a link target from data (`None` means render the label as text, not a link), and `json_attr`
  for `hx-vals`. Every HTML door that uses them gets the same output.
- **Rendered Markdown** (`src/markdown.rs`, served as `urn:iki:fn:markdown`): a body someone
  else wrote, as an XHTML fragment. Its words, headings, lists and tables are the author's; the
  element structure, the task-box label and the notice for a refused body are this crate's.
- **The text face of HTML** (`to_text`, served as `urn:iki:fn:html-to-text`): the words a page
  shows, as one line of plain text.
- **Plain-text answers** of the small functions (`toUpper`, `reverseList`, `split`, `wrap`,
  `greet`, `echo`, `digest`).
- **Error messages** (`src/lib.rs`, `src/prefix.rs`): a marker that does not parse or does not
  resolve, a template or source that is not text, an `each` source that is not a flat JSON array,
  a `conditional` answer that is not a boolean, a refused prefix table.

Not this repository's: every template's markup, and the page a host serves it in.

## The rules that output follows

Implementation rules; current verification and gaps are recorded below:

- Text from data reaches a page escaped (`&<>"'`), safe in element text and in either-quoted
  attributes; a C0 control other than tab, line feed and carriage return becomes U+FFFD rather
  than reaching the page raw.
- A link target from data is kept only for `http://`, `https://` or a single-`/` path; anything else
  is rendered as its label in text, so the words stay even when the link is refused.
- Rendered Markdown follows the same two rules, except that a link target may also be `mailto:`,
  `?…` or `#…`. A refused link keeps its words. An image is never loaded: it is a link labeled
  with its alt text, or the alt text alone when its target is refused.
- A rendered link with no words is labeled with its target, so it always has an accessible name.
- Rendered headings start below the page's own (`#` is `<h3>` by default), so a body cannot outrank
  them. A task box is a disabled `<input>` labeled `task`. A table's column headers are
  `<th scope="col">`.
- Element data in `each` reaches the output only escaped; a template that splices it as it is is
  refused, naming the marker.
- An error names what it is about: the endpoint (`compose:`, `each:`, `conditional:`), and the
  marker, argument or IRI.
- Plain-text answers and errors should carry meaning in words. This crate also writes HTML
  fragments and HTML comments; it does not supply a page layout or color palette.

## How it is tested

Every named check has a result: Pass, Fail or Blocked, with evidence, following
[gonk-starter PR 91](https://github.com/ikigai-rs/ikigai-gonk-starter/pull/91).
A Pass covers only the stated check, not overall accessibility. On 2026-11-13 (UTC),
`cargo test --all-targets` passed on main commit
`bdf63c198951987ca179f003e4f611c3564381dd`, before this documentation change, using
Cargo 1.99.1 and rustc 1.99.1. The automated checks below all ran in that command and
are repeated by the repository's Rust CI. They compare strings, errors and parsed XML;
they do not render a host page, evaluate its accessibility tree or establish WCAG conformance.

| Check | Result | Evidence |
|---|---|---|
| HTML text/attribute escaping, control replacement, URL encoding, safe link targets and JSON attributes | Pass | `tests/html_vectors.rs`, including `html_text_and_attr_match_the_escape_vectors`, `compose_h_escapes_by_the_same_vectors`, `safe_href_matches_the_vectors` and `json_attr_matches_the_vectors`, runs `tests/vectors/html_vectors.json`. Checks helper output, not every caller's use of it. |
| Rendered Markdown escaping, allowed/refused links, image alternatives and nonempty link labels | Pass | `tests/markdown.rs`: `raw_html_is_text_never_markup`, `the_allowed_links_become_links`, `every_other_link_renders_only_its_words`, `an_image_is_a_link_with_its_alt_text`, `an_image_with_a_refused_target_is_its_alt_text`, `a_link_with_no_words_is_labeled_with_its_target`. Checks emitted strings, not name quality. |
| Markdown heading offset, labeled disabled task boxes and scoped table headers | Pass | `tests/markdown.rs`: `headings_start_below_the_page`, `heading_offset_moves_them_and_h6_is_the_cap`, `tables_strikethrough_and_task_lists_are_on`. Checks markup, not author heading order or assistive-technology behavior. |
| Markdown XML structure, document output and refusal notice | Pass | `tests/markdown.rs`: `every_output_is_well_formed_xml` (roxmltree 0.20.0), `an_ordinary_document`, `a_host_rendering_to_a_page_gets_a_notice_for_a_refused_input`. XML validity is not an accessibility audit. |
| HTML-to-text extraction, word boundaries and removal of code/comments | Pass | `tests/html_to_text.rs`: `markup_is_not_text`, `blocks_separate_words_and_inline_elements_do_not`, `code_and_comments_are_not_text` and remaining tests. Pins current behavior, including loss of image alt text; does not establish equivalent text alternatives. |
| Literal template marker escaping | Pass | `tests/escape.rs`, including `every_marker_form_survives_compose_when_escaped` and `escaping_is_exact_for_every_run_of_dollars`. Checks preservation of template text. |
| Escaped element data and separators in each | Pass | `tests/each.rs`: `element_data_is_never_spliced_as_it_is`, `element_data_is_never_reserved_expanded`, `the_separator_goes_between_fills_escaped`. Does not check template markup accessibility. |
| Typed refusals for malformed markers, non-text input, invalid collections/conditions and prefix tables | Pass | `tests/templates.rs`, `tests/template_audit.rs`, `tests/each.rs`, `tests/prefixes.rs` and `src/lib.rs` unit tests exercise refusals. Checks asserted error types and message fragments, not readability of every message. |
| Complete error-message readability and meaning without color or terminal escapes | Blocked | No current human review of all messages. Historical source reading on 2026-10-31 is not repeated by CI; debug formatting and backticks remain limitations below. |
| Template/host language, headings, landmarks, labels and link context | Blocked | No host page was evaluated. These are supplied by callers; helper tests cannot establish their accessibility. |
| Browser rendering, contrast, zoom/reflow and host content security policy | Blocked | No browser evaluation was run for this update; no host page or browser harness is configured here. The historical source review of inline table alignment on 2026-10-31 is not a browser check. |
| Manual keyboard navigation and focus | Blocked | No person performed a keyboard pass on a host page; no interactive host session was available for this update. |
| Automated accessibility checker (axe or equivalent) | Blocked | No automated accessibility checker was run on this output; no browser checker is configured here. Rust tests are not such a checker. |
| Screen reader | Blocked | No screen reader was used on this output; no screen reader session was available for this update. Task-box labels and table navigation remain unverified. |
| Screen magnifier | Blocked | No screen magnifier was used on this output; no magnifier session was available for this update. |
| Speech input | Blocked | No speech-input software was used on this output; no speech-input session was available for this update. |
| Switch access | Blocked | No switch-access software was used on this output; no switch-access session was available for this update. |
| Braille display | Blocked | No braille display was used on this output; no braille display was available for this update. |

## Known limitations

Found by reading the source on 2026-10-31, not by testing:

- `to_text` drops every attribute value, `alt` included, so an image's text alternative is not
  in the text face: HTML whose only content is an image becomes empty text. `markup_is_not_text`
  pins this as intended.
- Rendered Markdown keeps the author's heading order, so a body that jumps from `#` to `###` skips
  a level, and a body with an image that has no alt text gives a link labeled with a URL. A link
  whose target is refused gives no sign that it was meant as a link. Task-box and table markup
  have not been tried with assistive technology.
- A table column's alignment is an inline `style`, which a strict content security policy drops.
- Whether a filled template is accessible depends entirely on the template; this crate checks
  nothing about the markup it fills (no `lang`, heading or label check).
- `conditional`'s refusal prints the answer it could not read in Rust's debug form (`{:?}`).
- Messages wrap names in backticks (`` `name` ``), Markdown punctuation that a screen reader may
  read aloud in a plain-text face.

## Contributing

Read this file before changing any output listed above: an escaping function, `to_text`, the
splicing rules or an error message. A change to escaping changes `tests/vectors/html_vectors.json`
on purpose, and the ports that copy it follow. Keep meaning in words, and say in the pull request
what was tested and what was not. Give each check a row in a Check | Result | Evidence
table with Pass, Fail or Blocked, its test or command and tool version, or why it could not
run. Keep unrun checks Blocked, including assistive technologies nobody used. A historical
run that CI does not repeat is Blocked with its date, not Pass.

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
| 2026-11-08 | Added rendered Markdown. |
| 2026-11-13 | Recorded current main test evidence and explicit Blocked checks; clarified output scope. |
