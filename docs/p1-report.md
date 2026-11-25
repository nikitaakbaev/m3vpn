# P1 — TUN и сеть

Дата: 2026-11-25
Статус: живой smoke нашёл проблему с SO_MARK; исправлено cgroup-маркировкой,
нужна повторная установка 0.2.0 и smoke

## Главная находка: петля TUN → front → TUN

TUN-ядро отправляет трафик в front-proxy через SOCKS. Сам front и backend —
обычные процессы, их исходящие соединения тоже попадали под default route
TUN. Получалась петля:

```
front → remote → TUN → front → remote → ...
```

Первая попытка исправления ставила `default_mark` / `sockopt.mark` прямо
в конфиги front и backend. Живой smoke показал, что это не работает:
`SO_MARK` требует `CAP_NET_ADMIN`, а sidecar'ы запускаются непривилегированно.
Каждый сокет падал с `operation not permitted`, и TUN-трафик не проходил.

Исправление:

- helper читает cgroup приложения из `/proc/<app_pid>/cgroup`;
- front и backend — дочерние процессы app, поэтому уже находятся в этой
  cgroup, отдельный launcher не нужен;
- helper ставит nft-правило
  `socket cgroupv2 level N "<полный путь app-подгруппы>" meta mark set 0x2022`
  в цепочке `type route hook output priority -150`;
- helper добавляет `ip rule fwmark 0x2022 lookup main priority 8999`;
- трафик с меткой уходит в main table, минуя TUN;
- kill switch разрешает `meta mark 0x2022`, поэтому VPN-транспорт не режется;
- TUN-ядро запускается helper'ом от root, наследует `CAP_NET_ADMIN` и ставит
  `route.default_mark = 0x2022` сам;
- `start_tun` проверяет, что front/backend находятся в cgroup приложения;
- helper отказывается маркировать слишком широкую cgroup (`user@...service`),
  чтобы не завернуть в обход TUN все процессы пользователя;
- TUN DNS по умолчанию — DoH через TCP, поэтому не зависит от UDP в
  прокси-цепочке;
- netns-тест проверяет и cgroup-правило, и fwmark; live smoke дополнительно
  проверяет DNS и HTTP.

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
| Живой TUN smoke | нашёл EPERM на SO_MARK; исправление готово |

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

- Переустановить 0.2.0 с cgroup-маркировкой и перезапустить приложение.
- Прогнать live smoke; проверить DNS и IPv6.
- После успешного smoke поставить тег `v0.2.0` и выпустить GitHub Release.
