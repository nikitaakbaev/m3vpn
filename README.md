# m3vpn

Клон функциональности INCY для Linux: Material 3 UI, автоматический пинг серверов,
выбор быстрейшего и безопасное отключение без потери интернета.

## Статус

- **M0 — готово.** Ядра собраны, AWG/selector/TUN проверены. См. [`docs/m0-report.md`](docs/m0-report.md).
- **M1 — готово.** Каркас Tauri 2 + React + Material 3, SQLite, настройки, трей.
  См. [`docs/m1-report.md`](docs/m1-report.md).
- **M2 — готово.** Импорт полных Xray-конфигов, реальный запуск Xray, подключение,
  новый dark Material 3 дизайн. См. [`docs/m2-report.md`](docs/m2-report.md).
- **M3 — готово.** Share-ссылки, обновление подписок, TCP-пинг, сортировка и
  подключение к самому быстрому. См. [`docs/m3-report.md`](docs/m3-report.md).
- **M4 — готово.** Исправлены зависания, добавлены AmneziaWG через sing-box,
  реальный RTT и второй core. См. [`docs/m4-report.md`](docs/m4-report.md).
- **M5 — готово.** Front-proxy с hot-switch и авто-переключением на быстрейший:
  смена сервера не рвёт активные соединения, при падении бэкенда трафик уходит
  в direct. См. [`docs/m5-report.md`](docs/m5-report.md).
- **M6 — готово.** TUN через привилегированный helper, snapshot/restore,
  watchdog и опциональный kill switch. См. [`docs/m6-report.md`](docs/m6-report.md).
- **M7 — готово.** Профили маршрутизации INCY, direct/proxy/block, DoH/DoU,
  geoip/geosite, FakeDNS для AWG и автообновление geo. См.
  [`docs/m7-report.md`](docs/m7-report.md).
- **M8 — готово.** Полные конфиги без пропусков: Hysteria 2 через sing-box,
  импорт JSON/share-ссылок из UI, автоматический выбор Xray или sing-box,
  переключение ядер. См. [`docs/m8-report.md`](docs/m8-report.md).
- **M9 — готово.** INGY Ping (реальный HTTP GET через прокси), настройки
  отображения пинга, новая палитра, Arch-пакет, portable tar.gz, `.desktop`,
  иконки, AppStream и проверка обновлений с личного сервера. См.
  [`docs/m9-report.md`](docs/m9-report.md).

## Обновления

В **Настройки → Обновления** укажи URL личного сервера:

```text
https://updates.example.com/m3vpn/latest.json
```

Приложение проверяет манифест по HTTPS, показывает новые версии и ссылки на
Arch-пакет/portable-архив, но не устанавливает их автоматически.

Публикация на сервер:

```bash
M3VPN_UPDATE_URL="https://updates.example.com/m3vpn" \
M3VPN_UPDATE_HOST="user@updates.example.com" \
M3VPN_UPDATE_PATH="/var/www/updates/m3vpn" \
bash scripts/publish-update.sh
```

Схема сервера и пример nginx: [`packaging/update/README.md`](packaging/update/README.md).

## Структура

```
apps/desktop/           React + Material 3 + Tauri 2
apps/desktop/src-tauri/ Rust: настройки, SQLite, трей, порты
crates/parser/          парсер полных Xray-конфигов
sidecars/               версии и хеши ядер (бинарники не в git)
scripts/                сборка ядер и spike-проверки
docs/                   отчёты по этапам
```

## Быстрый старт

```bash
# ядра и geo-ассеты
bash scripts/build-sidecars.sh
bash scripts/check-sidecars.sh

# приложение
cd apps/desktop
npm install
npm run tauri dev
```

Сборка Arch-пакета и portable-архива:

```bash
bash scripts/package-arch.sh
sudo pacman -U dist/m3vpn-0.1.0-1-x86_64.pkg.tar.zst
```

Пакет ставит приложение, sidecars, helper, `.desktop`, иконки и AppStream.
После установки для TUN нужно войти в группу `m3vpn` и перелогиниться:

```bash
sudo usermod -aG m3vpn "$USER"
```

Импорт подписки без UI (в ту же SQLite, что использует приложение):

```bash
apps/desktop/src-tauri/target/debug/m3vpn --import "<url>" "PingBooster"
```

Подписка и токены не попадают в git: `.secrets/` исключён и доступен только владельцу.

## Что уже решено

- Динамический выбор свободного прокси-порта (INCY занимает 10808/10809).
- Версии ядер и sha256 зафиксированы в `sidecars/versions.json`.
- Сборка `hiddify-sing-box` с `with_awg` и патч `soar` воспроизводятся скриптом.
- TUN restore после `SIGKILL` проверен в rootless netns.
