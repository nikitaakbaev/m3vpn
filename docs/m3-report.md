# M3 — пинг, share-ссылки и обновление подписок

Дата: 2026-11-24
Статус: готово

## Что сделано

### 1. Share-ссылки

`crates/parser/src/links.rs`:

- `vless://` — UUID, flow, TLS/Reality, TCP/WS/gRPC/HTTP/xHTTP, sni/fp/pbk/sid/spx;
- `vmess://` — base64 JSON, vnext, ws/grpc/http, TLS;
- `trojan://` — пароль, TLS, транспорты;
- `ss://` — SIP002 и legacy-формат, base64 userinfo;
- `socks://` / `socks5://` — с авторизацией и без;
- `http://` / `https://` — HTTP-прокси с авторизацией;
- `hysteria2`, `tuic`, `wireguard`, `amneziawg`, `ssr` — распознаются и
  пропускаются с понятной заметкой.

Каждая ссылка превращается в полноценный Xray-конфиг (socks inbound,
proxy/direct/block outbounds, routing), поэтому импорт и подключение
работают тем же путём, что и для полных конфигов.

### 2. Обновление подписки

- `subscription::import_into` умеет обновлять существующий профиль по ID.
- Команда `refresh_profile` читает сохранённый URL из SQLite и переимпортирует
  серверы без создания дубликата.
- На карточке подписки появилась кнопка «Обновить».
- `created_at` сохраняется, `last_refresh_at` обновляется.
- Добавлен тест: повторный импорт обновляет профиль на месте.

### 3. TCP-пинг

- Новый модуль `src-tauri/src/ping.rs`.
- Параллельные TCP-подключения к `host:port` каждого сервера
  (concurrency 8, timeout 3 с).
- Результат пишется в `servers.latency_ms`, `latency_ok`, `latency_at`.
- Команда `probe_servers`, CLI `m3vpn --probe`.
- Автопинг каждые `ping.interval_sec` секунд, если включён `ping.enabled`.
- Кнопки в Servers: «Проверить пинг» и «В самый быстрый» (прогнать пинг
  и подключиться к серверу с минимальной задержкой).
- Сортировка списка: группы / по пингу / по имени.
- Цветные бейджи: зелёный < 80 мс, жёлтый < 200, оранжевый < 500, красный > 500.

На реальной подписке:

```
probed 43 servers, 36 responded
```

Лучшие результаты: Finland reserve 23 мс, France reserve 24 мс,
Kazakhstan reserve 24 мс, Germany/France/Finland 29–31 мс.

### 4. UI

- Servers: панель сортировки, пинг-бейджи, кнопки проверки и автоподключения.
- Subscriptions: кнопка обновления, состояние загрузки.
- Home: задержка выбранного сервера в мета-строке.
- Settings: пояснение, что TCP-пинг измеряет адрес, а не RTT туннеля.
- Бейджи этапа обновлены на M3.

### Скриншоты

- [`screenshots/home.png`](screenshots/home.png)
- [`screenshots/servers.png`](screenshots/servers.png)
- [`screenshots/subscriptions.png`](screenshots/subscriptions.png)
- [`screenshots/subscription-dialog.png`](screenshots/subscription-dialog.png)
- [`screenshots/settings.png`](screenshots/settings.png)

## Проверки

| Проверка | Результат |
|---|---|
| `m3vpn-parser` tests | 10/10 |
| Rust tests | 8/8 + 2 ignored |
| `cargo clippy --all-targets -D warnings` | OK |
| `npm run typecheck` | OK |
| `npm run build` | OK |
| `tauri build --debug --no-bundle` | OK |
| CLI probe на реальной подписке | 36/43 responded |
| Runtime smoke | OK |

## Code review (skill `code-review`)

Найдено и исправлено в этом же проходе:

- `theme.ts`: при переключении dark → light старые inline-переопределения
  surface-токенов больше не остаются — иначе светлая тема оставалась тёмной.
- `useAppStore.ts`: ошибка ядра больше не переоткрывает диалог каждые 2 секунды;
  добавлен `reportedStatusError`. Фоновый автопинг стал silent и не спамит snackbar.
  Автовыбор сервера получил гистерезис 15 %, чтобы не дёргать выбор на каждом замере.
- `SettingsPage.tsx`: TUN больше не выглядит рабочим — опция disabled и есть
  пояснение, что трафик идёт через локальный прокси.
- `core/xray.rs`: тег `direct` гарантирован, если в конфиге есть только
  freedom-выход с другим именем.
- `core/supervisor.rs`: удалён неиспользуемый параметр `paths` у `disconnect`.
- `ping.rs`: таймаут замеряется на каждый адрес, добавлен фильтр `enabled`,
  ошибки DNS/connect пробрасываются в результат.
- `sidecars.rs`: проверяются оба geo-файла, а не только `geoip.dat`.
- `links.rs`: reality без `publicKey` отклоняется, HTTPS-proxy получает TLS.
- `parser`: удалён мёртвый `Meta`, обновлён module doc; `@material/web` удалён
  из зависимостей, так как M3-компоненты собственные.
- i18n: удалены 27 неиспользуемых ключей, добавлены подписи для новых действий;
  даты и uptime больше не захардкожены под русский язык.
- Доступность: `aria-pressed` у выбора сервера, `aria-label` у полей,
  `aria-labelledby` у диалогов, Escape не закрывает импорт во время загрузки.

## Ограничения

- **TCP-пинг** — это задержка TCP-хендшейка до адреса сервера, а не RTT
  внутри VLESS/Reality-туннеля. Реальный RTT появится в M4 вместе с
  probe через sing-box/Clash API.
- **Автопереключение на лету** пока не делается: «В самый быстрый»
  перезапускает Xray. Hot-switch без разрыва — M4.
- **AmneziaWG** пока не импортируется: нужен второй core (hiddify-sing-box)
  и отдельный supervisor. Это следующий крупный шаг.
- **TUN** не подключён.

## Следующий этап — M4

1. hiddify-sing-box как второй core: share-ссылки и AWG `.conf`.
2. Hot-switch серверов через Clash API без разрыва соединений.
3. Реальный RTT через локальный прокси/Clash API.
4. TUN + helper + snapshot/restore.
