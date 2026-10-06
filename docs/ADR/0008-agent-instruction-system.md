# 0008. AGENTS.md canonical; layered Claude rules and skills

- Status: Accepted
- Date: 2026-10-06

## Context
The project is developed largely with coding agents. Current Claude Code docs (read 2026-10-06, code.claude.com/docs/en/memory, /skills, /best-practices): Claude reads `AGENTS.md` natively only when no `CLAUDE.md` exists; a `CLAUDE.md` containing `@AGENTS.md` imports it (max 4 import hops); `.claude/rules/*.md` support `paths:` frontmatter and load when matching files are read/edited; instruction files should stay under ~200 lines because adherence drops with length; skills load on demand and support `disable-model-invocation`, `context: fork`, `agent:`; `/plan` and `/review` are built-in commands, so project skills must use other names; hooks are the only deterministic enforcement.

## Decision
- `AGENTS.md`: canonical, agent-neutral engineering contract (short; links to docs instead of repeating them).
- `CLAUDE.md`: `@AGENTS.md` plus Claude-specific notes only.
- `.claude/rules/`: four path-scoped rules (rust, addon-protocol, security, docs).
- `.claude/skills/`: `plan-change`, `implement-change`, `review-change` (forked, read-only), `addon-compat`, `adr`. User-invoked except `addon-compat` and `adr`, which Claude may pick up when relevant.
- `.claude/settings.json`: permission allowlist for the validation commands and a PostToolUse rustfmt hook on edited `.rs` files.
- Docs are the source of truth; instructions point to them.

## Consequences
- Other agents (Codex, etc.) get the same contract via AGENTS.md.
- Rules and skills must be pruned when they stop changing behavior; `/doctor prompt-audit` can help.
- The hook requires `jq` and `rustfmt` on PATH; failure is non-blocking.

## Rejected alternatives
A large CLAUDE.md (ignored rules, duplication); symlinking CLAUDE.md → AGENTS.md (breaks on Windows checkouts; Edit tools refuse symlinks — VERIFIED-DOC).
