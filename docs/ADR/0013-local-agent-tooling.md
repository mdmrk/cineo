# 0013. Agent tool configuration is local; the repo keeps AGENTS.md and docs

- Status: Accepted
- Date: 2026-10-06
- Supersedes: 0008

## Context
ADR-0008 versioned a Claude Code setup in `.claude/` (four path-scoped rules, five skills, a settings file with a permission allowlist and a rustfmt hook). The owner found the repository carried too many configuration and documentation files (owner decision 2026-10-06). The rules mostly repeated content that already has a home in the docs (SECURITY.md, ADDON_PROTOCOL.md, TESTING.md); the Rust, documentation and workflow-hardening conventions existed only there.

## Options considered
1. Keep `.claude/` versioned (ADR-0008) — consistent agent behavior for every clone / a second copy of the rules to keep in sync, tool-specific files in the tree.
2. Stop versioning `.claude/` and move any rule that exists only there into the docs — one home per fact, tool-neutral / each contributor sets up their own agent tooling.

## Decision
- `.claude/` is ignored by git. Contributors may keep a local one; it is not reviewed or relied on.
- `AGENTS.md` stays the canonical, tool-neutral contract. `CLAUDE.md` stays versioned and only imports it (`@AGENTS.md`).
- Conventions that lived only in `.claude/rules/` moved to `CONTRIBUTING.md` (Rust and documentation conventions) and `docs/SECURITY.md` (code and CI rules).
- Formatting is enforced by `scripts/check.sh` and CI, not by an editor hook.

## Consequences
- Fresh clones get no project skills, path-scoped rules, permission allowlist or rustfmt hook; agents work from AGENTS.md and the docs it links.
- Docs no longer reference skills (`/adr`, `/addon-compat`, …).

## Rejected alternatives
Keeping only `.claude/settings.json` versioned: the hook and allowlist are personal convenience, not project policy; `scripts/check.sh` is the enforced gate.
