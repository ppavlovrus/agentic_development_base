# Рецепт: добавить новую сущность

Полный вертикальный срез по образцу Task. Порядок шагов важен: каждый
следующий опирается на предыдущий. Референс — файлы сущности Task (ищи по
имени `task` в тех же каталогах).

## 0. Предусловия

Инфраструктура поднята: `just setup` (иначе `.sqlx` не сгенерировать).

## 1. Миграция

Создай `migrations/NNNN_create_<entity_plural>.sql` (номер — следующий по
порядку, посмотри каталог). Идемпотентность не нужна: sqlx хранит журнал.

```sql
CREATE TABLE <entities> (
    id          UUID PRIMARY KEY,
    -- ...бизнес-поля TEXT/NUMERIC/BOOL... (значения-перечисления — TEXT)
    created_at  TIMESTAMPTZ NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL
);
```

Применение: `sqlx migrate run` (из корня, `.env` подхватится сам).
Миграции только вперёд; применённый файл не правится.

## 2. domain

- `crates/domain/src/entities/<entity>.rs` — сущность и value-объекты.
  Некорректные значения невыразимы: обёртки с `new() -> Result<_, E>`.
- `crates/domain/src/entities/<entity>_status.rs` — если есть статусный
  автомат: `as_str`, `parse`, `can_transition_to` (+ unit-тесты переходов).
- `crates/domain/src/error.rs` — ошибки валидации (`<Entity>Error`) и
  варианты `RepositoryError`, если нужны новые.
- `crates/domain/src/repositories.rs` — трейт `<Entity>Repository`
  (`insert` / `find_by_id` / `list` / update-методы). Update-методы статусов
  делай compare-and-set (`from`, `to`), образец — `update_status` у Task.
- Необязательно (только если на события сущности реагируют фоновые
  обработчики): `crates/domain/src/events.rs` — событие `<Entity>Created`
  (serde Serialize + Deserialize) и routing key в `routing_keys`; тогда же
  `ensure_queue(...)` в `RabbitEventPublisher::new` (п. 3) и consumer в
  worker (п. 6).

Не добавляй в domain ничего из infra: сборка упадёт — так и задумано.

## 3. infra (PostgreSQL)

`crates/infra/src/database/<entity>_repository.rs` по образцу
`task_repository.rs`:

- raw-структура `<Entity>Row` (перечисления — String) + `TryFrom<Row>`;
- запросы только макросами `sqlx::query_as!` / `sqlx::query!`;
- `map_db_error` для перевода ошибок драйвера в `RepositoryError`;
- объявь модуль в `crates/infra/src/database/mod.rs`.

Обязательно: `just sqlx-prepare` и закоммить обновлённый `.sqlx/`.

Если сущность публикует события — добавь `ensure_queue(...)` для нового
routing key в `RabbitEventPublisher::new` (`crates/infra/src/messaging.rs`),
иначе сообщения в необъявленную очередь теряются молча.

## 4. app

`crates/app/src/services/<entity>_service.rs` по образцу `task_service.rs`:

- ошибки — `<Entity>ServiceError` (thiserror, `#[from]` доменных ошибок);
- методы-сценарии: валидация → repository → publish события;
- unit-тесты на fakes (`app::fakes`, при необходимости добавь fakes новой
  сущности туда же).

## 5. server

- `crates/server/src/api/handlers.rs` (или новый модуль) — DTO
  `Request/Response`, конверсия `From<Entity>`;
- ошибки сценария → HTTP в `crates/server/src/api/error.rs`;
- роуты в `crates/server/src/lib.rs`: `"/<entities>"`,
  `"/<entities>/{id}"` (axum 0.8: параметры в фигурных скобках);
- состояние — в `AppState` и composition root `crates/server/src/main.rs`;
- unit-тест роутера на fakes в `crates/server/tests/api.rs`.

## 6. worker (если событие обрабатывается фоном)

См. `skills/add-consumer.md`.

## 7. Проверка (гейт)

```
just check                 # fmt + clippy -D warnings + unit
just test-int              # если расширял интеграционные сценарии
```

Частые ошибки: забыл `sqlx-prepare` (CI падает на prepare --check); забыл
`ensure_queue`; положил serde-тип API прямо в domain.
