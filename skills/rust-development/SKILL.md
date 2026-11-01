---
name: rust-development
description: Guide Rust development using modern best practices including agent workflow (crate/API verification, cargo quality gates), tokio async patterns, ownership and borrowing, trait design, error handling with Result/thiserror/anyhow, and testing with cargo test and testcontainers. Use when writing Rust code, bootstrapping cargo projects, choosing crates, writing async code, implementing error handling or tests, or when the user asks about Rust best practices, code quality, or modern Rust features.
---

# Modern Rust Development

Comprehensive guide for writing high-quality Rust code with an agent-first
workflow: verify before you trust your memory, let the compiler enforce
invariants, gate every change with fmt/clippy/test.

## Quick Start

When writing Rust code, follow these priorities:

1. **Make illegal states unrepresentable** — types, `Result`/`Option`, newtype validation; the compiler is the first reviewer
2. **Errors**: `thiserror` in libraries, `anyhow` in binaries, `?` everywhere else
3. **Async**: tokio for I/O-bound work; never block the runtime
4. **Tests**: unit tests inline (`#[cfg(test)]`), integration in `tests/`, fakes hand-written
5. **Gate**: `cargo fmt` + `cargo clippy -D warnings` + `cargo test` before every "done"

## Agent Workflow

### The verify loop

Rust's compiler gives the tightest feedback loop of any mainstream stack —
use it as the source of truth:

```bash
cargo check                  # fast type/borrow errors
cargo clippy --workspace --all-targets -- -D warnings   # idioms enforced
cargo test                   # behavior
```

Fix the *cause* the compiler points at, not the symptom (don't `clone()` your
way past a borrow error without understanding why it happened). Two failed
fixes built on the same assumption mean the assumption is wrong — restate it
in one sentence and attack that.

### Crates move fast: never trust memory for versions or APIs

Rust ecosystem crates break APIs across minor-looking version bumps. Before
using any crate you haven't verified **this session**:

```bash
cargo search <crate>          # current version from crates.io
```

Then verify signatures against live docs (Context7 MCP: resolve library id →
query docs; 1–3 narrow queries per topic). Real breakage caught in practice
on this stack:

- `lapin` 2 → 4: string args need `.into()` (`ShortString`), `Delivery` moved to `lapin::message::Delivery`, `with_persistent` → `with_delivery_mode`
- `sqlx` 0.7 → 0.9: runtime features split (`runtime-tokio` + `tls-rustls`), macros behind the `macros` feature
- `redis` 0.24 → 1.x: module layout around `aio` reworked
- `secrecy` 0.8 → 0.10: `SecretString::expose_secret()` returns `&str`, not `&String` (`.as_str()` on it hits an unstable feature)

Feature flags are part of the API: `sqlx` without `runtime-tokio` panics at
first await, `lapin` picks its executor by feature. Prefer
`cargo add <crate> --features ...` over hand-writing versions.

### Project bootstrap

- **Single binary/library** — `cargo new`; keep `src/` flat until it hurts.
- **Several binaries + shared core** — cargo workspace with thin `[workspace.dependencies]` (path deps for internal crates, shared versions for external).
- **A service with HTTP + PostgreSQL/Redis/RabbitMQ** — don't scaffold by hand: copy `~/RustProjects/agentic_development_base/templates/rust-service` (verified 5-crate template, compile-time checked SQL, testcontainers tests) and follow its own AGENTS.md.

Pin the toolchain (`rust-toolchain.toml`), use edition 2024, commit
`Cargo.lock` for applications.

