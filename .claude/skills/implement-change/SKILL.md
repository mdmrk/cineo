---
name: implement-change
description: Implement an approved plan in small verified steps, with tests and docs in the same change.
disable-model-invocation: true
argument-hint: "[plan reference or summary]"
---

Implement the approved plan: $ARGUMENTS

If there is no approved plan in the conversation or the referenced file,
stop and ask for one (or suggest `/plan-change`).

For each plan step:
1. For bug fixes, write the failing test first and show that it fails.
2. Make the minimal change for the step. Do not touch unrelated code: no
   drive-by refactors, renames or reformatting.
3. Run the narrowest relevant tests (`cargo test -p <crate> <name>`).

After all steps:
4. Run `scripts/check.sh`. Fix the root causes of failures. Never weaken a
   lint, test or policy to get green.
5. Update the docs listed in the plan, plus any others the change made
   stale (AGENTS.md → "Documentation sync").
6. If reality diverged from the plan, say how and why. If the divergence
   is architectural, stop and propose an ADR instead of continuing.
7. Summarize:
   - the files changed,
   - the tests added,
   - the exact validation commands and their results,
   - the remaining UNKNOWNs,
   - a suggested Conventional Commit message.
