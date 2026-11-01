# Canonical tasks. `just` with no arguments lists them all.
default: --list

# One-time bootstrap: .env, infrastructure containers, database schema
setup:
    @[ -f .env ] || cp .env.example .env
    docker compose up -d --wait
    sqlx migrate run

# Start infrastructure and wait until healthy
up:
    docker compose up -d --wait

# Stop infrastructure (volumes are kept)
down:
    docker compose down

# Tail infrastructure logs
logs:
    docker compose logs -f

# Quality gate: formatting + lints + unit tests. Run before every commit.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Unit tests only (offline, no docker needed)
test:
    cargo test --workspace

# Integration tests (requires docker; spins throwaway containers)
test-int:
    cargo test --workspace -- --ignored

lint:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

# Run the HTTP API
run:
    cargo run -p server

# Run the background worker
run-worker:
    cargo run -p worker

# Apply pending SQL migrations
migrate:
    sqlx migrate run

# Regenerate the compile-time SQL cache after any SQL or schema change;
# commit the updated .sqlx/ directory together with your code
sqlx-prepare:
    cargo sqlx prepare --workspace
