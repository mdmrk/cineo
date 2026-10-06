@AGENTS.md

## Claude Code specifics
- Topic rules load automatically from `.claude/rules/` when you touch
  matching files. Follow them.
- Skills:
  - `/plan-change`: research and plan, no edits.
  - `/implement-change`: execute an approved plan.
  - `/review-change`: independent read-only diff review.
  - `/addon-compat`: add or verify protocol behavior.
  - `/adr`: record a decision.
- A PostToolUse hook runs rustfmt on edited `.rs` files. Formatting changes
  after an edit are expected.
- When compacting, preserve: the current plan, the files modified, the
  validation commands run and their results, and any open UNKNOWNs.
