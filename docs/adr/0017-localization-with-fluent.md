# 0017. Localization: Fluent catalogs in the desktop shell; core reports typed messages

- Status: Accepted
- Date: 2026-10-08

## Context
The desktop UI's text was hard-coded English, and `cineo-core` built some
user-visible English strings too: `Loadable::Failed(String)` ("Invalid addon
URL: …"), `State::notice` ("Torrent streams are turned off in Settings") and
catalog titles ("Popular Movies"). The owner asked for localization, with
English and Spanish shipping first, the system language as the default, and
Fluent as the format (2026-10-08).

Research (crates.io, 2026-10-08):
- `fluent-bundle` 0.16.0, Apache-2.0 OR MIT, MSRV 1.67. Mozilla's Fluent
  runtime, with CLDR plural rules (`intl_pluralrules`). VERIFIED (crate
  metadata and source).
- `unic-langid` 0.9.6 (MIT/Apache) is already a dependency of
  `fluent-bundle`. Its `macros` feature gives a compile-time `langid!`.
- `sys-locale` 0.3.2 (MIT/Apache) reads the OS locale. On Unix its only
  dependency is `libc`. VERIFIED (crate metadata).
- `rust-i18n` 4.2.4 uses YAML/TOML and macros, and its plural handling is
  weaker. `i18n-embed` builds on Fluent but adds a filesystem and asset
  layer that we do not need.
- egui does not shape right-to-left text, and the bundled fonts are
  Latin-only. VERIFIED for the fonts (`theme.rs`). INFERRED for RTL (no
  bidi support in epaint).

## Options considered
1. **Fluent catalogs in `cineo-desktop`; core returns typed reasons.** Core
   stays language-free. The shell owns every word on screen.
2. Fluent in `cineo-core`, which would return translated strings. That
   puts a locale in core state and turns every `update` into a function of
   the UI language.
3. A new `cineo-i18n` crate. There is no new IO boundary, so ADR-0002 says
   to use a module.

## Decision
- `cineo-core` builds no user-visible prose. Failures are
  `Loadable::Failed(Problem)` and notices are `State::notice:
  Option<Notice>`, both typed enums. A `String` inside them is a detail
  that an IO layer produced (an HTTP status, an mpv error), and it is shown
  as is. A catalog target carries the catalog's `name`; the shell adds the
  content type.
- The UI language is the core setting `ui_language` (`system`, `en`, `es`).
  Core only stores it.
- `cineo-desktop/src/i18n.rs` holds one Fluent bundle per locale, built
  from `.ftl` files embedded with `include_str!`. The locale is
  thread-local and is set from the setting at the start of every frame.
  `System` is resolved once at startup through `sys-locale`; tests resolve
  it to English. Lookups go through the `t!` macro. A missing message falls
  back to English, and then to the message id.
- Unicode isolation marks are off, because egui draws them as boxes.
- Every locale has the same message ids, and every `t!` id in the source
  exists. Both are checked by tests.
- Locales that need right-to-left shaping or non-Latin fonts are not
  offered until the UI can render them.

## Consequences
- Adding a language means adding an `.ftl` file and a `UiLanguage`
  variant. No code changes elsewhere.
- Core tests assert on enums instead of English substrings.
- Addon-provided text (names, descriptions, genres) is never translated.
- Technical details from `cineo-net`, `cineo-player` and `cineo-stream`
  stay in English inside a translated sentence. Recorded in
  `docs/TECHNICAL_DEBT.md`.

## Rejected alternatives
- Option 2: core would depend on a localization runtime, and its state
  would change with a display preference. That breaks the
  "effects as data" core (ADR-0001).
- Option 3: a crate without an IO boundary (ADR-0002).
- `rust-i18n`: weaker plurals, and its YAML would need `serde_yaml` in the
  tree. A hand-written table: plurals would be ours to get right, and
  translators would have to edit Rust.
