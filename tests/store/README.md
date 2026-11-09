# Store fixtures

One SQL script per **released** schema version of `cineo-store`. Each script
recreates a database exactly as that version wrote it, with sample rows,
including rows that later code must tolerate. Migration tests open every
script's database with the current code (`crates/cineo-store/tests/`).

Rules:
- Never edit a script after its version was released; add a new one.
- Hand-written; no real user data. Ids are IMDb-style placeholders.

| File | Schema version | Notes |
|------|----------------|-------|
| `v1.sql` | 1 | Two addons out of insertion order; a valid item, an item with a non-http poster, and items with a negative time and an empty content type (to be skipped) |
| `v4.sql` | 4 | First released schema (`v0.1.0`). Items with a URL stream, a torrent stream and an unreadable stream, a negative favorite order, and an empty content type (to be skipped); settings with an unreadable value and a retired key |
