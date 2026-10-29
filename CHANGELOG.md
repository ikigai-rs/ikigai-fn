# Changelog

## 0.8.0 (ledger #2506)

- Compose marker arguments written as unquoted `<iri>` values are issued as
  references. The kernel settles them by the target contract; quoted angle values
  remain inline text. Quote literal markup arguments that begin and end with angle
  brackets (for example, `in="<i>x</i>"`). Template variables inside references are percent-encoded,
  and query delimiters inside angle brackets stay in the reference IRI. Requires
  ikigai-core 0.2.8 for contract-based reference settlement (K1).

## 0.7.2 (ledger #2528)

- `html::to_text` and the `html-to-text` endpoint at `urn:iki:fn:html-to-text`, in
  `space()`: the words HTML shows a reader, as one line of plain text. It is built on
  html5ever's WHATWG tokenizer (new dependency, `html5ever = "0.40"`, wasm-clean). Tags,
  attribute values, comments and `script`/`style`/`template` bodies are dropped and
  references are decoded. A block boundary or `<br>` separates words, and whitespace
  collapses. With it, a search over an RSS description or `content:encoded` matches words,
  not `strong`, `img` or `href`, and a page can show a summary as text. Declared pure
  and held to it by the conformance walk.

## 0.7.1 (ledger #2498)

- Declare compose's Source arguments as open, so REPL requests such as
  `source urn:iki:fn:compose src=urn:file:param.txt min=6` pass `min` to the
  template's `{min}` marker. The required `src` input is unchanged. This changes
  compose's contract IRI because the open argument set is part of the contract.
