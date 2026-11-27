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

Do not restore an older copy of the database, or replace it with an empty
one, while devices are paired. Devices remember record versions and a pull
position newer than what the server then has; they get conflicts on every
push and miss other devices' new records. Recovering from that is not built
yet (ADR-0043, step 5c). The same applies to a space swept after 365
days.
