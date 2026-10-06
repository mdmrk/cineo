---
name: review-change
description: Independent read-only review of the current diff against Cineo's architecture, security, testing and documentation rules. Reports findings; does not fix them.
disable-model-invocation: true
context: fork
agent: Explore
background: false
argument-hint: "[base ref, default main]"
---

Review the changes on this branch against the base ref `$ARGUMENTS` (use `main`
if empty). Use
`git diff` and `git status`; include untracked files. Read `AGENTS.md` and
the docs relevant to the touched areas first.

**Report, do not fix.** For each finding give: severity (blocker / major /
minor), `file:line`, the problem, a concrete failure scenario, and a
suggested direction.

Check:
1. **Scope:** unrelated edits, refactors or formatting churn.
2. **Architecture:** the dependency direction (core has no IO/async), crate
   boundaries, effects as data, accidental invariant changes without an ADR.
3. **Correctness:** edge cases, error handling (no `unwrap` on input paths;
   errors carry context), partial-failure behavior.
4. **Concurrency:** blocking in async, unbounded tasks/channels, missing
   timeouts, ordering assumptions.
5. **Security** (docs/SECURITY.md): untrusted input handling, the network
   policy bypassed or relaxed, secrets/URLs in logs or errors, player
   argument or header injection, workflow permissions and pinning.
6. **Tests:** every behavior change is tested, tests are named for
   behavior, regression tests exist for fixes, no network or sleeps.
7. **Dependencies:** new crates justified in docs/dev/DEPENDENCIES.md, with
   minimal features and an allowed license.
8. **Docs:** COMPATIBILITY/ADDON_PROTOCOL/SECURITY/CHANGELOG updated;
   claims labelled; nothing aspirational presented as fact.

Only flag issues that affect correctness, security, maintainability or the
stated rules. Skip style preferences. End with a one-line verdict:
**ready**, **ready after minor fixes**, or **needs changes**.
