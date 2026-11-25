# P1 — TUN и сеть

Дата: 2026-11-25
Статус: живой TUN smoke пройден (auto_redirect, HTTP через VPN и direct после stop)

## Главная находка: петля TUN → front → TUN

TUN-ядро отправляет трафик в front-proxy через SOCKS. Сам front и backend —
обычные процессы, их исходящие соединения тоже попадали под default route
TUN. Получалась петля:

```
front → remote → TUN → front → remote → ...
```

Исправление в несколько шагов:

- front/backend запускаются непривилегированно; helper читает cgroup
  приложения из `/proc/<app_pid>/cgroup` и помечает её nft-правилом
  `socket cgroupv2 ... meta mark 0x2022`;
- `ip rule fwmark 0x2022 lookup main priority 8999` уводит этот трафик из TUN;
- helper отказывается маркировать слишком широкую cgroup (`user@...service`);
- TUN-ядро запускается helper'ом от root и использует **auto_redirect**
  (TPROXY/nftables) — штатный Linux-путь Hiddify/sing-box;
- `auto_redirect_output_mark = 0x2022` совпадает с меткой app-cgroup,
  поэтому auto_redirect не заворачивает front/backend обратно в TUN;
- приватные сети вынесены в `route_exclude_address`, чтобы LAN не шёл в TUN;
- helper добавляет узкое nft-правило `iifname "m3vpn0" accept`, чтобы UFW
  с `INPUT policy drop` не резал TCP-ответы TUN;
- kill switch разрешает метки `0x2022–0x2025`;
- helper удаляет за собой nft-таблицу `inet sing-box` и fallback-правило
  auto_redirect;
- TUN DNS по умолчанию — DoH через TCP;
- `stack: gvisor`, `gso: false`: TCP не зависит от TCP-стека хоста.

Удаление:

- helper удаляет правило 8999 при stop/restore;
- kill switch table `m3vpn` удаляется;
- TUN-интерфейс и правила 9000–9010 чистятся.

## Дополнительно

- watchdog теперь проверяет не только PID приложения/front, но и само
  TUN-ядро: если оно упало, helper восстанавливает сеть;
- смена режима TUN ↔ proxy при активном подключении делает disconnect →
  set mode → reconnect, вместо оставления старого TUN;
- netns-тест проверяет fwmark-правило и kill switch;
- добавлен `scripts/spikes/tun_live_smoke.sh`:
  - отказывается работать без `M3VPN_TUN_SMOKE=1`;
  - проверяет интерфейс, mark, DNS, HTTP и IPv6;
  - останавливает TUN через trap;
  - проверяет удаление правил и kill switch.

## Проверки

| Проверка | Результат |
|---|---|
| App tests | 31/31 + 11 ignored |
| Helper tests | 3/3 + 1 ignored |
| `cargo clippy -D warnings` | OK |
| `npm run typecheck` / build | OK |
| Helper netns + fwmark | `HELPER_NETNS_OK` |
| Helper netns + kill switch | OK |
| TUN-пакет 0.2.0 | собран |
| Живой TUN smoke | **TUN_LIVE_SMOKE_OK** |

## Как проверить на живой системе

```bash
sudo pacman -U /home/shizunaru/Проекты/m3vpn/dist/m3vpn-0.2.0-1-x86_64.pkg.tar.zst
sudo systemctl restart m3vpn-helper

# перезапустить приложение
M3VPN_TUN_SMOKE=1 bash /home/shizunaru/Проекты/m3vpn/scripts/spikes/tun_live_smoke.sh
```

Скрипт меняет маршруты только на время теста и останавливает TUN через trap.
Kill switch отдельно:

```bash
M3VPN_TUN_SMOKE=1 M3VPN_TUN_KILL_SWITCH=1 \
  bash /home/shizunaru/Проекты/m3vpn/scripts/spikes/tun_live_smoke.sh
```

## Результат smoke

```text
TUN interface: 172.19.0.1/30, fdfe:dcba:9876::1/126
HTTP via TUN:   89.23.119.188   (VPN)
Post-TUN HTTP:  85.140.80.25    (direct)
TUN_LIVE_SMOKE_OK
```

## Осталось

- Собрать финальный пакет с `log.level=warn`.
- Поставить тег `v0.2.0` и выпустить GitHub Release.
