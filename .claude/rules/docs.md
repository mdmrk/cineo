---
paths:
  - "docs/**"
  - "*.md"
---

# Documentation rules

- Docs describe **verified** behavior. Use VERIFIED / INFERRED / UNKNOWN for
  claims, and the status vocabulary in `docs/COMPATIBILITY.md` for features.
- One home per fact. Link instead of duplicating; `AGENTS.md` links, it does
  not explain.
- Date research snapshots (versions, external behavior): "as of YYYY-MM-DD".
- Never rewrite an accepted ADR's decision. Supersede it with a new ADR and
  update the index in `docs/DECISIONS.md`.
- Keep instruction files short: `AGENTS.md` and `CLAUDE.md` under about
  100 lines, each rule file under about 50. Prune instructions that no
  longer change behavior.
- Do not add documents without a clear reader and purpose.
