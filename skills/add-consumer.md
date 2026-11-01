# Рецепт: добавить consumer в worker

Референс: обработка `task.created` в `crates/worker/src/main.rs` +
`infra::messaging`.

## Шаги

1. **Routing key** уже объявлен в `domain::events::routing_keys` (событие
   публикует сервис). Очередь с этим именем должна быть декларирована в
   `RabbitEventPublisher::new` (`crates/infra/src/messaging.rs`) — обычно уже
   сделано на шаге add-entity.

2. **Функция обработки** в worker (тип события — из `domain::events`):

   ```rust
   async fn handle_note_created(
       delivery: &lapin::message::Delivery,
       redis: &RedisClient,
   ) -> Result<i64, ProcessingError> {
       let event: NoteCreated = serde_json::from_slice(&delivery.data)?;
       let count = redis.incr("stats:notes_created").await?;
       tracing::debug!(note_id = %event.note_id, count, "note.created");
       Ok(count)
   }
   ```

   Если обработке нужны данные из PostgreSQL — инжектируй репозиторий тем же
   способом, как в `crates/server/src/main.rs` (Arc<dyn Trait>).

3. **Подключение в main**: отдельный consumer и `CancellationToken` на каждый
   поток обработки; независимые обработки можно развести по `tokio::select!`
   веткам или в отдельные задачи (`tokio::spawn` + свой select-цикл).

4. **Правила надёжности** (уже зашиты в шаблон, сохраняй их):
   - ack — только после успешной обработки;
   - ошибка обработки → `messaging::reject` (nack без requeue), иначе
     «отравленное» сообщение крутится вечно;
   - select на `shutdown.cancelled()` — graceful shutdown обязателен.

## Тест

Интеграционный тест уже содержит образец consume+ack
(`crates/server/tests/integration.rs`, `task_lifecycle_end_to_end`): публикуй
событие сервисом, прими `consumer.next()` с `tokio::time::timeout`, проверь
payload.

## Проверка

```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
just run-worker   # ручной smoke: создай сущность через API и смотри логи
```
