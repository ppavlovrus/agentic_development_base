# Рецепт: тесты

Три уровня; выбирай минимально достаточный.

## 1. Unit (domain, app) — офлайн, без I/O

- **domain**: правила и value-объекты. Смотри `entities/task.rs` (Title),
  `entities/task_status.rs` (переходы): `#[cfg(test)] mod tests` прямо в
  файле,AAA, табличные случаи в цикле по кортежам.
- **app**: сервисы на fakes. Fakes — вручную написанные реализации трейтов в
  `crates/app/src/fakes.rs` (`InMemoryTaskRepository`, `CapturingPublisher`),
  переиспользуются тестами всех крейтов. Никакого mockall.
- В тестах разрешён `unwrap()` (в крейтах стоит `cfg_attr(test, allow(...))`).

## 2. API unit (server) — router на fakes, сеть не нужна

`crates/server/tests/api.rs`: `build_router` на fakes + `router.oneshot(req)`
(tower `ServiceExt`), тело читается через `http_body_util::BodyExt`.
Health для таких тестов подменяется заглушкой `HealthProbe`.

## 3. Integration — реальные PG/Redis/RabbitMQ в testcontainers

`crates/server/tests/integration.rs`. Каждый тест помечен:

```rust
#[tokio::test]
#[ignore = "requires docker (testcontainers)"]
```

поэтому обычный `cargo test` остаётся офлайн и быстрым. Запуск:
`just test-int` (CI запускает их всегда). Контейнеры одноразовые, с
уникальными портами; миграции применяются к контейнеру напрямую.

Что покрывать интеграционно (по приоритету):

1. сквозной сценарий среза: create → persisted → событие дошло → обработано;
2. сборка connection-string / URL из настроек (дешёвая защита от регрессий
   конфигурации);
3. не покрывать: чистую логику domain/app — она закрыта unit-тестами.

## Правила

- AAA: Arrange-Act-Assert, один сценарий — один тест.
- Падения проверяются `matches!(err, ...)` / `assert!(matches!)`, не строками
  сообщений (строки = хрупкость).
- Тест не должен быть зелёным, если тестируемый код вернул пустоту:
  проверяй содержимое, не только статус.
- Мокаются только внешние границы (трейты domain), не внутренности.
