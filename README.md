# m3vpn

Клон функциональности INCY для Linux: Material 3 UI, автоматический пинг серверов,
выбор быстрейшего и безопасное отключение без потери интернета.

## Статус

- **M0 — готово.** Ядра собраны, AWG/selector/TUN проверены. См. [`docs/m0-report.md`](docs/m0-report.md).
- **M1 — готово.** Каркас Tauri 2 + React + Material 3, SQLite, настройки, трей.
  См. [`docs/m1-report.md`](docs/m1-report.md).

## Структура

```
apps/desktop/           React + Material 3 + Tauri 2
apps/desktop/src-tauri/ Rust: настройки, SQLite, трей, порты
crates/                 будущие parser/model/helper
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

Подписка и токены не попадают в git: `.secrets/` исключён и доступен только владельцу.

## Что уже решено

- Динамический выбор свободного прокси-порта (INCY занимает 10808/10809).
- Версии ядер и sha256 зафиксированы в `sidecars/versions.json`.
- Сборка `hiddify-sing-box` с `with_awg` и патч `soar` воспроизводятся скриптом.
- TUN restore после `SIGKILL` проверен в rootless netns.
