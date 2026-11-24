# M9 — INGY Ping, палитра и упаковка

Дата: 2026-11-24
Статус: готово; Arch-пакет и portable-архив собраны и проверены

## Пинг как в INCY

Добавлены четыре режима из INCY:

| Режим | Что делает |
|---|---|
| **INGY Ping** | реальный HTTP GET через сам прокси; результат делится на **3.3** |
| **TCP** | TCP-соединение с адресом сервера |
| **HTTP GET** | HTTP GET через прокси, без деления |
| **HTTP HEAD** | HTTP HEAD через прокси, без деления |

Формула INGY взята из документации INCY: «real HTTP GET through the
proxy, but the value is divided by ~3.3 so it reads like a familiar ping».
Для каждого сервера временно поднимается его ядро на отдельном порту,
запрос идёт через SOCKS этого ядра, после замера процесс останавливается.

Дополнительно:

- тестовый URL и пресеты **Google 204 / Cloudflare / Apple**;
- тайм-аут 1–10 секунд со степпером;
- отображение пинга: **Цифры / Шкала / Оба / Точки**;
- INGY Ping включён по умолчанию;
- в списке серверов подпись меняется на выбранный режим.

Проверка на реальной подписке:

```
probed 58 servers, 46 responded
elapsed: 18 s
Hysteria 2: 14 из 15 responded
```

INGY-значения выглядят как обычный пинг: 129–543 мс для дальних серверов,
а не полный HTTP round-trip. TCP-режим остался быстрым и не запускает ядра.

## Палитра

Проблема была не в шрифте, а в цвете чернил:

- `--m3-accent` брался из seed;
- `--m3-accent-ink` был жёстко `#052E16` (тёмно-зелёный);
- при фиолетовом seed получался фиолетовый переключатель с болотно-зелёным
  кругом и тёмно-зелёный текст на фиолетовых кнопках.

Исправлено:

- контрастный цвет текста теперь вычисляется по WCAG для любого seed;
- старые мутные swatch'и заменены на яркие:
  `#22C55E`, `#2563EB`, `#7C3AED`, `#EA580C`, `#0F766E`, `#CA8A04`;
- существующий seed мигрируется один раз (`ui.palette_v2`): например
  `#7A4E8C` → `#7C3AED`;
- фиолетовая тема теперь использует белый текст, зелёная — тёмный.

## Обновления с личного сервера

Добавлен безопасный механизм обновлений без автоустановки:

- приложение читает `latest.json` по HTTPS;
- показывает версию, заметки и ссылки на артефакты;
- требует корректный SHA-256 для каждого артефакта в манифесте;
- открывает пакет в браузере, но **не запускает установщик сам**;
- auto-check раз в 6 часов и при запуске.

Настройка в приложении: **Настройки → Обновления → Сервер обновлений**.
Можно указать URL до `latest.json` или до папки с ним.

Публикация:

```bash
M3VPN_UPDATE_URL="https://updates.example.com/m3vpn" \
M3VPN_UPDATE_HOST="user@updates.example.com" \
M3VPN_UPDATE_PATH="/var/www/updates/m3vpn" \
M3VPN_UPDATE_NOTES="TUN fixes" \
bash scripts/publish-update.sh
```

Скрипт собирает пакеты, создаёт `dist/update/latest.json`, `sha256sums.txt`
и заливает через `rsync`. Пример nginx и схема сервера —
в [`packaging/update/README.md`](../packaging/update/README.md).

## Гео-карточка

Карточка «Гео-данные» переведена с flex на grid: иконка, текст и контролы
больше не разъезжаются. На широком экране всё в одну строку, на узком —
контролы переносятся отдельной строкой.

## Оптимизация по skill

Установлен безопасный skill **blazing-rust** (MIT, только markdown, без
исполняемых скриптов): `~/.config/opencode/skills/blazing-rust`.

Замер INGY Ping на 58 серверах, release-сборка:

