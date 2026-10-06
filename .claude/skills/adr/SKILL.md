---
name: adr
description: Record an architecture decision as a numbered ADR and update the decisions index. Use when a change affects crate boundaries, core concepts, state flow, security policy, protocol scope, or adds an architectural dependency.
argument-hint: "[decision topic]"
---

Record a decision about: $ARGUMENTS

1. Read `docs/DECISIONS.md` and any related ADRs. If an accepted ADR
   already covers the topic, the new ADR **supersedes** it. Never edit an
   accepted decision in place; only its Status line changes, to
   "Superseded by NNNN".
2. Research the options from primary sources, with current versions and
   docs. Label claims VERIFIED / INFERRED / UNKNOWN.
3. Copy `docs/ADR/TEMPLATE.md` to `docs/ADR/NNNN-kebab-title.md`, using the
   next free number. Fill in Context, Options considered, Decision,
   Consequences and Rejected alternatives. Use Status `Proposed` unless the
   user has accepted it.
4. Add the ADR to the table in `docs/DECISIONS.md`.
5. List the follow-up doc updates the decision implies (ARCHITECTURE.md,
   AGENTS.md invariants, rules, SECURITY.md). Apply them only if the user
   asked to accept the ADR.
