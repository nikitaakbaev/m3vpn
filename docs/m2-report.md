# M2 — импорт полных Xray-конфигов и реальное подключение

Дата: 2026-11-24
Статус: готово

## Что сделано

### Backend

- Новый crate `crates/parser` (`m3vpn-parser`):
  - JSON-массив полных Xray-конфигов;
  - одиночный полный конфиг;
  - base64-обёрнутый JSON;
  - определение протокола и endpoint'а;
  - пропуск конфигов с `hysteria`/`tuic` (Xray их не понимает);
  - заметки о отложенных форматах (share links, AmneziaWG).
- `subscription.rs`:
  - HTTP-загрузка подписки с таймаутом и User-Agent `m3vpn/...`;
  - разбор заголовков `profile-title`, `profile-description`, `subscription-userinfo`,
    `profile-update-interval`, `hide-url`, `support-url`;
  - сохранение профиля и серверов в SQLite одной транзакцией;
  - CLI-режим `m3vpn --import <url> [name]`.
- `core/xray.rs`:
  - принудительный `127.0.0.1` для socks/http inbound'ов;
  - подстановка активных портов (10810/10811 при занятых 10808/10809);
  - логи Xray в `~/.local/share/m3vpn/core/xray.log`;
  - добавление `stats`, direct-правила для DNS при observatory+balancer;
  - запись конфига с правами `0600`.
- `core/supervisor.rs`:
  - запуск/остановка Xray;
  - проверка, что порт открылся (до 6 секунд);
  - чтение хвоста лога при падении;
  - `PR_SET_PDEATHSIG` — Xray умирает вместе с UI, не остаётся сиротой;
  - статус: running, server, PID, uptime, socks/http порты, error.
- Новые Tauri-команды: `import_subscription`, `list_profiles`, `list_servers`,
  `delete_profile`, `connect_server`, `disconnect_core`, `connection_status`,
  `core_info`, `open_core_dir`.
- Автоподключение при старте, если включена настройка.

### Frontend

Дизайн переработан по skill `ui-ux-pro-max` (design system сохранён в
`design-system/m3vpn/MASTER.md`):

- Палитра «shield dark + connected green»: фон `#0F172A`, панели `#192134`,
  акцент `#22C55E`, текст `#F8FAFC`, muted `#94A3B8`.
- Шрифт **Inter Variable** (локально, без Google Fonts).
- Material 3 сохранён: dynamic color, shape, segmented, switch, dialog.
- Новый Home: hero-карточка соединения, большая power-кнопка, выбранный сервер,
  трафик, состояние ядра, быстрый выбор серверов, детали соединения.
- Servers: реальный список из SQLite, поиск, группировка по подписке,
  выбор сервера, подключение/отключение, latенси (пока «—»).
- Subscriptions: карточки профилей, трафик и срок, удаление, модальный диалог
  добавления (native `<dialog>` с focus trap и Escape).
- Settings: добавлен раздел «Ядро Xray» (путь, версия, geo, папка ядра).
- Ошибки показываются в диалоге с кнопкой «Логи ядра» (error recovery).
- Mock-слой для браузера: можно смотреть UI без Tauri.
- Дизайн-токены, адаптив до 560 px, `prefers-reduced-motion`, focus-visible.

### Скриншоты

- [`screenshots/home.png`](screenshots/home.png)
- [`screenshots/servers.png`](screenshots/servers.png)
- [`screenshots/subscriptions.png`](screenshots/subscriptions.png)
- [`screenshots/subscription-dialog.png`](screenshots/subscription-dialog.png)
- [`screenshots/settings.png`](screenshots/settings.png)

## Проверки

| Проверка | Результат |
|---|---|
| `m3vpn-parser` unit tests | 5/5 |
| `cargo test` | 6/6 + 2 ignored |
| E2E: реальная подписка + Xray + curl | OK (~76 с) |
| Supervisor: локальный Xray + SOCKS + curl | OK |
| `cargo clippy --all-targets -D warnings` | OK |
| `npm run typecheck` | OK |
| `npm run build` | OK |
| `tauri build --debug --no-bundle` | OK |
| Runtime smoke с реальной БД | OK |
| `scripts/check-sidecars.sh` | OK |

Импорт реальной подписки:

```
imported 43 servers into PingBooster (skipped 15)
```

Пропущены 15 конфигов с `hysteria` — Xray их не поддерживает. Это ожидаемо и
показывается в UI как «пропущено».

## Что важно знать

- **Переключение сервера сейчас перезапускает Xray.** Hot-switch через Clash API
  будет в M4 на sing-box. Существующие соединения при переключении рвутся —
  это осознанное ограничение M2.
- **Пинг ещё не реализован.** В списке серверов latency = «—».
- **Share-ссылки и AmneziaWG** пока не импортируются. Полные Xray-конфиги — да.
- **TUN** пока не подключён; режим TUN в настройках сохраняется, но соединение
  идёт через локальный SOCKS/HTTP.
- Токен подписки хранится в SQLite на устройстве; в UI URL не показывается.

## Следующий этап — M3/M4

1. Автопинг серверов через локальный SOCKS/Xray и отображение latency.
2. Share-ссылки и обновление подписки без повторного ввода URL.
3. Импорт AmneziaWG `.conf`.
4. Переход на sing-box для генерации конфигов и hot-switch без разрыва.
5. TUN + helper + snapshot/restore.
