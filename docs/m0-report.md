# M0 — отчёт по spike-этапу

Дата: 2026-11-24
Хост: CachyOS Linux, x86_64, Hyprland, Go 1.27.4, Rust 1.99.1

## Итог

| Проверка | Результат |
|---|---|
| Сборка `hiddify-sing-box` с `with_awg` | OK |
| Полные Xray-конфиги из подписки | OK |
| VLESS Reality через sing-box | OK |
| AmneziaWG handshake + трафик | OK |
| Переключение selector без разрыва загрузки | OK |
| Восстановление TUN после `SIGKILL` | OK |

Все артефакты зафиксированы в `sidecars/versions.json`.

## 1. Подписка

URL и токен хранятся только в `.secrets/subscription.url` (0600), в git не попадают.

Ответ сервера:

- `content-type: application/json`
- 58 полных Xray-конфигов (массив), каждый с `inbounds` + `outbounds`;
- протоколы: `vless` (Reality/TCP и XHTTP/TLS), `hysteria`;
- `burstObservatory` + balancer `leastLoad` с `tolerance: 0.4` и `costMultiplier`;
- routing: `geoip:private`, `geosite:category-ru` в direct, остальное через прокси;
- inbounds: SOCKS `127.0.0.1:10808`, HTTP `127.0.0.1:10809`.

Вывод: наш сценарий «полные Xray-конфиги через Xray-core» — не гипотетический,
это основной формат этого провайдера. Парсер должен сохранять `fullConfigJson`
и патчить inbounds/логи/DNS так, как описано в M8.

Проверка:

```bash
python3 scripts/spikes/inspect_subscription.py
python3 scripts/spikes/inspect_config.py 4
```

## 2. Сборка ядра

Собрано из `hiddify/hiddify-sing-box` v5 (`1267fe6`), Go 1.27.4,
Makefile-теги (включая `with_awg`).

```bash
bash scripts/build-sidecars.sh
sidecars/bin/hiddify-sing-box version
```

Версия сообщает:

```
Tags: ...,with_awg,...
Revision: 1267fe6aac8d11f3a562e8feecad11d11f48f965
```

### Проблемы сборки и решения

1. Shallow clone без submodules → `replace/psiphon-tls/go.mod` отсутствует.
   Решение: `git submodule update --init --recursive --depth 1`.
2. Записанный в superproject коммит `replace/sing` недоступен (force-push).
   Решение: checkout ветки `v5` (`b40ea07`).
3. `replace/sing-mux` был на `main`, а нужен `v5`.
   Решение: checkout `v5` (`ea35b38`).
4. `replace/soar`: коммит `d7c81ce` недоступен. Ветка `hiddify` содержит
   `Discovery.List()`, но не `Conn.Abort()`, который ожидает superproject.
   Временный патч: `c.Conn.Abort()` → `c.Conn.Close()` в
   `protocol/hiddify/soar/outbound.go`.
   **TODO до релиза:** либо найти совместимый коммит, либо отключить `soar`,
   либо отправить патч в upstream. Патч применяется автоматически в
   `scripts/build-sidecars.sh` и зафиксирован в `versions.json`.

## 3. Полные Xray-конфиги

```bash
bash scripts/spikes/run_xray_test.sh 4   # Germany, whitelist, XHTTP/TLS
bash scripts/spikes/run_xray_test.sh 0   # auto, leastLoad, Reality
```

Результат:

- direct IP: `85.140.80.25`
- proxied IP: `185.192.23.120` (`loc=DE`)
- `curl` через SOCKS5 `127.0.0.1:20808` работает.

Замечание: изначально тест шёл на порту 10808, но на хосте уже запущен
**INCY** (`/opt/incy/bin/incy`, pid 1903), который держит 10808/10809.
Поэтому все spike-скрипты используют 20808/20809, а приложение должно уметь
выбирать свободный порт.

Для Xray нужны `geoip.dat`/`geosite.dat`; они скачаны в `sidecars/assets`
и подключаются через `XRAY_LOCATION_ASSET`.

## 4. VLESS Reality через sing-box

Конвертер одного outbound из полного конфига:

```bash
python3 scripts/spikes/xray_vless_to_singbox.py \
  --config-index 0 --outbound-index 2 -o .secrets/vless-singbox.json
sidecars/bin/hiddify-sing-box run -c .secrets/vless-singbox.json
```

Проверка outbound `proxy-paid-3` (Reality, TCP):

- `curl --socks5-hostname 127.0.0.1:20808 https://api.ipify.org` → `185.192.23.120`.

Серверы 0 и 1 не ответили (i/o timeout), 2 и 4–7 работают — провайдерский
список содержит как живые, так и мёртвые узлы. Это подтверждает необходимость
ping/failover.

## 5. AmneziaWG

