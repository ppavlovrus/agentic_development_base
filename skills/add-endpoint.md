# Рецепт: добавить HTTP-endpoint

Референс: `crates/server/src/api/handlers.rs` (Task), роуты — в
`crates/server/src/lib.rs`.

## Шаги

1. **Сценарий в app.** Эндпоинт — тонкая обёртка над методом сервиса
   (`TaskService`). Новое поведение начинается там, не в handler.

2. **DTO** в `crates/server/src/api/handlers.rs`:

   ```rust
   #[derive(Deserialize)]
   pub struct DoThingRequest { /* только то, что приходит извне */ }

   #[derive(Serialize)]
   pub struct ThingResponse { /* плоские типы: String, Uuid, DateTime<Utc> */ }
   ```

   Доменные типы наружу не отдаются — только конверсия `From<...>`.

3. **Handler** — извлекаторы axum (`State`, `Path`, `Json`, `Query`),
   результат — `Result<Json<T>, ApiError>`:

   ```rust
   pub async fn do_thing(
       State(state): State<AppState>,
       Path(id): Path<Uuid>,
       Json(request): Json<DoThingRequest>,
   ) -> Result<(StatusCode, Json<ThingResponse>), ApiError> {
       let thing = state.things.do_thing(id, request.payload).await?;
       Ok((StatusCode::OK, Json(ThingResponse::from(thing))))
   }
   ```

4. **Ошибки → HTTP**: маппинг варианта service-ошибки в `ApiError` —
   в `crates/server/src/api/error.rs` (404 not found, 422 валидация,
   409 конфликт, 503 зависимость недоступна, 500 остальное).

5. **Роут** в `build_router` (`crates/server/src/lib.rs`):

   ```rust
   .route("/things/{id}/do", post(handlers::do_thing))
   ```

   Синтаксис axum 0.8: параметры пути в `{фигурных}` скобках. Все
   бизнес-роуты вкладываются в `.nest("/api", ...)`.

6. **Тест** в `crates/server/tests/api.rs`: собрать router на fakes,
   `router.oneshot(request)`, проверить статус и тело. См. примеры
   `create_task_returns_201_with_task` и `status_transitions_enforced_over_http`.

## Проверка

```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
