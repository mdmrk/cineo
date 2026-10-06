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
| `v1.sql` | 1 (M4) | Two addons out of insertion order; a valid item, an item with a non-http poster, and items with a negative time and an empty content type (to be skipped) |
