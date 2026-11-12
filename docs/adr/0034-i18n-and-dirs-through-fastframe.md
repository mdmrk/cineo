# 0034. Translations and folders through fastframe

- Status: Accepted
- Date: 2026-11-12
- Supersedes: the catalog format and lookup of 0021 (its wording rules
  stay), and the `i18n` and `dirs` entries in the rejected alternatives of
  0032.

## Context
The owner asked to adopt `fastframe-i18n` and `fastframe-dirs` 0.17.0
(2026-11-12). Research, 2026-11-12, from the crates' source at the tag:

- VERIFIED: `fastframe-i18n` compiles `assets/i18n/*.po` into Rust in
  `build.rs` (`polib` and `include-po` are build-time only) and looks
  messages up with `gettext`, `pgettext` and `ngettext`. English is the
  source language and has no catalog. Fuzzy or empty entries fall back to
  English. `detect` walks every preferred system language.
- VERIFIED: GNU `xgettext` 1.0 extracts Rust macro calls with
  `--keyword=t!:1` and `--keyword=pt!:1c,2` (tried on a sample file).
- VERIFIED: `fastframe-dirs` uses `directories::ProjectDirs`. With an
  empty qualifier and organization, the database moves from
  `~/.local/share/cineo` to `~/.local/state/cineo` (or
  `$XDG_STATE_HOME/cineo`) on Linux
  (`data_goes_to_the_xdg_state_folder_and_cache_to_the_xdg_cache_folder`).
  On Windows it moves from `%APPDATA%\Cineo\data` to
  `%LOCALAPPDATA%\Cineo\data` (VERIFIED in the `directories` 6.0.0
  source; not run on Windows). The cache folder does not change. A portable
  copy needs a `cineo-portable.txt` marker next to the executable, which
  Cineo does not ship.

## Options considered
For message ids: English source text (gettext practice), or the current
keys inside PO files. For existing data: move it, copy it, or leave it.

## Decision
- Catalogs are `crates/cineo-desktop/assets/i18n/<tag>.po`, with English
  text as the message id and `cineo.pot` as the template.
  `scripts/update-translations.sh` runs fastframe's extraction script.
  `t!("English {name}", name = value)` translates and fills `{name}`.
  `pt!("context", "English")` is used where one English text has
  different translations. The system language follows
  `fastframe_i18n::detect`, which tries every preferred language in order.
  The wording rules of 0021 stay: counted messages are a label and a
  number.
- The data folder is the `state` folder of
  `fastframe_dirs::App::installed()`, and the cache its `cache` folder.
  Data in the previous folders is left where it is: the owner chose not to
  move it. `--data-dir` and `--cache-dir` still override both. Portable
  mode is not offered.

## Consequences
- `rust-i18n`, `sys-locale` (as a direct dependency) and `etcetera` leave
  the tree. `fastframe-i18n` and `fastframe-dirs` enter.
- A user upgrading from an earlier version starts with an empty library
  unless they pass `--data-dir` or move the folder (TECHNICAL_DEBT.md).
- Updating the template needs GNU gettext 0.24 or later. Builds do not.

## Rejected alternatives
- Keys as message ids: no `xgettext` extraction or `msgmerge`.
- Moving or copying the old data folder: the owner chose to leave it.
- `fastframe-icons`: it draws SVG images, which needs `egui_extras`' SVG
  loader and about 20 more crates, and images cannot sit inside egui text,
  where Cineo also draws icons. The owner chose to keep Tabler glyphs from
  `iconflow` (2026-11-12).
