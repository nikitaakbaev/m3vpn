# P3 — routing, DNS и geo rule-sets

Дата: 2026-11-25
Статус: sing-box rule-sets и TUN DNS готовы; split tunneling — следующий

## sing-box rule-sets

Раньше `geoip:` и `geosite:` теги работали только в Xray, а sing-box
получал лишь обычные домены и CIDR. Теперь:

- профиль собирает уникальные `geosite:*` и `geoip:*` теги;
- для каждого тега скачивается локальный `.srs` из SagerNet:
  - `sing-geosite/rule-set/geosite-<tag>.srs`;
  - `sing-geoip/rule-set/geoip-<tag>.srs`;
- `geosite:ru` нормализуется в `category-ru` — этого тега нет в обычном
  sing-geosite;
- файлы кэшируются в `~/.local/share/m3vpn/geo/rulesets`;
- скачивание ограничено 6 секундами на файл и 12 секундами суммарно;
- если тег не скачался, профиль всё равно запускается без него;
- при обновлении geo кэш rule-set удаляется, следующий connect скачает
  свежие категории;
- `route.rule_set` добавляется в конфиг sing-box, правила получают
  `rule_set` вместо пропущенных тегов.

Проверено:

- скачивание `geosite-category-ru.srs`, `geosite-google.srs`,
  `geoip-ru.srs`;
- `sing-box check` на конфиге с этими rule-set — OK;
- unit-тест правил `rule_set` для direct/proxy/reject.

## TUN DNS

Раньше TUN-ядро всегда использовало `1.1.1.1` через SOCKS. Теперь приложение
передаёт helper'у remote DNS из активного routing-профиля:

- `DoH` → `{ "type": "https", "server": "...", "path": "/dns-query" }`;
- `DoU` / IP → `{ "type": "udp", "server": "..." }`;
- `tls://` и `quic://` поддерживаются как `tls` / `quic`;
- helper всегда добавляет `dns-bootstrap` — DoH `1.1.1.1` с SNI
  `cloudflare-dns.com` через SOCKS; DNS идёт по TCP, поэтому работает даже
  если прокси-цепочка не поддерживает UDP;
- если DNS-сервер задан доменом, helper ставит ему
  `domain_resolver: dns-bootstrap`, поэтому bootstrap-запрос идёт через
  прокси и не утекает;
- `detour` принудительно `proxy`, даже если профиль попытается его
  переопределить;
- неизвестные типы игнорируются, TUN всё равно стартует на bootstrap DNS.

Конфиг проверен `sing-box check`.

## UI rule-sets

В карточке geo-данных показываются закэшированные категории
(`geosite:category-ru`, `geoip:ru`, …) с размером и датой в tooltip.
После обновления geo кэш очищается, и список исчезает до следующего
подключения.

## Макет настроек

По замечанию к полному окну настройки больше не раскладываются на две
независимые плитки. `settings-grid` теперь одна колонка: карточки идут
друг за другом и растягиваются на всю доступную ширину.

## Geo-источники

В карточке geo-данных появился выбор источника:

- `Global` — Loyalsoldier, текущее поведение;
- `Россия` — `runetfreedom/russia-v2ray-rules-dat`;
- профиль всё ещё может переопределить URL через `Geoipurl` / `Geositeurl`.

Checksum ищется в `.sha256` и `.sha256sum`, поэтому файлы runetfreedom тоже
проверяются по SHA-256.

## IPv6 policy

В настройках соединения появился переключатель **IPv6 в TUN**:

- включён (по умолчанию) — TUN получает IPv4 и IPv6 адреса;
- выключен — TUN создаётся только с IPv4, а helper поднимает nft-таблицу
  `m3vpn_ipv6` с приоритетом `-200`, которая блокирует весь IPv6-output,
  кроме loopback;
- это защищает от IPv6-утечек, когда провайдер или профиль не поддерживает
  IPv6;
- настройка применяется после переподключения.

## Geo auto

Источник geo теперь может быть `auto`:

- `ru`, если `LC_ALL` / `LANG` начинается с `ru` или
  `/etc/timezone` содержит `Moscow`;
- иначе `global`;
- явные `ru` / `global` по-прежнему переопределяют авто-выбор.

`auto` — значение по умолчанию для новых установок; существующие настройки
не перезаписываются.

## FakeDNS для Xray

Если в routing-профиле включён `FakeDNS`:

- в Xray-конфиг добавляется `fakedns` с пулом `198.18.0.0/15`;
- `fakedns` становится первым DNS-сервером;
- sniffing на SOCKS/mixed inbound получает `destOverride: ["fakedns"]`
  в дополнение к `http` и `tls`;
- это позволяет Xray восстанавливать домен по FakeIP.

## Split tunneling

В настройках соединения появился список запущенных приложений с отдельной
systemd-cgroup:

- выбранные приложения помечаются nft-правилом
  `socket cgroupv2 ... meta mark set 0x2022`;
- auto_redirect видит output mark и оставляет их трафик в main table;
- приложение работает напрямую, вне TUN;
- список строится из текущего `app.slice` пользователя;
- старые/закрытые cgroup пропускаются;
- настройка применяется после переподключения.

## DNS-политика

В карточке geo-данных показывается:

- тип remote DNS (DoH/DoU);
- адрес/домен резолвера;
- статус `TUN DNS hijack: on`.

DNS-запросы в TUN перехватываются и идут через DoH/прокси; при отключённом
IPv6 helper блокирует IPv6-выход.

## Осталось в P3

1. **FakeDNS для Xray** — сделано.
2. **Проверка DNS-утечек** — базовая политика в UI; внешний leak-test
   сервис не добавлялся.
3. **Split tunneling** — сделано для systemd-cgroup приложений.

## Проверки

| Проверка | Результат |
|---|---|
| App tests | 37/37 + 13 ignored |
| Helper tests | 4/4 + 1 ignored |
| Rule-set download | 3/3 |
| sing-box check с rule-set | OK |
| sing-box check TUN DNS | OK |
| Rule-set list | OK |
| Helper netns | `HELPER_NETNS_OK` |
| clippy / typecheck / build | OK |
