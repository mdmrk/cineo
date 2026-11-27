# Sync server

`cineo-sync-server` stores Cineo's end-to-end encrypted sync data
(ADR-0043). It sees only a hash of each space's token and sealed records;
it cannot read titles, progress, addons or settings. This page explains how
to run your own.

Status: the server and its tests exist; the app cannot be pointed at a
server yet (that comes with the Settings screen, ADR-0043 steps 6 and 7).

## Build and run

```sh
cargo build --release -p cineo-sync-server
./target/release/cineo-sync-server --database /var/lib/cineo-sync/sync.db
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--listen` | `127.0.0.1:8787` | Address and port to serve plain HTTP on |
| `--database` | required | SQLite file; created with its tables on first start |
| `--trust-forwarded-for` | off | Limit requests by the last address in `X-Forwarded-For` instead of the connecting address. Turn it on only behind a proxy that sets that header, or anyone can choose their own address |

`RUST_LOG` sets the log level (`info` by default). The log never contains
tokens or record contents.

## TLS

The app accepts only `https` sync servers, except `http` to the same
machine (`localhost`, `127.0.0.1`, `::1`). Put the server behind a reverse
proxy that terminates TLS and forwards to `--listen`, and start the server
with `--trust-forwarded-for` so that rate limits apply per client. Which
proxies set `X-Forwarded-For` the way the server expects (the client's
address last) has not been tried (UNKNOWN).

## Limits

The server refuses a request body over 256 KiB, more than 64 records per
request, records over 32 KiB, and more than 10,000 records or 8 MiB per
space. Each client address may make 120 requests a minute and create 10
spaces an hour. A space without a write for 365 days is deleted, checked
every hour.

## Data and backups

All data is in the `--database` file (SQLite in WAL mode, so also its
`-wal` and `-shm` files). Nothing in it is readable without the sync codes,
which never reach the server. Every device keeps its own full copy.

Restoring an older copy, or starting from an empty file, is recovered by
the devices: each one notices that the server went back (or that a space is
new) and pushes what the server lacks, at the latest on its first sync
after the app starts or a day later. Until then, other devices do not see
records that only the device in question still has.

Versions are numbered from the server clock, so keep it right: a restore
together with a clock set far into the past can make the server reuse
version numbers, and devices then miss writes until their daily check.

## On Cloudflare Workers

The same server also builds as a Cloudflare Worker with a D1 database
(`crates/cineo-sync-server/wrangler.toml`). It has run only under
`wrangler dev` with its local D1 so far; a deploy has not been tried
(UNKNOWN), and this host has no per-address rate limit yet. Steps, from
`crates/cineo-sync-server`:

```sh
npx wrangler@4.161.0 login
npx wrangler@4.161.0 d1 create cineo-sync     # put the printed id in wrangler.toml
npx wrangler@4.161.0 d1 execute DB --remote --file schema.sql
npx wrangler@4.161.0 deploy
```

To try it locally, run `npx wrangler@4.161.0 d1 execute DB --local --file
schema.sql` and `npx wrangler@4.161.0 dev --local --port 8788`, then
`CINEO_SYNC_SERVER=http://127.0.0.1:8788/ cargo test -p cineo-sync-server
--test end_to_end -- --ignored`.
