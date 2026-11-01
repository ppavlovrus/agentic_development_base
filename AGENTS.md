# AGENTS.md — контракт для AI-агентов

Шаблон production-сервиса на Rust: **axum + PostgreSQL (sqlx) + Redis + RabbitMQ**, Clean Architecture в cargo workspace из 5 крейтов. Сценарий использования: клонировал → переименовал → разрабатываешь (агентом или сам).

## Карта репозитория

```
crates/domain    — чистая бизнес-логика: entities, трейты репозиториев, события, ошибки
crates/app       — use cases: сервисы, оркестрация над трейтами domain
crates/infra     — реализации трейтов: sqlx/redis/lapin; конфигурация, телеметрия
crates/server    — HTTP API (axum), composition root; миграции применяются на старте
crates/worker    — фоновые потребители RabbitMQ; composition root
migrations/      — sqlx-миграции (только вперёд)
.sqlx/           — offline-кэш compile-time SQL (коммитится!)
skills/          — рецепты типовых задач (читай по необходимости, не всё сразу)
docker-compose.yml, .env.example, justfile, .github/workflows/ci.yml
```

Живой пример сквозного среза — **Task**: `migrations/0001_create_tasks.sql` → `domain/src/entities/task.rs` → `app/src/services/task_service.rs` → `infra/src/database/task_repository.rs` → `server/src/api/handlers.rs` → `worker/src/main.rs`. Не понимаешь, как что-то устроено — сначала смотри, как то же самое сделано для Task.

## Границы слоёв (проверяет компилятор, не честное слово)

- `domain` зависит только от uuid / chrono / serde / thiserror / async-trait. Импорт sqlx, redis, lapin или axum в domain **не скомпилируется** — это главное правило шаблона, не обходи его.
- Зависимости направлены только внутрь: `domain ← app`, `domain ← infra`, `app + infra ← server/worker`.
- `app` не знает о SQL, HTTP и AMQP — только трейты из domain.
- Бинарные крейты (server, worker) — тонкие composition roots: собрать зависимости, запустить, обработать graceful shutdown.

## Цикл проверки перед «готово»

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # юнит, офлайн
```

- Трогал SQL или миграции → `just sqlx-prepare` и закоммить обновлённый `.sqlx/` (иначе CI падает на `cargo sqlx prepare --check`).
- Трогал infra / интеграционные сценарии → `just test-int` (нужен запущенный docker).
- `just check` = fmt + clippy + unit; `just setup` поднимает .env, контейнеры и схему БД.

## Типовые задачи → рецепты в skills/

| Задача | Рецепт |
|---|---|
| Новая сущность (мигpация→domain→repo→сервис→endpoint→тесты) | `skills/add-entity.md` |
| Новый HTTP-endpoint | `skills/add-endpoint.md` |
| Новый consumer в worker | `skills/add-consumer.md` |
| SQL, миграции, `.sqlx` | `skills/sql-and-migrations.md` |
| Тесты: unit / integration / fakes | `skills/testing.md` |

## Соглашения

- **Ошибки**: `thiserror` во всех библиотечных крейтах; `anyhow` — только в `main()` бинарей. `unwrap()` в либах запрещён (`clippy::unwrap_used = warn`, в CI `-D warnings`); в тестах — можно (там `#![allow]` уже стоит).
- **Валидация на границах**: некорректные значения невыразимы — value-объекты с конструкторами в domain (`Title::new`), `parse` для строк извне (`TaskStatus::parse`); вход HTTP дополнительно проверяется serde-десериализацией.
- **Данные**: id — `Uuid::now_v7()`, время — `DateTime<Utc>`, в PostgreSQL — `TIMESTAMPTZ`; статусные перечисления в БД — TEXT + parse на чтении (domain не знает sqlx).
- **Секреты** — только через `secrecy::SecretString`; никогда не в логи и не в ошибки.
- **Миграции** — только вперёд; применённый файл не правится, добавляется новый.
- **SQL** — только compile-time макросы `sqlx::query!` / `query_as!` (offline-кэш `.sqlx` коммитится).
- **События** — JSON; routing key из `domain::events::routing_keys` он же имя очереди; новую очередь декларируй и в `RabbitEventPublisher::new`, и consumer получает её через `messaging::consumer_for`.
- **Новые зависимости** — только при реальной необходимости; версию фиксируй в `[workspace.dependencies]` корня.
- Комментарии в коде и commit messages — на английском, императив.
- Осознанные упрощения помечай в коде комментарием `shortcut: <ограничение>, <когда наращивать>`.

## Известные упрощения (не чини молча — обсуди с владельцем)

- Публикация событий без outbox: при недоступном брокере после успешного INSERT событие теряется (`app/src/services/task_service.rs`).
- Reconnect-ретраи только на старте; потеря соединения в рантайме завершает процесс — рестартует супервизор (`infra/src/messaging.rs`).
- CORS fully permissive (`server/src/lib.rs`) — перед продом ограничить origins.
- OTLP-трейсинг — опциональная фича `infra/otlp`, по умолчанию выключена.

## Команды

`just` (список) · `just setup` · `just check` · `just test` · `just test-int` · `just run` · `just run-worker` · `just migrate` · `just sqlx-prepare` · `just down`
