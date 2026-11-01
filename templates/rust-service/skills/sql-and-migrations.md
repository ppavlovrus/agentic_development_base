# Рецепт: SQL и миграции

## Миграции

- Каталог: `migrations/`, имя `NNNN_описание.sql` (номер следующий по порядку).
- Создание и применение:

  ```
  sqlx migrate add create_notes      # создаст файл с номером
  sqlx migrate run                   # применить (нужен DATABASE_URL из .env)
  ```

- **Только вперёд**: применённый файл не редактируется — новый файл-фикс.
- Идемпотентные конструкции (`IF NOT EXISTS`) не нужны: sqlx ведёт журнал
  применённых миграций в таблице `_sqlx_migrations`.
- Миграции применяются автоматически на старте server и в интеграционных
  тестах (`infra::database::run_migrations`).

## Compile-time проверенные запросы

В `infra` используются только макросы:

```rust
sqlx::query!(r#"INSERT INTO notes (...) VALUES ($1, $2)"#, ...)
sqlx::query_as!(NoteRow, r#"SELECT ... FROM notes WHERE id = $1"#, id)
```

Макрос проверяет SQL против реальной схемы и типы плейсхолдеров/колонок на
этапе компиляции. NULL-инференс при необходимости — аннотации
`"col!: T"` (not null) / `"col?: T"` (nullable).

## Offline-кэш `.sqlx/` — обязательный workflow

Компиляция с макросами требует либо живую БД (`DATABASE_URL`), либо
закэшированные описания запросов в `.sqlx/`. Кэш коммитится — это позволяет
собирать проект (и CI) без БД.

**После любого изменения SQL или схемы:**

```
just sqlx-prepare        # cargo sqlx prepare --workspace
git add .sqlx            # вместе с кодом!
```

CI проверяет актуальность: `cargo sqlx prepare --check --workspace` —
рассинхрон = красный билд.

## Частые ошибки

- `error: set DATABASE_URL ... or run cargo sqlx prepare` — кэш неполон для
  нового запроса: подними `just up`, выполни `just sqlx-prepare`.
- `column "x" is of type ... but expression is of type ...` — тип плейсхолдера
  не совпал: проверь тип поля (uuid → Uuid, timestamptz → DateTime<Utc>,
  перечисления передаются строкой `as_str()` и парсятся на чтении).
- Изменил миграцию после применения — откат volume в dev: `docker compose down
  -v && just setup` (уничтожит данные dev-БД).
- Не забывай `ensure_queue` для новых routing keys (это не SQL, но ошибка
  того же класса — «забыл сопроводить код сверху»).
