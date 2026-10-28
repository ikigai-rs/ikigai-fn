# Changelog

## Unreleased

- Declare compose's Source arguments as open, so REPL requests such as
  `source urn:iki:fn:compose src=urn:file:param.txt min=6` pass `min` to the
  template's `{min}` marker. The required `src` input is unchanged. This changes
  compose's contract IRI because the open argument set is part of the contract.
