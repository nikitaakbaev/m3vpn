# M7 — маршрутизация, DNS и geo-файлы

Дата: 2026-11-24
Статус: готово; TUN по-прежнему требует установленного helper

## Что добавлено

- Экран **Маршруты** с импортом профилей INCY.
- Поддержка форматов:
  - JSON профиля;
  - URL на JSON;
  - base64 и URL-safe base64;
  - deep link `incy://routing/onadd/<payload>`.
- Применение профиля к **Xray-конфигам** и к **AmneziaWG (sing-box)**.
- Правила `direct`, `proxy`, `block` по доменам и IP.
- `GlobalProxy=false` переводит необработанный трафик в `direct`.
- DNS:
  - DoH по URL/домену/IP;
  - обычный DNS (DoU) для domestic-сервера;
  - `DnsHosts`;
  - DNS-серверы исключаются из проксирования, чтобы не было петли.
- `DomainStrategy`: `AsIs`, `IPIfNonMatch`, `IPOnDemand`.
- **FakeDNS** для sing-box/AWG: `fakeip`-сервер и правило для доменов из `ProxySites`.
- Geo-файлы:
  - `~/.local/share/m3vpn/geo/{geoip.dat,geosite.dat}`;
  - начальное заполнение из `sidecars/assets`;
  - скачивание с проверкой `.sha256`, если сервер её отдаёт;
  - автообновление раз в 6 часов при запущенном приложении, если срок истёк;
  - интервал настраивается на экране «Маршруты»: выкл / 12 ч / 24 ч / 48 ч / 7 дней;
  - Xray запускается с `XRAY_LOCATION_ASSET` на эту папку.
- Активный профиль применяется сразу: если VPN подключён, приложение
  переподключает текущий сервер через hot-switch.

## Как это работает

```
профиль (JSON/URL/deep link)
        │
        ▼
routing_profiles (SQLite)
        │  активный профиль
        ▼
connect_server
   ├── Xray:  routing.rules + dns.servers + dns.hosts
   └── sing-box: route.rules + route.final + dns (FakeDNS)
        │
        ▼
front-proxy → backend → интернет
```

Front-proxy не знает о правилах: маршрутизацию решает backend-ядро. Поэтому
hot-switch, fail-open и direct-режим продолжают работать без изменений.

## Совместимость geo-тегов

В bundled `geosite.dat` (Loyalsoldier) нет записи `RU`, есть `CATEGORY-RU`.
INCY-профили часто используют `geosite:ru`, и Xray отказывался стартовать с
`illegal domain rule: geosite:ru`. Профиль нормализует `geosite:ru` →
`geosite:category-ru`.

Проверено на Xray 26.11.9: сгенерированный конфиг с `geoip:ru`,
`geosite:category-ru`, `geosite:google`, `geosite:category-ads-all` проходит
`xray run -test`.

## Проверки

| Проверка | Результат |
|---|---|
| `cargo clippy -D warnings` (app) | OK |
| Rust app tests | 17/17 + 6 ignored |
| Parser tests | 14/14 |
| Helper tests | 2/2 |
| `npm run typecheck` | OK |
| `npm run build` | OK |
| Tauri debug build | OK |
| Helper netns test | `HELPER_NETNS_OK` |
| Xray config validation | OK |
| Screenshots | home, servers, subscriptions, routing, settings, dialog |

## Ограничения

- **FakeDNS** реализован для sing-box/AWG. Xray-конфигам FakeDNS не
  добавляется: формат Xray зависит от версии ядра, а неправильная настройка
  ломает DNS. Для Xray работают обычные DoH/DoU.
- **Geo-теги** (`geoip:`, `geosite:`) применяются только в Xray. Для sing-box
  они пропускаются, остаются домены и CIDR. Это ограничение самого sing-box:
  его geo-формат отличается и требует отдельных rule-set файлов.
- **Неизвестный geo-код** (кроме `ru`) по-прежнему вызовет ошибку Xray. Это
  ожидаемо: приложение не подменяет данные, а показывает ошибку подключения.
- **Автообновление geo** проверяется при запуске и раз в 6 часов, а не по
  точному расписанию. Если приложение закрыто, обновление произойдёт при
  следующем запуске.
- **TUN** остаётся недоступен, пока не установлен `m3vpn-helper`. M7 не меняет
  установку helper и не требует root.
- 60 FPS — ограничение WebKitGTK/Wayland, не связано с M7.

## Следующий этап — M8

M8 фактически закрыт в M2/M5: полные Xray-конфиги, share-ссылки и
переключение Xray ↔ sing-box работают. Осталось формально проверить
edge-cases и обновить статус. Дальше — M9: упаковка, `.desktop`, иконки,
deep links, QR и автообновление приложения.
