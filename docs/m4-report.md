# M4 — производительность, AmneziaWG, sing-box и RTT

Дата: 2026-11-24
Статус: готово (hot-switch и TUN вынесены в следующий этап)

## 1. Исправление тормозов

Жалоба: приложение зависало на несколько секунд. Причины были архитектурные,
а не в React:

- `import_subscription`, `refresh_profile`, `probe_servers`, `connect_server`,
  `disconnect_core`, `core_info` были **синхронными Tauri-командами**. Они
  выполняли HTTP, DNS, запуск Xray и ожидание порта до 6 секунд прямо в
  обработчике команды, из-за чего UI замирал.
- `refreshData` при каждом пинге заново вызывал `core_info`, а тот запускал
  `xray version` отдельным процессом.
- `App` подписывался на весь объект `status`, который обновляется каждые 2 секунды,
  и перерисовывал всё дерево.
- `ServersPage` перерисовывал 43 строки на каждом тике статуса.
- `backdrop-filter: blur(18px)` на навигации и топбаре давал лишнюю нагрузку
  WebKitGTK при скролле.

Что сделано:

- Все тяжёлые команды переведены на `tauri::async_runtime::spawn_blocking`.
- `core_info` убран из `refreshData` и вызывается один раз при загрузке.
- `App` больше не подписан на `status`; в нём только `isConnected` и `serverCount`.
- `TopbarStatus` — отдельный компонент с примитивными селекторами.
- `ServersPage` использует `connectedServerId`, а не весь статус.
- `SettingsPage` использует `isConnected`.
- Автопинг больше не спамит snackbar и не запускается во время connect.
- Убраны `backdrop-filter` с nav/topbar.
- Автопинг теперь silent, с гистерезисом 15 % для авто-выбора.

Замер после исправления: idle CPU **1.5–2.0 %**, RSS ~190 МБ, без долгих
зависаний при старте и фоновом пинге.

## 2. AmneziaWG

### Парсер

`crates/parser/src/awg.rs`:

- `.conf` (`[Interface]`, `[Peer]`, `[Device]`);
- `Jc/Jmin/Jmax`, `S1–S4`, `H1–H4`, `I1–I5`;
- AmneziaWG 3.x: `HeaderProtectionKey`, `ContentPaddingAddition`,
  `Rekey*`, `RejectAfterTime`, `KeepaliveTimeout`, `MaxHandshakeAttempts`,
  `RandomTrailers`, `DisableCookies`;
- base64/hex ключи, диапазоны, булевы значения;
- `amneziawg://` / `awg://` ссылки;
- raw `.conf` в теле подписки;
- JSON-контейнер `{ "type": "amneziawg", "servers": [...] }`.

Парсер строит готовый **sing-box** конфиг с endpoint `type: "awg"`.

### sing-box core

- `core/singbox.rs` патчит mixed inbound, порт и Clash API.
- `Sidecars` знает `hiddify-sing-box`, проверяет бинарник и версию.
- `core::connect_singbox` запускает `hiddify-sing-box run -c`, ждёт SOCKS-порт,
  использует `PR_SET_PDEATHSIG`, пишет лог в `core/singbox.log`.
- `connect_server` выбирает движок по протоколу: `awg` → sing-box,
  остальное → Xray.
- `ConnectionStatus.engine` показывает `xray` или `sing-box` в UI.

### Импорт в UI

Диалог подписки принимает не только URL, но и:
- содержимое `.conf`;
- ссылку `amneziawg://` / `awg://`.

Такие профили сохраняются без URL (обновление скрыто), сервер сразу доступен
для подключения.

E2E-тест с локальным AmneziaWG-сервером: импорт `.conf` → sing-box → `curl`
через SOCKS → `loc=...` — OK.

## 3. Реальный RTT

- Команда `connection_rtt` измеряет HTTP 204 через локальный SOCKS с
  `socks5h://127.0.0.1:<port>`.
- Home показывает `RTT` для активного соединения, обновление раз в 10 секунд.
- Если RTT недоступен, показывается TCP-пинг выбранного сервера.
- В деталях соединения появился движок (`xray` / `sing-box`).

## Проверки

| Проверка | Результат |
|---|---|
| `m3vpn-parser` tests | 14/14 |
| Rust tests | 10/10 + 3 ignored |
| AWG E2E с реальным сервером | OK |
| RTT через локальный Xray | OK |
| `cargo clippy -D warnings` (app + parser) | OK |
| `npm run typecheck` | OK |
| `npm run build` | OK |
| `tauri build --debug --no-bundle` | OK |
| Runtime smoke + CPU | 1.5–2.0 % idle |
| `check-sidecars.sh` | OK |

## Ограничения

- **Hot-switch не сделан.** Полные Xray-конфиги по-прежнему требуют перезапуска
  ядра при смене сервера. Для настоящего hot-switch нужен постоянный front-proxy
  и drain старого Xray.
- **TUN не подключён.** TUN остаётся отключённым в UI с пояснением.
- RTT измеряется через выбранный сервер, но не для каждого сервера отдельно.
- Автовыбор по TCP-пингу имеет гистерезис 15 %, но живое переключение во время
  активного соединения не выполняется.

## Следующий этап — M5

1. Постоянный front-proxy (sing-box mixed inbound) + внутренний Xray upstream:
   hot-switch новых соединений без падения слушателя.
2. Drain старого Xray перед остановкой.
3. TUN через helper + snapshot/restore.
4. RTT по каждому серверу через sing-box URLTest/Clash API.
