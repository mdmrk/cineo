# 0021. Localization: rust-i18n catalogs instead of Fluent

- Status: Accepted
- Date: 2026-10-11
- Supersedes: the Fluent runtime choice in 0017. 0017's split stays: core
  reports typed messages and the shell words them.

## Context
The owner asked to replace Fluent with `rust-i18n` (2026-10-11).

Research (crates.io and the crate source, 2026-10-11):
- `rust-i18n` 4.2.4 (2026-09-29), MIT. The `i18n!` proc macro reads
  YAML, JSON or TOML catalogs at compile time and embeds them, so the
  catalog parsers are compile-time dependencies only. VERIFIED (crate
  source: the `codegen` feature of `rust-i18n-support` is enabled only by
  `rust-i18n-macro` and by the opt-in `load-path` feature).
- `t!(key, locale = …, name = value)` looks a key up in the given locale,
  then its RFC 4647 parents, then the `fallback` list. A key missing
  everywhere returns the key itself. Arguments replace `%{name}` and are
  formatted with `Display`. VERIFIED (macro source, and the tests below).
- `rust_i18n::set_locale` is process-wide. Passing `locale =` per call
  avoids it. VERIFIED (crate source).
- It has no plural rules: no CLDR `one`/`few`/`many` selection. VERIFIED
  (README and source).

## Options considered
1. **rust-i18n with TOML catalogs, our own thread-local locale, and
   messages worded so they need no plural forms.**
2. rust-i18n plus `intl_pluralrules` for CLDR plural categories, with
   one key per category.
3. Keep Fluent (0017).

## Decision
- `cineo-desktop` embeds `assets/locales/{en,es,it,ru}.toml` with
  `rust_i18n::i18n!("assets/locales", fallback = "en")` at the crate root.
- The locale stays a thread-local `i18n::Locale`, set from the
  `ui_language` setting every frame. Our `t!` macro passes it to
  `rust_i18n::t!` as `locale =`; the process-wide `set_locale` is not used.
- A missing message falls back to English, then to the message id.
- Messages with a count are worded as a label and a number
  ("Titles: 3", "Названий: 3") or use a unit symbol ("30 s"), so every
  language reads correctly without plural selection.
- Every locale has the same message ids and `%{…}` arguments, and every
  `t!` id in the source exists. Both are checked by tests.

## Consequences
- `fluent-bundle` and `unic-langid` leave the tree; `rust-i18n` enters.
- Translators edit flat TOML (`key = "text with %{arg}"`).
- A future message that needs a real plural sentence must be reworded,
  or this ADR revisited (option 2).
- `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md#dependencies` and
  `docs/TECHNICAL_DEBT.md` are updated.

## Rejected alternatives
- Option 2: the owner chose to reword the five counted messages instead
  of adding a plural-rules dependency.
- Option 3: the owner asked for rust-i18n.
