# Rust Development Reference

Deep-dive companion to [SKILL.md](SKILL.md). Anchors: workspace · ownership ·
traits · tokio · sqlx · lapin · redis · tracing · testing · tooling ·
debugging.

## Workspace {#workspace}

### Layout ladder

Start small; escalate only when it hurts:

1. **Single crate** — `src/main.rs` + modules. Right answer for CLIs and
   small services.
2. **One lib crate + thin bins** — `src/lib.rs` (everything testable) and
   `src/bin/*.rs` (composition roots) when 2 binaries share logic.
3. **Cargo workspace** — several `[[bin]]`s with distinct deps/teams/release
   cadence, or when you want *compiler-enforced* layer boundaries (a `domain`
   crate that cannot depend on `sqlx` even if an agent tries).

### Workspace skeleton

```toml
# root Cargo.toml
[workspace]
resolver = "3"
members = ["crates/domain", "crates/app", "crates/infra", "crates/server", "crates/worker"]

[workspace.dependencies]        # single source of versions
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
sqlx = { version = "0.9", default-features = false, features = ["runtime-tokio", "tls-rustls", "postgres", "uuid", "chrono", "migrate", "macros"] }
domain = { path = "crates/domain" }

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
unwrap_used = "warn"
```

Every member crate: `[lints] workspace = true` and
`dep = { workspace = true }`.

### Feature flag pitfalls

- Features are **additive and unified** across the graph: if any crate
  enables `sqlx/runtime-async-std`, it merges with tokio. Prefer
  `default-features = false` + explicit features everywhere you care.
- Executors are feature-picked (`lapin` default = tokio; `sqlx` needs
  `runtime-tokio` or every await panics).
- Optional integrations behind your own features:
  `otlp = ["dep:opentelemetry-otlp"]` keeps heavy deps out of default builds.

### Toolchain and editions

- `rust-toolchain.toml` with a pinned channel keeps every machine and CI on
  the same compiler.
- Edition 2024 is the current default for new projects (`let-else` chains,
  `if let` && chains, RPIT in traits stabilized in this era).
- Commit `Cargo.lock` for applications/bins; libraries usually don't.

---

## Ownership {#ownership}

### Lifetime decision tree

- Function borrows → `&T` / `&str` / `&[T]`; caller keeps ownership
- Struct stores beyond the call → own (`String`, `Vec<T>`); borrows in
  structs (`&'a T`) are for views/caches with a clear shorter scope
- Multiple owners, one thread → `Rc<T>`; across threads → `Arc<T>`
- Mutation shared → `Rc<RefCell<T>>` / `Arc<Mutex<T>>` — but first ask why
  the data isn't in one place (store, channel, or task-local)
- Avoid copying on return → `Cow<'a, str>` for mostly-borrowed-sometimes-owned

### Smart pointers quick table

| Type | Thread-safe | Use |
|---|---|---|
| `Box<T>` | — | Heap allocation, recursive types, trait objects |
| `Rc<T>` | no | Shared read-only within one thread |
| `Arc<T>` | yes | Shared handles (clone = refcount bump, cheap) |
| `Cow<'a, B>` | — | Borrow normally, allocate only when mutation needed |
| `Arc<Mutex<T>>` | yes | Shared mutation — keep critical sections tiny |

### Fighting the borrow checker productively

1. Read the error: it names the exact conflicting borrows and their spans.
2. Common real causes: returning a borrow of a temporary; mutating while
   iterating (collect keys first, or `retain`/`drain`); self-referential
   structs (redesign, or `ouroboros`-style crate — rare).
3. Legit escapes in order: reborrow (`&*x`), scope the mutable borrow, split
   fields into locals, then — last resort — `clone()` with a comment on why.

---

## Traits {#traits}

### Object safety

`dyn Trait` requires object-safe methods (no generics, no `Self: Sized`
returns, no returning `impl Trait`). Async methods break it — hence
`#[async_trait]`, which desugars to `-> Pin<Box<dyn Future>>` (one allocation
per call; irrelevant next to any I/O). Native async-fn-in-trait (AFIT) works
for static dispatch (`impl Trait` bound) but **not** for `dyn Trait`.

### Choosing bounds

```rust
fn parse_all<I: IntoIterator<Item = String>>(items: I) -> Vec<Title>  // generics: static, zero-cost
fn repo(&self) -> &Arc<dyn TaskRepository>                             // dyn: swappable, injectable
```

Rule: generics for algorithms/utilities; `dyn` for dependencies you swap in
tests or between environments.

### Send + Sync at boundaries

