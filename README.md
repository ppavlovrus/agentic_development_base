# agentic_development_base

The agent-development base for Rust: everything an AI agent needs to run
Rust projects the way the Python stack works with skills and contracts —
not a one-off scaffold, but a reusable harness.

Two components:

- **`skills/rust-development/`** — the universal Rust skill (installed into
  `~/.zcode/skills/`, loaded by agents on every Rust task): agent workflow,
  crate/API verification, async, ownership, traits, error handling, testing,
  style gates — plus a deep `reference.md` (tokio, sqlx, lapin, redis,
  tracing).
- **`templates/rust-service/`** — a working, fully tested service template
  (axum + PostgreSQL/sqlx + Redis + RabbitMQ, 5-crate clean-architecture
  workspace, compile-time checked SQL, testcontainers integration tests,
  CI). Copy it to start a new service project in minutes.

## Install the skill

```bash
ln -s ~/RustProjects/agentic_development_base/skills/rust-development \
      ~/.zcode/skills/rust-development
```

This repo stays the source of truth: edit the skill here, commit; the
symlink picks changes up on the next session.

## Third-party skills

Some deep topics are covered by external skills instead of ours — installed
outside this repo (like cargo dependencies, updated via `git pull`), never
vendored in:

```bash
# postgresql-best-practices (microsoft/postgres-skills, MIT)
git clone --depth 1 https://github.com/microsoft/postgres-skills ~/agent-skills/postgres-skills
ln -s ~/agent-skills/postgres-skills/plugin/skills/postgresql-best-practices \
      ~/.zcode/skills/postgresql-best-practices
```

Cross-linked from `skills/rust-development/reference.md` (#sqlx) and the
template's `skills/sql-and-migrations.md`.

## Start a new service project

```bash
cp -r ~/RustProjects/agentic_development_base/templates/rust-service ~/RustProjects/<new-project>
cd ~/RustProjects/<new-project>
cp .env.example .env
just setup && just check && just run
```

Details, conventions and task recipes live in
[`templates/rust-service/AGENTS.md`](templates/rust-service/AGENTS.md) —
that contract applies inside every project created from the template.

## Repo map

```
skills/rust-development/   universal Rust skill (SKILL.md + reference.md)
templates/rust-service/    service template (see its own AGENTS.md)
.github/workflows/ci.yml   CI for the template (working-directory based)
```
