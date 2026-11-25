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
- helper всегда добавляет `dns-bootstrap` — UDP `1.1.1.1` через SOCKS;
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

## Осталось в P3

1. **Split tunneling** по приложениям (cgroup/UID).
2. **IPv6 policy** для TUN и kill switch.
3. **FakeDNS для Xray**, если формат будет стабильным.
4. **Traffic-aware auto-switch**: не менять сервер во время активной
   загрузки/стрима (нужны счётчики трафика).
5. **Автоподбор geo-файлов** под провайдера.

## Проверки

| Проверка | Результат |
|---|---|
| App tests | 30/30 + 11 ignored |
| Helper tests | 3/3 + 1 ignored |
| Rule-set download | 3/3 |
| sing-box check с rule-set | OK |
| sing-box check TUN DNS | OK |
| Rule-set list | OK |
| Helper netns | `HELPER_NETNS_OK` |
| clippy / typecheck / build | OK |