| Параллельность | Прогоны | Медиана |
|---|---:|---:|
| 4 | 21.0 / 18.0 / 23.2 с | 21.0 с |
| 8 | 10.6 / 8.8 / 10.2 с | 10.2 с |

Изменение: HTTP-пинг ограничен 8 параллельными проверками вместо 4;
новый default — 8. Работа на сервер не изменилась, число ответивших
серверов осталось в том же диапазоне. Остальной разрыв — запуск временного
ядра на каждый сервер; следующий кандидат — пул прогретых probe-ядер, но
это уже сложнее и не входило в M9.

## Упаковка

Добавлен `scripts/package-arch.sh`. Он собирает release-приложение и helper,
раскладывает sidecars и создаёт:

- `dist/m3vpn-0.1.0-1-x86_64.pkg.tar.zst` — Arch-пакет;
- `dist/m3vpn-0.1.0-linux-x86_64.tar.gz` — portable-архив.

Состав пакета:

```
/usr/bin/m3vpn
/usr/lib/m3vpn/m3vpn-helper
/usr/lib/m3vpn/sidecars/{bin,assets}
/usr/lib/systemd/system/m3vpn-helper.service
/usr/share/applications/cc.m3vpn.desktop
/usr/share/metainfo/cc.m3vpn.desktop.metainfo.xml
/usr/share/icons/hicolor/{32x32,128x128,512x512}/apps/cc.m3vpn.desktop.png
```

Установка:

```bash
sudo pacman -U dist/m3vpn-0.1.0-1-x86_64.pkg.tar.zst
sudo usermod -aG m3vpn "$USER"   # затем перелогиниться
```

`post_install` создаёт группу `m3vpn`, включает `m3vpn-helper.service` и,
если пакет ставили через `sudo`, добавляет `$SUDO_USER` в группу.

Проверки пакета:

- `desktop-file-validate` — OK;
- `appstreamcli validate --no-net` — OK;
- `pacman -Qip` — корректные метаданные и install-скрипт;
- содержимое пакета проверено `bsdtar`;
- release-бинарник из пакета запущен с sidecars из пакета: `--probe` работает.

## Проверки

| Проверка | Результат |
|---|---|
| INGY Ping на реальной подписке | 46/58 responded, 18 с |
| Hysteria 2 через INGY Ping | 14/15 |
| INGY/HTTP GET/HTTP HEAD на локальном прокси | OK |
| App tests | 24/24 + 8 ignored |
| Parser tests | 18/18 |
| Helper tests | 2/2 |
| `cargo clippy -D warnings` | OK |
| `npm run typecheck` / `build` | OK |
| Tauri release build | OK |
| Arch package build | OK |
| Portable tarball | OK |
| desktop / AppStream validation | OK |
| Update manifest + local HTTP test | OK |
| INGY Ping concurrency 4 → 8 | 21.0 → 10.2 с (медиана) |

## Ограничения

- **INGY Ping медленнее TCP**: для каждого сервера поднимается временное
  ядро. На 58 серверах это около 18 секунд при 4 параллельных проверках.
- Некоторые серверы не отвечают на выбранный тестовый URL; это нормально
  и показывается как `—`.
- **`.deb` / `.rpm` / AppImage** в этом проходе не собраны: на машине нет
  `dpkg-deb`/`rpmbuild`/AppImage tooling. Скрипт Arch-пакета готов; для
  Debian/Fedora нужен отдельный CI-шаг.
- **Автоустановка обновлений** не входит в M9: приложение проверяет
  манифест и открывает пакет в браузере, но не запускает `pacman`/установщик.
- **QR и deep links** не входят в M9. Это полировка 1.0.
- TUN по-прежнему требует группу `m3vpn` и повторный вход в сессию.
- 60 FPS — ограничение WebKitGTK/Wayland.

## Что осталось до 1.0

1. `.deb`/`.rpm`/AppImage в CI.
2. QR-коды для share-ссылок.
3. Deep links `m3vpn://` / `incy://`.
4. Автоустановка обновлений (сейчас только проверка и ссылки).
5. Smoke на чистом Arch/Ubuntu.
