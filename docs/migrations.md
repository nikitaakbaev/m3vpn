# Миграции настроек и схемы

## Принципы

- Пользовательские настройки не перезаписываются при обновлении.
- Новые ключи добавляются через `INSERT OR IGNORE` в `store::seed`.
- Новые колонки добавляются через `ensure_column` с проверкой
  `pragma_table_info`.
- Схема помечается `PRAGMA user_version`; текущая версия — `2`.

## История

| Версия | Изменения |
|---|---|
| 1 | Базовые таблицы, серверы, routing, ping samples |
| 2 | `servers.favorite`, `server_tags`, split tunneling settings, traffic guard |

## Обновление

1. Увеличить `SCHEMA_VERSION` в `store::migrate`.
2. Добавить идемпотентный шаг (`CREATE TABLE IF NOT EXISTS`,
   `ensure_column`, `INSERT OR IGNORE`).
3. Добавить тест на старую БД, если шаг меняет данные.
4. Не удалять старые настройки без явной миграции.

Favorites, tags, ping history and split tunneling settings survive reinstall:
they live in `~/.local/share/m3vpn/m3vpn.db`.
