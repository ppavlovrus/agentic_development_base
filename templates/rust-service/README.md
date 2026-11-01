# rust-service template

A Rust service template built for agent-driven development: a working, fully
tested vertical slice (entity → repository → service → HTTP API → background
worker) rather than a folder of instructions. Agents copy live patterns from
the Task example; the compiler enforces the architecture.

Part of the `agentic_development_base` harness: **copy this directory to
start a new project** (`cp -r templates/rust-service ~/RustProjects/<name>`),
then follow the Quickstart below.

**Stack**: Rust (edition 2024) · axum · PostgreSQL + sqlx (compile-time checked
queries) · Redis · RabbitMQ (lapin) · tokio · tracing. Clean Architecture as a
5-crate cargo workspace where layer boundaries are compile errors, not
conventions.

## Prerequisites

- Rust via [rustup](https://rustup.rs) (the version is pinned in `rust-toolchain.toml`)
- Docker (with the daemon running)
- [`just`](https://github.com/casey/just) — `cargo install just` or `brew install just`
- [`sqlx-cli`](https://github.com/launchbadge/sqlx) — `cargo install sqlx-cli --no-default-features --features postgres,rustls`

## Quickstart

```bash
cp .env.example .env
just setup        # docker: postgres/redis/rabbitmq + migrations
just check        # fmt + clippy -D warnings + unit tests
just run          # HTTP API on :8080
```

```bash
curl -X POST localhost:8080/api/tasks -H 'content-type: application/json' -d '{"title":"hello"}'
curl localhost:8080/api/tasks
```

In a second terminal: `just run-worker` — it consumes `task.created` events
into a Redis counter (`redis-cli get stats:tasks_created` to see it).

Integration tests spin throwaway containers via testcontainers:

```bash
just test-int
```

RabbitMQ management UI: http://localhost:15672 (app/app).

## Layout

```
crates/domain   pure business logic (no infra deps allowed)
crates/app      use cases over domain traits
crates/infra    sqlx / redis / lapin implementations, config, telemetry
crates/server   axum HTTP API (binary)
crates/worker   RabbitMQ consumers (binary; remove for single-task projects)
```

## For agents and humans

- [`AGENTS.md`](AGENTS.md) — the agent contract: boundaries, quality gate,
  conventions, known shortcuts.
- [`skills/`](skills/) — task recipes: add-entity, add-endpoint, add-consumer,
  SQL & migrations, testing (in Russian).

CI (`.github/workflows/ci.yml`): fmt, clippy, unit tests, offline SQL cache
check, testcontainers integration tests, release build — all offline-safe via
the committed `.sqlx/` cache.
