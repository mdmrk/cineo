# 0003. Lenient wire parsing, strict domain types

- Status: Accepted
- Date: 2026-10-06

## Context
Real addon responses are frequently malformed: empty-string URLs, numbers where strings are documented, nulls, unknown enum values, duplicates. The reference client tolerates these with per-field serde adapters (`DefaultOnError`, `NoneAsEmptyString`, `VecSkipError`, `UniqueVec`) — VERIFIED-REF. Silent tolerance, however, hides problems from users and developers.

## Options considered
1. Strict serde derive — breaks many real addons.
2. Copy the reference adapters' behavior silently.
3. Two layers: a lenient interpretation of the JSON that records a `Warning` (with location) for everything it drops or defaults, producing strict domain types with invariants.

## Decision
Option 3. Parsers return `Result<Parsed<T>, Error>` where `Parsed { value, warnings }`. Errors only when a required invariant cannot be established (e.g. manifest `id`, `version`). Domain types encode invariants (non-empty ids, http(s)-only image URLs, resolved resource inheritance) so downstream code does not re-validate. Where documented behavior and the reference client differ, follow the reference client and record it in ADDON_PROTOCOL.md.

## Consequences
- Every tolerance rule needs a fixture and a test (TESTING.md).
- Warnings are surfaced in logs and `cineo addon inspect`, giving addon authors actionable feedback.
- Slightly more code than derive-based parsing.

## Rejected alternatives
- Option 1 fails the compatibility goal. Option 2 makes debugging addon issues guesswork.