**For workspace layout details and feature-flag pitfalls**, see [reference.md](reference.md#workspace)

---

## Async Programming (tokio)

### When to Use Async

✅ **Use async for:**
- Network I/O (HTTP services, database pools, message brokers)
- Many concurrent connections or tasks
- Timers, timeouts, multiplexed protocols

❌ **Don't use async for:**
- CPU-bound work (use rayon or plain threads)
- Simple sequential scripts and CLI tools
- Code that doesn't involve I/O

### Essential Patterns

**Reuse expensive clients and pools** — a connection pool or channel created
once and shared (`Arc`/clone — most clients are cheaply cloneable handles to
an internal pool). Creating a client per request is the classic anti-pattern:

```rust
// ✅ Pool built once at startup, cloned per task (PgPool is an Arc inside)
let pool = PgPoolOptions::new().max_connections(5).connect(&url).await?;
let state = AppState { pool: pool.clone() };

// ❌ New connection per request
async fn handler() -> Json<Out> {
    let pool = PgPoolOptions::new().connect(&url).await.unwrap(); // per request!
    ...
}
```

**Never block the executor thread** — inside async code, never call `std::fs`,
`thread::sleep`, `reqwest::blocking`, or heavy CPU work:

```rust
// ✅ Async-native equivalents
let data = tokio::fs::read("config.json").await?;
tokio::time::sleep(Duration::from_secs(1)).await;
let digest = tokio::task::spawn_blocking(|| sha256(big_blob)).await??; // CPU-bound

// ❌ Freezes every task on the worker thread
let data = std::fs::read("config.json")?;
std::thread::sleep(Duration::from_secs(1));
```

(`spawn_blocking` returns `JoinError` from `.await` *and* your inner `Result`
— note the double `?` or flatten explicitly.)

**Concurrent operations** — join futures, don't await sequentially:

```rust
let (postgres_ok, redis_ok) = tokio::join!(probe_postgres(&pool), probe_redis(&redis));
```

**Cancellation safety in `select!`** — every branch of `tokio::select!` may be
cancelled at any await point. Prefer cancel-safe operations (channels,
timeouts) or make cancellation explicit with `CancellationToken`; drain/cleanup
happens after the loop, not inside the cancelled branch.

**Graceful shutdown** — services must stop on SIGTERM (containers) and
Ctrl-C, and finish in-flight work:

```rust
let token = CancellationToken::new();
let child = { let t = token.clone(); tokio::spawn(async move {
    sigterm_or_ctrl_c().await; t.cancel();
})};
loop {
    tokio::select! {
        _ = token.cancelled() => break,
        msg = consumer.next() => process(msg).await?,
    }
}
```

**For channels, mutexes (std vs tokio), spawn rules, timers**, see [reference.md](reference.md#tokio)

---

## Ownership and Borrowing in Practice

### Borrow at the boundaries, own inside

```rust
// ✅ Take &str for reads; the caller keeps ownership
fn greeting(name: &str) -> String { format!("hello {name}") }

// Store only what you outlive: &str in structs needs lifetimes —
// usually a sign the struct should own (String) or borrow shorter.
struct User { name: String }
```

Rules of thumb:
- Function parameters: `&str` over `&String`, `&[T]` over `&Vec<T>`, `impl Iterator` over collections
- Struct fields: own (`String`, `Vec`), unless caching borrows is measured
- `.clone()` is fine when data genuinely needs two owners; a clone per loop
  iteration to appease the borrow checker means the design is wrong

### Newtype validation — illegal states unrepresentable

```rust
pub struct Title(String);

impl Title {
    pub fn new(raw: String) -> Result<Self, TaskError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() { return Err(TaskError::EmptyTitle); }
        Ok(Self(trimmed.to_owned()))
    }
}
// A `Title` in a signature guarantees a non-empty, trimmed value;
// re-validate on the way *in* from storage or the wire, trust inside.
```

**For lifetimes beyond the basics, Rc/Arc/Box/Cow, when to reach for each**, see [reference.md](reference.md#ownership)

---

## Traits and Abstraction

### Traits as dependency boundaries

Express ports as traits; implement them at the edges (the same idea as
Python's `Protocol`). This is what makes unit tests trivial and layers
replaceable:

```rust
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, RepositoryError>;
}

pub struct TaskService {
    repository: Arc<dyn TaskRepository>,   // injected, fake in tests
}
```

### dyn vs impl Trait

- `impl Trait` in return position: zero-cost, one concrete type — prefer for helpers
- `Arc<dyn Trait>` in struct fields: runtime dispatch, swappable — the norm for injected dependencies
- `Send + Sync` bounds: required on anything shared across await points or
  threads; add them at the boundary (`Box<dyn Error + Send + Sync>`), not
  sprinkled through

Keep trait method count small (interface segregation); split god-traits.

**For object safety rules and async-trait trade-offs**, see [reference.md](reference.md#traits)

---

## Error Handling

### thiserror in libraries, anyhow in binaries

```rust
// Library/domain: typed, matchable errors
#[derive(Debug, thiserror::Error)]
pub enum TaskServiceError {
    #[error("task not found: {0}")]
    TaskNotFound(Uuid),
    #[error(transparent)]
    InvalidTask(#[from] TaskError),
    #[error(transparent)]
    Storage(#[from] RepositoryError),
}

// Binary main(): free-form context chains
fn main() -> anyhow::Result<()> {
    let settings = Settings::load().context("load settings")?;
    ...
}
```

### Propagation and boundaries

- `?` everywhere; convert driver/protocol errors into your error types with
  `#[from]` at the layer boundary — inner code never sees `sqlx::Error`
- Translate storage errors into domain terms (`UniqueViolation` → `Conflict`)
  at the repository implementation, not at call sites
- `unwrap()`/`expect()` forbidden in library code (enforce with
  `clippy::unwrap_used = "warn"` + CI `-D warnings`); `expect` with a reason
  is acceptable in binaries for truly impossible states
- Log errors once, at the boundary that handles them — don't log-and-return
  the same error up the stack

```rust
// ✅ HTTP layer maps domain errors to status codes in one place
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            ApiError::NotFound => StatusCode::NOT_FOUND,          // 404
            ApiError::Validation(_) => StatusCode::UNPROCESSABLE,  // 422
            ApiError::Conflict(_) => StatusCode::CONFLICT,         // 409
            _ => StatusCode::INTERNAL_SERVER_ERROR,                // 500
        };
        ...
    }
}
```

---

## Testing

### Layout

| Kind | Location | Runs |
|---|---|---|
| Unit (pure logic) | `#[cfg(test)] mod tests` in the same file | every `cargo test` |
| Integration (crate API) | `tests/*.rs` | every `cargo test` |
| Integration (real infra) | `tests/*.rs` + `#[ignore]` + testcontainers | `cargo test -- --ignored` / CI |

`#[ignore = "requires docker (testcontainers)"]` keeps plain `cargo test`
offline and fast; CI runs the ignored ones explicitly.

### Essential Patterns

**Plan scenarios before code** — happy path, edge cases (empty, boundary
values), error paths; then write the test.

**AAA and naming** — `test_<unit>_<scenario>_<expected>`:

```rust
#[test]
fn title_rejects_whitespace_only_input() {
    let err = Title::new("   ".to_owned()).expect_err("whitespace-only is invalid"); // Arrange+Act
    assert_eq!(err, TaskError::EmptyTitle);                                          // Assert
}
```

**Table-driven over copy-paste**:

```rust
for (from, to, allowed) in [(Todo, Doing, true), (Doing, Done, true), (Todo, Done, false)] {
    assert_eq!(from.can_transition_to(to), allowed, "{from} -> {to}");
}
```

**Async tests** — `#[tokio::test]` (single-task runtime is enough for most;
`#[tokio::test(flavor = "multi_thread")]` only when spawning blockers).

**Fakes over mocking frameworks** — hand-written in-memory implementations of
your own traits (kept in a shared `fakes` module). Mock only external
boundaries; mocking internals verifies the mock, not the code:

```rust
pub struct InMemoryTaskRepository { tasks: RwLock<Vec<Task>> }

#[async_trait]
impl TaskRepository for InMemoryTaskRepository { /* insert/find/list... */ }
```

**Failure assertions** — `matches!(err, RepoError::Conflict(_))` or
`assert!(matches!(...))`, not message-string matching (brittle).

**testcontainers gotcha** — a container is removed when its handle drops:
helpers must return `(Settings, ContainerAsync<Image>)` and the test keeps
the handle alive until the end.

**For testcontainers setup, sqlx testing, property testing**, see [reference.md](reference.md#testing)

---

## Code Style

### Naming

```rust
struct UserManager;            // types: PascalCase
fn get_user_data() {}          // functions/methods: snake_case
const MAX_RETRIES: u32 = 3;    // constants: SCREAMING_SNAKE
mod task_service;              // modules: snake_case
```

### Modules and docs

- One concern per module; `mod.rs`/`lib.rs` holds only declarations and re-exports
- `///` doc comments on every public item — examples in docs compile as tests
  (`cargo test` runs them); `//` only for the non-obvious *why*
- Formatting: rustfmt defaults, no debate — `cargo fmt --all` in CI
- Lints: centralize in the root manifest so every crate inherits:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
unwrap_used = "warn"
```

---

## Common Patterns

### Serde at the boundaries

```rust
#[derive(Deserialize)]
pub struct CreateUserRequest { pub email: String }      // wire shape ≠ domain type

#[derive(Serialize, Deserialize)]
pub struct TaskCreated { pub task_id: Uuid, pub occurred_at: DateTime<Utc> }  // events
```

Deserialize into request DTOs, convert to domain types (with validation),
never expose domain types directly on the wire.

### Configuration from environment

`APP__SECTION__KEY` convention via the `config` crate (+ `dotenvy` for local
dev, `.env.example` committed, `.env` ignored); secrets as
`secrecy::SecretString` so they can't be logged by accident.

### Time and IDs

- `chrono::{DateTime<Utc>}` / `TIMESTAMPTZ` in Postgres — UTC everywhere, convert at the UI edge
- `uuid::Uuid::now_v7()` for primary keys (time-ordered, index-friendly)

### Compare-and-set for state transitions

Guard read-then-write races at the storage layer:

```sql
UPDATE tasks SET status = $3, updated_at = $4 WHERE id = $1 AND status = $2
-- 0 rows affected => concurrent change => Conflict error, not silent overwrite
```

---

## Anti-Patterns to Avoid

```rust
// ❌ unwrap in library code — panics take down the worker mid-message
let port = env::var("PORT").unwrap();

// ✅ Typed error, handled at the boundary
let port: u16 = env::var("PORT").map_err(|_| ConfigError::Missing("PORT"))?.parse()?;

// ❌ Blocking call inside async — stalls the executor thread
let body = std::fs::read_to_string(path)?;

// ✅ Async or offloaded
let body = tokio::fs::read_to_string(path).await?;

// ❌ Arc<Mutex<>> wrapped around everything "to be safe"
struct AppState { tasks: Arc<Mutex<HashMap<..>>> }

// ✅ Immutable shared state; data lives in the store, channels for mutation
struct AppState { pool: PgPool, service: Arc<TaskService> }

// ❌ pub everywhere "for flexibility"
pub mod internal_guts { pub fn helper(...) }

// ✅ Narrow API; pub only at module boundaries that mean it

// ❌ .clone() to silence the borrow checker without understanding
let name = user.name.clone();

// ✅ Borrow where lifetimes allow; clone where two owners genuinely exist
greet(&user.name);
```

---

## Performance

- Prefer iterators and `&str` slices over intermediate `Vec<String>`/`String`
- Return `impl Iterator<Item = &T>` instead of collecting `Vec<T>`
- `&format!` only at display edges; `write!` into a buffer in hot paths
- Measure before optimizing: `criterion` benchmarks in `benches/`, `cargo flamegraph` for profiles
- Release profile: `lto = "thin"` is a safe default win

**For benchmarking and profiling setup**, see [reference.md](reference.md#tooling)

---

## Additional Resources

For deeper material, see [reference.md](reference.md):
- [Workspace organization and feature pitfalls](reference.md#workspace)
- [Ownership deep dive: lifetimes, Rc/Arc/Box/Cow](reference.md#ownership)
- [tokio internals: channels, mutexes, cancellation](reference.md#tokio)
- [sqlx: pools, transactions, offline cache, migrations](reference.md#sqlx)
- [lapin/RabbitMQ: confirms, qos, ack policy](reference.md#lapin)
- [redis: ConnectionManager patterns](reference.md#redis)
- [tracing: spans, layers, JSON logs](reference.md#tracing)
- [Tooling: watch, expand, tree, audit, criterion](reference.md#tooling)
- [Debugging: backtraces, logs, common compiler fights](reference.md#debugging)

---

## Checklist

Before committing code, verify:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` green; scenarios planned before writing tests
- [ ] Crate versions/APIs verified via `cargo search` + docs this session (not memory)
- [ ] No `unwrap()`/blocking I/O in async or library code
- [ ] Errors: `thiserror` types in libs, `#[from]` conversions at boundaries, mapped once at the edge
- [ ] New types make invalid states unrepresentable (validated constructors)
- [ ] Secrets handled as `SecretString`, never logged
- [ ] Concurrent state changes use compare-and-set (or equivalent guard)
- [ ] Service handles SIGTERM and drains work before exit
- [ ] Doc comments on new public items; comments explain *why*, not *what*
