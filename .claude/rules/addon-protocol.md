---
paths:
  - "crates/cineo-core/src/addon/**"
  - "crates/cineo-net/**"
  - "tests/fixtures/addons/**"
  - "docs/ADDON_PROTOCOL.md"
  - "docs/COMPATIBILITY.md"
---

# Addon protocol rules

- `docs/ADDON_PROTOCOL.md` is the spec. If code and that doc disagree, stop
  and resolve it in the doc first.
- When the SDK docs and the reference client (`stremio-core`) differ, follow
  the reference client and add a row to "Doc vs reference differences".
- Label sources: VERIFIED-DOC, VERIFIED-REF (name the file/function you
  read), INFERRED, UNKNOWN.
- Parsing (ADR-0003):
  - Required invariant missing → error.
  - Optional field unusable → absent + `Warning`.
  - Bad array element → skipped + `Warning`.
  - Never panic on input.
- URL building: `encodeURIComponent` set per component; extra = `name=value`
  joined by `&`; preserve the transport URL's prefix path and query.
- Never send a request for a path the addon's manifest does not support.
- Every new rule needs:
  1. a fixture (`basic/`, `quirks/` or `invalid/`, with the provenance
     rules from the fixtures README),
  2. a behavior-named test,
  3. a COMPATIBILITY.md update.
- Use the `/addon-compat` skill for this workflow.
