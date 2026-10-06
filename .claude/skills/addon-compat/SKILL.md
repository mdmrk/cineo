---
name: addon-compat
description: Add, change or verify a piece of addon-protocol behavior (manifest, catalog, meta, stream, subtitles, request encoding, filtering) with spec text, fixture, test and compatibility status. Use when protocol handling changes or an addon misbehaves.
argument-hint: "[behavior or addon]"
---

Protocol behavior to handle: $ARGUMENTS

1. **Establish the expected behavior** from primary sources and label it:
   - the SDK docs (<https://stremio.github.io/stremio-addon-sdk/>):
     VERIFIED-DOC;
   - the reference client `stremio-core` (MIT; name the file/function):
     VERIFIED-REF;
   - otherwise INFERRED or UNKNOWN.

   If they differ, follow the reference client. Never copy reference code;
   reimplement it.
2. **Spec first.** Update `docs/ADDON_PROTOCOL.md`: the rule, its label and,
   if relevant, a row in "Doc vs reference differences".
3. **Fixture.** Add or extend a file in `tests/fixtures/addons/{basic,quirks,invalid}/`.
   - Hand-written, illustrative ids only.
   - Never paste configured addon URLs or real user data.
   - Follow the provenance rules in the fixtures README.
4. **Test.** Add a behavior-named test in `cineo-core` (and in `cineo-net`
   if transport is involved). Assert warning **locations** for lenient
   cases. Show the test failing before the implementation when fixing a bug.
5. **Implement** the change in `cineo-core::addon` (pure). Keep lenient
   parsing and strict domain types (ADR-0003).
6. **Status.** Update the row in `docs/COMPATIBILITY.md`:
   - Supported needs the test name.
   - Verified needs a dated check against a real public addon (add it to
     "Verified addons").
7. Run `scripts/check.sh` and report the results.
