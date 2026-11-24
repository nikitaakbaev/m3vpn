# M6 — TUN, привилегированный helper и безопасный restore

Дата: 2026-11-24
Статус: код и тесты готовы; установка helper требует root

## Архитектура TUN

```
системный трафик
      │
      ▼
m3vpn0 (TUN, root, helper)
      │  sing-box TUN inbound
      ▼
127.0.0.1:<front socks>
      │
      ▼
front-proxy → backend (Xray/AWG) или direct
```

- TUN поднимает отдельный **привилегированный sing-box**, который запускает
  helper от root.
- Приложение остаётся непривилегированным: оно только просит helper
  включить/выключить TUN через Unix socket.
- TUN-ядро ходит в уже существующий front-proxy через SOCKS. Поэтому
  hot-switch, fail-open и direct-режим работают и в TUN.
- Helper следит за PID приложения и front-proxy. Если любой из них исчез,
  TUN останавливается, маршруты и правила восстанавливаются.

## Helper

`crates/helper`:

- Unix socket `/run/m3vpn/helper.sock` (`root:m3vpn`, `0660`);
- команды `start_tun`, `stop_tun`, `status`, `panic`, `restore`;
- snapshot перед стартом: маршруты, IPv6-маршруты, default routes,
  nftables, iptables, `/etc/resolv.conf`, `resolvectl`;
- запуск TUN-ядра `hiddify-sing-box` с `interface_name=m3vpn0`,
  `auto_route=true`, `strict_route=false`;
- watchdog по `/proc/<app_pid>` и `/proc/<front_pid>`;
- при остановке: kill TUN-ядра, удаление `m3vpn0`, удаление policy rules
  9000–9010, flush table 2022, удаление nft-таблицы `m3vpn`, восстановление
  default-маршрутов;
- idempotent `restore` для `ExecStopPost` и восстановления после краша;
- kill switch — опциональная nftables-таблица, по умолчанию выключена.

## Установка

```bash
sudo ./scripts/install-helper.sh
# затем выйти из сессии и войти снова (группа m3vpn)
```

Скрипт:

- собирает `m3vpn-helper` release;
- копирует helper и sidecars в `/usr/lib/m3vpn`;
- создаёт группу `m3vpn`, добавляет пользователя;
- устанавливает и запускает `m3vpn-helper.service`.

После установки в приложении станет доступен режим **TUN**. Пока helper
не установлен, TUN отключён, а в настройках написано, что нужно сделать.

## Проверки

| Проверка | Результат |
|---|---|
| Helper unit tests | 2/2 |
| Helper client round-trip | OK |
| Netns-тест helper | TUN `m3vpn0`, policy rules, stop, restore routes/rules — OK |
| `cargo clippy -D warnings` (helper) | OK |
| `cargo clippy -D warnings` (app) | OK |
| Rust app tests | 11/11 + 6 ignored |
| `npm run typecheck` / `build` | OK |
| Tauri build | OK |

Netns-тест не меняет сеть хоста:

```bash
bash scripts/spikes/helper_netns_test.sh
```

## Ограничения

- **Helper не установлен автоматически.** Для него нужен root; в текущем
  окружении нет passwordless sudo, поэтому установка остаётся за тобой.
- После `usermod -aG m3vpn` нужен повторный вход в сессию, иначе socket
  будет недоступен приложению.
- Kill switch при аварии helper'а снимается вместе с TUN (fail-open) —
  это сделано намеренно, чтобы не оставить систему без интернета.
- Восстановление DNS ограничено: helper возвращает default-маршруты и
  делает `resolvectl revert m3vpn0`; полный snapshot `/etc/resolv.conf`
  хранится для ручного restore.
- TUN-тест проверяет интерфейс/маршруты/правила, но не гоняет реальный
  трафик через внешний сервер в netns.
- 60 FPS остаётся ограничением WebKitGTK/Wayland.

## Следующий этап — M7

1. Профили маршрутизации (geoip/geosite, DoH/DoU, FakeDNS).
2. Автообновление geo-файлов.
3. Упаковка helper и приложения в deb/rpm/AUR.
