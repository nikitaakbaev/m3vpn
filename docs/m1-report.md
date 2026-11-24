# M1 — каркас приложения

Дата: 2026-11-24
Статус: готово (с двумя отложенными пунктами, см. ниже)

## Что сделано

### Стек

| Компонент | Версия |
|---|---|
| Node.js | 26.13.1 |
| React / React DOM | 19.3.1 |
| Vite | 8.3.8 |
| TypeScript | 5.9.3 |
| Tauri (Rust) | 2.12.7 |
| `@tauri-apps/api` / CLI | 2.12.5 / 2.12.6 |
| Material Symbols | `@material-symbols/font-400` 0.49.0 |
| Material color utilities | 0.4.0 |
| `@material/web` | 2.5.0 (подключён на будущее) |
| rusqlite | 0.43.0 |
| Rust | 1.99.1 |

### Backend (Rust)

- `paths.rs` — XDG-каталог `~/.local/share/m3vpn`, права `0700`.
- `logging.rs` — `tracing` + daily rolling file `logs/m3vpn.log.YYYY-MM-DD`.
- `store.rs` — SQLite, WAL, миграции всех таблиц из плана, seed настроек.
- `port.rs` — поиск свободного TCP-порта на `127.0.0.1` (10808 → 10810 при занятых 10808/10809).
- `commands.rs` — `get_app_info`, `get_settings`, `set_setting`, `pick_free_port`,
  `open_logs_dir`, `log_frontend`.
- `tray.rs` — иконка, меню «Открыть / Скрыть / Выход», левый клик открывает окно.
- `lib.rs` — single instance, close-to-tray, выбор активного порта при старте,
  `umask(0077)` для приватных файлов.

### Frontend (Material 3)

- Dynamic color из seed-цвета (`themeFromSourceColor`), режимы system/light/dark.
- Собственные M3-компоненты: buttons, segmented, switch, fields, cards, pills,
  snackbar, navigation rail, empty states.
- Экраны: Home, Servers, Subscriptions, Routing, Settings.
- Hash-роутинг (`#home`, `#settings`, …) — удобно для скриншотов и deep links.
- Русский и английский языки, переключение на лету.
- Настройки меняют SQLite сразу: тема, seed, язык, режим, порты, пинг, трей.
- В браузере (без Tauri) работает mock-слой, чтобы UI можно было смотреть через Vite.

### Скриншоты

- [`docs/screenshots/home.png`](screenshots/home.png)
- [`docs/screenshots/settings.png`](screenshots/settings.png)

## Проверки

| Проверка | Результат |
|---|---|
| `npm run typecheck` | OK |
| `npm run build` | OK, 342.75 kB JS / 20.44 kB CSS |
| `cargo check` | OK |
| `cargo test` | 3/3 passed |
| `cargo clippy --all-targets -- -D warnings` | OK |
| `npm run tauri build -- --debug --no-bundle` | OK |
| Runtime smoke (`timeout 12s ./target/debug/m3vpn`) | OK |
| `scripts/check-sidecars.sh` | все хеши совпали |
| Автовыбор порта | стартовал на **10810**, потому что INCY держит 10808/10809 |
| Права файлов | `700` каталоги, `600` БД, WAL, SHM и лог |

Лог запуска:

```
INFO m3vpn_lib: m3vpn starting version="0.1.0" ... port=10810
INFO m3vpn_lib: application ready
```

## Решённые проблемы M0

1. **Занятые порты INCY** — реализован динамический выбор свободного порта,
   отображается в Home и Settings, есть кнопка «Подобрать порт».
2. **Права файлов** — процесс выставляет `umask 0077`; БД, WAL/SHM и логи создаются `0600`.
3. **Патч `soar`** — вынесен в `patches/0001-soar-abort-fallback.patch`,
   `scripts/build-sidecars.sh` применяет его через `git apply` (с sed-fallback).
4. **Воспроизводимость ядер** — `scripts/check-sidecars.sh` сверяет sha256 с
   `sidecars/versions.json`; geo-ассеты прибиты к релизу `202611232203`.
5. **Полные Xray-конфиги** — приоритет M8 поднят: это основной формат подписки.
   Отражено в `docs/m0-report.md`.

## Отложено

- **Встроенный просмотрщик логов** — сейчас кнопка открывает папку; viewer
  логичнее сделать вместе с диагностикой ядра в M2/M6.
- **Визуальная проверка трея** — приложение стартует без ошибок, `application ready`
  пишется, но иконку в трее нужно глазами проверить в сессии Hyprland/Waybar.
- **`@material/web`** — зависимость установлена, но M3-компоненты пока собственные;
  миграция на официальные web components не блокирует M2.

## Как запустить

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Или собранный debug-бинарник:

```bash
apps/desktop/src-tauri/target/debug/m3vpn
```

## Следующий этап — M2

1. Парсер share-ссылок (VLESS/VMess/Trojan/SS/SOCKS/HTTP) в `crates/parser`.
2. Генератор sing-box конфига для proxy-режима.
3. Supervisor ядра + Clash API.
4. Импорт из буфера/URL, сохранение серверов.
5. Реальный connect/disconnect и статус ядра в Home.
