---
name: plan-change
description: Research the codebase and docs, then produce an implementation plan for a change without editing any files. Use before multi-file or behavior-changing work.
disable-model-invocation: true
argument-hint: "[what to change]"
---

Produce a plan for: $ARGUMENTS

Do NOT edit files in this skill.

1. **Understand.** Restate the goal in one or two sentences. Find the
   relevant milestone in `docs/ROADMAP.md` and its acceptance criteria.
2. **Research.** Read the code and the docs that govern it:
   ARCHITECTURE, the ADRs, ADDON_PROTOCOL and SECURITY as relevant. For
   anything external (crate versions, protocol behavior, tool behavior),
   check primary sources and label findings VERIFIED / INFERRED / UNKNOWN.
3. **Output the plan:**
   - **Scope:** what is in and what is explicitly out.
   - **Affected components:** files and modules, and which side of each
     boundary (core / IO / shell) they sit on.
   - **Architecture impact:** none, or the ADR to write or update. If an
     invariant in AGENTS.md would change, say so prominently.
   - **Steps:** the smallest vertical slice first. Each step must be
     independently verifiable.
   - **Tests:** behavior names, fixtures to add, regression test for bugs.
   - **Docs to update:** COMPATIBILITY, ADDON_PROTOCOL, SECURITY,
     DEPENDENCIES, CHANGELOG, TECHNICAL_DEBT.
   - **New dependencies:** with the research table row (version, license,
     MSRV, alternatives), or "none".
   - **Risks and UNKNOWNs,** with how each will be resolved.
4. Stop and wait for approval.
