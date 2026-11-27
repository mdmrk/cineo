# Sync server

`cineo-sync-server` stores Cineo's end-to-end encrypted sync data
(ADR-0043). It sees each space's public key and sealed records, checks
the signature on every request, and cannot read titles, progress, addons
or settings. This page explains how to run your own.

Status: the server is deployed as Cineo's default, and the desktop and
Android apps can be pointed at your own in Settings → Data → Sync server.
Every device in a space must use the same server. The server's clock must
be right within a few minutes, because signed requests more than 10
minutes off are refused.

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
signatures, keys or record contents.

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
(`crates/cineo-sync-server/wrangler.toml`). The default instance runs
this way at `https://cineo-sync.cineo-sync-server.workers.dev`. Its rate
limits are Cloudflare Rate Limiting bindings in `wrangler.toml` (120
requests a minute per space, 10 new spaces a minute per address); they are
approximate and per location, so short bursts get through. If you deploy
your own, pick other `namespace_id` values only if they clash with
limiters you already have. To deploy your own, from
`crates/cineo-sync-server` (with your own database id in
`wrangler.toml`):

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
