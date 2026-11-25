# P1 — TUN и сеть

Дата: 2026-11-25
Статус: код готов, пакет 0.2.0 собран; живой smoke ожидает установки

## Главная находка: петля TUN → front → TUN

TUN-ядро отправляет трафик в front-proxy через SOCKS. Сам front и backend —
обычные процессы, их исходящие соединения тоже попадали под default route
TUN. Получалась петля:

```
front → remote → TUN → front → remote → ...
```

Исправление:

- front sing-box получает `route.default_mark = 0x2022`;
- Xray-бэкенды получают `streamSettings.sockopt.mark = 0x2022`;
- sing-box-бэкенды получают `route.default_mark = 0x2022`;
- helper добавляет `ip rule fwmark 0x2022 lookup main priority 8999`;
- трафик с меткой уходит в main table, минуя TUN;
- kill switch разрешает `meta mark 0x2022`, поэтому VPN-транспорт не режется.

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
| App tests | 28/28 + 9 ignored |
| Helper tests | 2/2 |
| `cargo clippy -D warnings` | OK |
| `npm run typecheck` / build | OK |
| Helper netns + fwmark | `HELPER_NETNS_OK` |
| Helper netns + kill switch | OK |
| TUN-пакет 0.2.0 | собран |
| Живой TUN smoke | ожидает установки 0.2.0 |

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

## Осталось

- Установить 0.2.0 и прогнать live smoke.
- Проверить IPv6/DNS на реальном провайдере.
- После успешного smoke поставить тег `v0.2.0` и выпустить GitHub Release.
