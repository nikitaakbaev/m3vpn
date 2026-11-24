# M8 — полные конфиги и два ядра

Дата: 2026-11-24
Статус: готово

## Что закрыто

M8 был «полные Xray-конфиги». В M2/M5 они уже работали, но подписка
провайдера содержала 15 конфигов с **Hysteria 2**, которые Xray не умеет
запускать. Теперь они не пропускаются, а конвертируются в sing-box.

Итог на реальной подписке:

| Метрика | До M8 | После M8 |
|---|---:|---:|
| Импортировано серверов | 43 | **58** |
| Пропущено | 15 | **0** |
| VLESS | 43 | 43 |
| Hysteria 2 | 0 | 15 |

## Архитектура

```
сервер (protocol)
      │
      ├── vless / vmess / trojan / shadowsocks / socks / http
      │        └── Xray
      │
      └── awg / hysteria2 / tuic
               └── hiddify-sing-box
```

Выбор ядра теперь делается по протоколу в одном месте:
`m3vpn_parser::uses_singbox()`. Это же значение показывается в списке
серверов бейджем `Xray` / `sing-box`.

## Hysteria 2

Полный Xray-конфиг Hysteria 2 выглядит так:

```json
{
  "protocol": "hysteria",
  "settings": { "address": "…", "port": 8479, "version": 2 },
  "streamSettings": {
    "network": "hysteria",
    "security": "tls",
    "hysteriaSettings": { "auth": "…", "version": 2 },
    "tlsSettings": { "serverName": "…", "alpn": ["h3"] }
  }
}
```

Парсер превращает его в sing-box outbound:

```json
{
  "type": "hysteria2",
  "server": "…",
  "server_port": 8479,
  "password": "…",
  "tls": { "enabled": true, "server_name": "…", "alpn": ["h3"] }
}
```

Если в конфиге несколько hysteria-outbound'ов (у провайдера есть конфиги
с 2 и 22 серверами), все они сохраняются, а вместо Xray-балансировщика
добавляется sing-box `urltest` с тегом `proxy`. Это сохраняет автовыбор
внутри одного конфига.

Важное ограничение: **uTLS не поддерживается QUIC-протоколами**
(Hysteria 2/TUIC). Поле `fingerprint` из Xray-конфига не переносится —
иначе sing-box падает с `unsupported usage for uTLS`. Это выяснилось на
реальном подключении, а не только по документации.

## Share-ссылки

Добавлены:

- `hysteria2://` и `hy2://`;
- `tuic://`.

Поддерживаются `sni`, `alpn`, `insecure`, `obfs` / `obfs-password`,
`congestion_control`, `udp_relay_mode`, `udp_over_stream`,
`zero_rtt_handshake`. Обе ссылки создают sing-box outbound.

## Импорт из UI

Раньше диалог принимал только URL подписки и AmneziaWG `.conf`. Теперь в
одно поле можно вставить:

- URL подписки;
- полный Xray JSON (один объект или массив);
- base64-подписку;
- share-ссылку (`vless`, `vmess`, `trojan`, `ss`, `hysteria2`, `tuic`);
- AmneziaWG `.conf` или `amneziawg://` / `awg://`.

URL уходит в обычный импорт подписки, всё остальное — в новый
`import_config`. Профиль получает название из поля «Название» или из
первого сервера.

## Переключение Xray ↔ sing-box

Добавлен интеграционный тест:

1. подключить Xray backend;
2. проверить трафик через front-proxy;
3. hot-switch на sing-box backend;
4. снова проверить трафик;
5. вернуться на Xray.

Тест проходит: каждый шаг отдаёт `curl` через один и тот же SOCKS-порт.

## Проверки

| Проверка | Результат |
|---|---|
| Парсер полной подписки | 58 серверов, 0 пропущено |
| Импорт реальной подписки в temp DB | 58 серверов, 0 пропущено |
| `xray run -test` для 43 VLESS-конфигов | 43/43 OK |
| `hiddify-sing-box check` для 15 Hysteria 2 | 15/15 OK |
| Реальное подключение Hysteria 2 через sing-box + curl | OK |
| Переключение Xray → sing-box → Xray под трафиком | OK |
| Parser tests | 18/18 |
| App tests | 19/19 + 7 ignored |
| Сетевые ignored-тесты (Xray/sing-box switch, fail-open, hot-switch, keeper) | 5/5 |
| `cargo clippy -D warnings` | OK |
| `npm run typecheck` / `build` | OK |
| Tauri debug build | OK |
| Helper netns test | `HELPER_NETNS_OK` |

## Ограничения

- **Hysteria 1** не поддерживается: sing-box умеет только Hysteria 2.
  Такие конфиги пропускаются с пояснением.
- **TCP-пинг** для Hysteria 2/TUIC показывает `—`: это QUIC/UDP-серверы,
  TCP-порт у них не отвечает. Поэтому автовыбор по TCP не сможет выбрать
  их без отдельного QUIC-замера.
- **TUIC** поддержан в share-ссылках. Полные Xray-конфиги TUIC не
  конвертируются: в подписке провайдера их нет, а формат Xray-обёртки
  для TUIC не стандартизирован.
- **uTLS fingerprint** для QUIC игнорируется (см. выше).
- **TUN** по-прежнему требует установленного `m3vpn-helper`.
- 60 FPS — ограничение WebKitGTK/Wayland.

## Следующий этап — M9

1. `.deb` / `.rpm` / AppImage / AUR.
2. `.desktop`, иконки, deep links.
3. QR-коды для share-ссылок.
4. Автообновление приложения.
5. Финальная документация и проверка на чистом Arch/Ubuntu.