Anything awaited inside `tokio::spawn` must be `Send`. Local non-Send values
are fine on the current task — the requirement only bites at spawn points and
when storing in shared state (`Send + Sync`). Express it once at the boundary
(`Box<dyn Error + Send + Sync>`), don't sprinkle bounds.

---

## tokio {#tokio}

### Channels

| Channel | Pattern |
|---|---|
| `mpsc` | producer(s) → single consumer; backpressure via `capacity` |
| `oneshot` | single request → single reply |
| `broadcast` | fan-out to many subscribers (each gets a copy) |
| `watch` | latest-value-only, change notification |

Prefer channels over shared `Mutex<State>` for coordinating tasks — the
channel *is* the synchronization.

### Mutexes: std vs tokio

- `std::sync::Mutex` — fine for short critical sections **without** `.await`
  inside (use `parking_lot` for cheaper locks if profil says so)
- `tokio::sync::Mutex` — only when you must hold across an `.await`
- Holding a lock across `.await` is usually a design smell: shrink the
  section, copy data out, or restructure around a channel/task-owner

### Tasks

- `tokio::spawn` → `JoinHandle`; detached by default (handle dropped = task
  keeps running); `handle.abort()` for cancellation
- Errors: `JoinError` if panicked/aborted — unwrap the double Result
  (`spawn(...)).await` gives `Result<Result<T, E>, JoinError>`)
- `spawn_blocking` for CPU-bound/blocking FFI; keep the pool small, pass
  owned data in/out
- `FuturesUnordered` / `JoinSet` for bounded fan-out with early exits

### Cancellation and time

- `.await` cancellation is implicit: futures are dropped at `select!` branch
  switches — keep branches cancel-safe (idempotent, no partial side effects)
- `tokio::time::timeout` wraps risky awaits: always bound external calls
- `CancellationToken` (tokio-util) is the clean shutdown signal: cancel in
  signal handler, `select!` on `token.cancelled()` in every loop

---

## sqlx {#sqlx}

### Pool

```rust
let pool = PgPoolOptions::new()
    .max_connections(5)
    .acquire_timeout(Duration::from_secs(5))  // covers container startup
    .connect(&url).await?;
```

`PgPool` is a cheap clone (Arc inside) — share it, never rebuild per request.

### Compile-time checked queries

```rust
sqlx::query!(r#"INSERT INTO tasks (...) VALUES ($1, $2)"#, ...)
let row = sqlx::query_as!(TaskRow, r#"SELECT id, title FROM tasks WHERE id = $1"#, id)
    .fetch_optional(&pool).await?;
```

Nullability overrides when inference can't know:
`SELECT status as "status!: text"`, `"col?"` for nullable. Enums/values cross
as `String` + `parse` on read, keeping domain crate sqlx-free.

### Offline workflow (.sqlx)

The macros need a live DB *or* a committed cache:

```bash
sqlx migrate run                # schema up to date
cargo sqlx prepare --workspace  # refresh .sqlx/ (COMMIT IT)
SQLX_OFFLINE=true cargo build   # builds without any DB (CI default)
cargo sqlx prepare --check --workspace   # CI guard: cache must match code
```

After *any* SQL or schema change: `prepare` + commit `.sqlx/` together with
the code, or CI fails the check step.

### Transactions

```rust
let mut tx = pool.begin().await?;
sqlx::query!("INSERT ...").execute(&mut *tx).await?;
sqlx::query!("UPDATE ...").execute(&mut *tx).await?;
tx.commit().await?;   // or drop for rollback
```

Keep transactions short; don't `.await` external calls inside.

### Migrations

`migrations/NNNN_name.sql`, forward-only; applied files never edited.
`sqlx::migrate!("../../migrations").run(&pool)` at startup keeps every
environment current. Dev reset (destroys dev data): `docker compose down -v`.

---

## lapin (RabbitMQ) {#lapin}

### Publishing with confirms

```rust
let channel = connection.create_channel().await?;
channel.confirm_select(ConfirmSelectOptions::default()).await?;
let confirmation = channel
    .basic_publish(
        "".into(),                          // default exchange
        routing_key.into(),                 // queue name as routing key
        BasicPublishOptions::default(),
        payload,
        BasicProperties::default().with_delivery_mode(2), // persistent
    ).await?                                // written
    .await?;                                // broker acked (PublisherConfirm)
if !confirmation.is_ack() { /* broker nacked */ }
```

**Declare queues before publishing**: a message routed to a missing queue is
silently dropped — declare all routing keys' queues at publisher startup
(idempotent).

### Consuming

- `basic_consume` → `Consumer: Stream<Item = Result<Delivery>>`; manual acks
  by default — ack **after** processing succeeds
- `basic_qos(50, ...)` before consume: without a prefetch limit the broker
  pushes the whole queue to the first consumer