Поднят локальный AmneziaWG-сервер в Docker (`metaligh/amneziawg`), порт
`51830/udp`, obfuscation `Jc=7, Jmin=50, Jmax=1000, S1=68, S2=149, H1..H4`
разные.

Клиентский `.conf` сгенерирован внутри контейнера, сконвертирован в
sing-box endpoint:

```bash
python3 scripts/spikes/awg_conf_to_singbox.py \
  .secrets/awg-client.conf -o .secrets/awg-singbox.json
sidecars/bin/hiddify-sing-box check -c .secrets/awg-singbox.json
sidecars/bin/hiddify-sing-box run -c .secrets/awg-singbox.json
curl --socks5-hostname 127.0.0.1:20808 https://www.cloudflare.com/cdn-cgi/trace
```

Результат:

- handshake: `latest handshake: 1 second ago`
- transfer: `9.49 KiB received, 13.20 KiB sent`
- `cf trace`: `ip=85.140.80.25 loc=RU` (сервер NAT-ит на тот же хост — это
  ожидаемо для локального теста; важен сам факт прохождения трафика через
  `endpoint/awg`, видно в логе sing-box).

Конвертер поддерживает `[Interface]`, `[Peer]`, `[Device]`, диапазоны,
булевы значения и поля AmneziaWG 3.x. Это прототип будущего Rust-парсера.

## 6. Переключение без разрыва соединения

```bash
bash scripts/spikes/selector_switch_test.sh
```

Сценарий:

1. sing-box с inbound `127.0.0.1:20808`, selector `proxy` (`srv-a` / `srv-b`),
   Clash API на 29090.
2. Скачивается 8 МБ через SOCKS с ограничением 400 КБ/с.
3. На середине — `PUT /proxies/proxy {"name":"srv-b"}`.
4. Загрузка завершается, `curl exit 0`, размер 8 388 608, `cmp` совпадает.

Вывод: `interrupt_exist_connections: false` + смена selector через API —
рабочая схема «не рвать соединения при переключении».

## 7. Восстановление TUN после краша

```bash
bash scripts/spikes/tun_restore_test.sh
```

Тест изолирован в `unshare -Urn` (rootless user+network namespace), хост не
затрагивается. Внутри:

1. Базовая конфигурация: dummy-интерфейс, маршрут, nft-таблица.
2. Дочерний процесс создаёт `tun9`, default route, nft-таблицу `m3vpn`
   и убивает себя `SIGKILL`.
3. Восстановление: удалить `tun9`, удалить `m3vpn`, восстановить routes/nft
   из snapshot.
4. Сравнение JSON маршрутов и вывода nft — совпадает.

Результат: `TUN_RESTORE_OK`, 11 маршрутов до и после.

Это подтверждает подход: snapshot + идемпотентный restore + watchdog.
Реализацию переносим в `crates/helper` (Rust).

## 8. Что это значит для плана

1. M8 (полные Xray-конфиги) — не «потом», а **основной путь** для этой
   подписки. Его надо поднять раньше, чем в плане: сразу после M2/M3.
2. Автопинг: у провайдера уже есть `burstObservatory` + `leastLoad`, но для UI
   нужен свой планировщик через Clash API; для Xray-конфигов — отдельный путь
   (Xray Stats/observatory). На первом этапе Xray-серверы можно исключить из
   авто-выбора и показывать «—».
3. Порты: 10808/10809 заняты INCY. Нужен автоподбор свободного порта и
   настройка `proxy.port`.
4. AmneziaWG подтверждён; можно смело включать в MVP.
5. Сборка ядра требует фиксации submodule-коммитов и патча `soar`; это должно
   быть в CI, иначе сборка не воспроизводится.

## 9. Артефакты

| Файл | Назначение |
|---|---|
| `scripts/build-sidecars.sh` | воспроизводимая сборка ядер и geo |
| `scripts/spikes/inspect_subscription.py` | разбор тела подписки |
| `scripts/spikes/inspect_config.py` | санитизация одного Xray-конфига |
| `scripts/spikes/run_xray_test.sh` | запуск Xray + curl через SOCKS |
| `scripts/spikes/xray_vless_to_singbox.py` | VLESS/Reality → sing-box |
| `scripts/spikes/awg_conf_to_singbox.py` | AWG `.conf` → `endpoint.awg` |
| `scripts/spikes/selector_switch_test.sh` | проверка hot-switch |
| `scripts/spikes/tun_restore_test.sh` | проверка restore в netns |
| `sidecars/versions.json` | версии, коммиты, sha256, результаты |

## 10. Следующий шаг

M1: каркас Tauri + Material 3, SQLite, настройки, трей, логи.
Параллельно — перенос `awg_conf_to_singbox.py` и парсера подписок в
`crates/parser` с unit-тестами на фикстурах из этой подписки.
