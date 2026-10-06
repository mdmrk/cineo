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

### Store: manifests are not cached
- Where: [migrate.rs](../../crates/cineo-store/src/migrate.rs) (only
  transport URLs are stored)
- Gap: at startup every installed addon's manifest is fetched again; an
  addon that is offline is reported and missing until the next start.
- Why: the core's `Restore` action takes URLs only (M2); caching was not in
  M4's scope.
- Instead: `State.notice` reports the addon that failed to load.
- Exit: store the last good manifest and restore from it (v0.x caching).