- Broken message → `nack` with `requeue: false` (poison messages must not
  loop); transient failure → retry/requeue by design
- Container lifetime note: a `Consumer` tied to a dropped `Connection` ends
  the stream — keep the connection alive for the loop's lifetime

### Reconnect strategy

Simple and supervisor-friendly: retry with backoff at startup; on mid-run
connection loss, let the process exit and let the orchestrator restart it.
Auto-reconnect inside lapin is experimental — avoid for v1.

---

## redis {#redis}

```rust
let client = redis::Client::open(url)?;                  // no I/O
let conn = client.get_connection_manager().await?;       // connects; auto-reconnects
```

- `ConnectionManager` multiplexes; `clone()` shares the connection — pass
  clones around instead of locks
- Features: `tokio-comp` + `connection-manager`
- Prefer explicit `redis::cmd("INCR").arg(key).query_async::<i64>(&mut conn)`
  over trait-inferred methods when unsure of the exact async trait surface
  (it moved across 0.x → 1.x)
- After a connection error, one command fails while the manager reconnects
  in the background — retry once on connection-class errors

---

## tracing {#tracing}

```rust
tracing_subscriber::fmt()              // pretty in debug
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .init();                           // exactly once per process; second call panics
// release: .json().flatten_event(true) for log aggregators

#[tracing::instrument(skip(self, payload), fields(task_id = %id))]
pub async fn process(&self, id: Uuid, payload: &Bytes) { ... }

tracing::info!(count, task_id = %event.task_id, "processed");
```

- Structured fields over message interpolation — `info!(user_id = %id, ...)`
- `instrument` on service methods; `TraceLayer` (tower-http) for HTTP spans
- Never log secrets: `SecretString` doesn't implement `Debug` output of the
  inner value — keep it that way
- OTLP export: opentelemetry-otlp + tracing-opentelemetry as an optional
  feature, enabled by `OTEL_EXPORTER_OTLP_ENDPOINT`; the OTel crate family
  renumbers majors frequently — verify pairing via docs before use

---

## Testing {#testing}

### testcontainers

```rust
async fn start_postgres() -> (DatabaseSettings, ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    (settings_for(port), container)   // container must be returned and kept alive!
}
```

- Modules: `testcontainers-modules` features `postgres`, `redis`, `rabbitmq`
- Default credentials per module (postgres/postgres, guest/guest) — build
  URLs from the mapped port
- Mark such tests `#[ignore = "requires docker (testcontainers)"]`; CI runs
  `cargo test -- --ignored` (ubuntu runners have docker)

### serde/HTTP handler tests

`tower::ServiceExt::oneshot` against the real router with fakes injected:
status + JSON body assertions, no sockets. `http-body-util`'s `BodyExt` to
read response bodies.

### Property testing

`proptest` for parsers/invariants (round-trips, invariants over random
inputs); regular table tests for business rules. Don't property-test I/O.

---

## Tooling {#tooling}

| Tool | Use |
|---|---|
| `cargo watch -x check` / `bacon` | loop on save while developing |
| `cargo expand` | see what macros (derive, sqlx, async_trait) generate |
| `cargo tree -i <crate>` | why is this dependency here / duplicated |
| `cargo audit` / `cargo deny` | vulnerable / duplicated / licensed deps |
| `cargo outdated` | dependency freshness report |
| `criterion` | statistically honest benchmarks in `benches/` |
| `just` | task runner: setup/check/test-int/run as project entry points |
| `sqlx-cli` | migrations + `prepare`/offline cache |

Benchmarks:

```rust
// benches/parse.rs  +  [[bench]] name = "parse" harness = false
fn bench_title(c: &mut Criterion) {
    c.bench_function("title/new", |b| b.iter(|| Title::new(black_box("x".into()))));
}
```

---

## Debugging {#debugging}

- `RUST_BACKTRACE=1` (or `full`) for panics; `RUST_LOG=debug,my_crate=trace`
  to zoom EnvFilter-based logs
- Panic in async: the task dies, the runtime lives — look for the JoinError
  or the missing `.await` on a JoinHandle
- Deadlock smells: lock held across `.await`; `block_on` inside async
  context (forbidden); blocking syscall in a worker thread (see
  `tokio-console` for task inspection)
- "cannot borrow as mutable" while iterating: the iterator borrows the
  collection — collect first, or use `retain`/`drain`/index loops over keys
- sqlx "mismatched types ... INT4 vs INT8": Postgres `SELECT 1` is i32;
  annotate or cast (`1::bigint`)
- Compile-time SQL errors: schema drift — `sqlx migrate run`, then
  `cargo sqlx prepare --workspace`
