# Technical debt and known gaps

Known limitations that are **intentional for now**. Each entry says:
- what is affected (with a link to code),
- what does not work,
- why,
- what happens instead,
- what would remove the limitation.

Fix an entry and delete it in the same change. Do not hide debt in `TODO`
comments without an entry here.

Format:

```
### <Area>: <short title>
- Where: [path](../../path)
- Gap: …
- Why: …
- Instead: … (observable behavior today)
- Exit: … (what would fix it; link issue/milestone)
```

## Entries

### Store: no in-app recovery from a corrupt database
- Where: [store.rs](../../crates/cineo-store/src/store.rs)
- Gap: a corrupt `cineo.db` makes every persistence command fail.
- Why: silently replacing it would lose the user's library.
- Instead: the error and `cineo doctor` explain it; the user moves the file
  away and Cineo starts fresh.
- Exit: a GUI prompt (M5) that renames the file aside and starts fresh.

### Desktop: no log file or diagnostics view
- Where: [main.rs](../../crates/cineo-desktop/src/main.rs)
- Gap: logs go to stderr only; DEBUGGING.md planned a rotating file and an
  About/Diagnostics view.
- Why: it would add a dependency (`tracing-appender`) and a page outside
  M5's acceptance.
- Instead: start `cineo-desktop -v` from a terminal; `cineo doctor` covers
  the database.
- Exit: a log file in the platform state directory, plus a diagnostics page.

### Desktop: image cache is unbounded for a session
- Where: [images.rs](../../crates/cineo-desktop/src/images.rs)
- Gap: fetched image bytes (≤ 4 MiB each) stay in memory until exit.
- Why: egui decides when to forget images; a size-bounded cache was not
  needed for normal browsing.
- Instead: memory grows with the number of distinct posters viewed.
- Exit: an LRU cap on `NetImageLoader` bytes.

### Desktop: private networks are a launch flag only
- Where: [main.rs](../../crates/cineo-desktop/src/main.rs)
- Gap: self-hosted addons need `--allow-private-network`; there is no
  setting, and no per-addon trust (GOALS.md v0.x).
- Why: a settings page was not in M5's scope.
- Instead: blocked addons fail to install with "blocked by network policy".
- Exit: a settings page, then per-addon trust.

### Store: manifests are not cached
- Where: [migrate.rs](../../crates/cineo-store/src/migrate.rs) (only
  transport URLs are stored)
- Gap: at startup every installed addon's manifest is fetched again; an
  addon that is offline is reported and missing until the next start.
- Why: the core's `Restore` action takes URLs only (M2); caching was not in
  M4's scope.
- Instead: `State.notice` reports the addon that failed to load.
- Exit: store the last good manifest and restore from it (v0.x caching).
