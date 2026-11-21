# Tessel

End-to-end encrypted messaging, open source, on the web and on mobile. Tessel aims to replace WhatsApp and takes its spaces, channels and roles from Discord. The server reads nothing: it only routes envelopes it cannot open.

Why and how: [the vision](.workstream/vision.md). The work is tracked in [`.workstream/`](.workstream/).

## Layout

| Folder | Contents |
|---|---|
| `apps/server` | [Ream](https://github.com/C9up/ream) app: the API (`comet`), real time (`relay`) and the interface (aurora / nebula), written once for the web and then for mobile |
| `crates/tessel-core` | The Rust core: MLS, encrypted local database, send queue, media encryption |
| `crates/tessel-wasm` | Its WASM binding for the web. The mobile binding will follow Ream's mobile stack (ADR-003) |
| `packages/core-web` | The core for the browser: the WASM module and its loader |
| `packages/protocol` | The client-server contract, shared by the server and the clients |

## Running

Prerequisites: Node ≥ 24, pnpm 12, Rust through rustup (the toolchain and its targets are pinned by `rust-toolchain.toml`; `scripts/cargo.sh` goes through rustup even when it is not in the PATH), `wasm-bindgen-cli` 0.2.129, Podman, `podman-compose`, `openssl` and `python3`.

PostgreSQL is reachable only over mutual TLS (`sslmode=verify-full` and a client certificate), in development as in production. It keeps only what devices have already end-to-end encrypted, or opaque identifiers (ADR-006). Two roles: `tessel_owner` owns the schema and runs the migrations, `app` runs the application and owns nothing.

```bash
pnpm install
cp apps/server/.env.example apps/server/.env
pnpm --filter @tessel/server exec ream generate:key
pnpm infra      # PostgreSQL and KeyDB, one compose project (scripts/stack.sh); writes the DB_* lines of .env
pnpm --filter @tessel/server migrate   # as tessel_owner (scripts/migrate.sh)
pnpm dev        # server + web shell
pnpm check      # lint, types, TS and Rust tests, dependency audit
pnpm infra:down # stops everything and erases the stack's certificates and passwords
```

The core for the web:

```bash
pnpm --filter @tessel/core-web build   # tessel-wasm → packages/core-web/generated
```

Mobile uses neither React Native nor Expo: it will be built on Ream's mobile stack (its ADR-009, epic 73), of which Tessel is the pilot app.

Development ports, chosen to be free of the machine's other Ream apps:

| Service | Port |
|---|---|
| Server (API + web) | 3460 |
| PostgreSQL | 5460 |
| KeyDB | 6460 |

## License

[MIT](LICENSE) (ADR-005).
