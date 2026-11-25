# P4 — упаковка и обновления

Дата: 2026-11-25
Статус: первый срез готов; `.rpm`, AppImage и AUR остаются

## Что сделано

### Debian-пакет

`scripts/package-deb.sh` собирает `.deb` из того же stage-дерева, что и
Arch-пакет, без `dpkg-deb`:

- `debian-binary`, `control.tar.gz`, `data.tar.gz`;
- `control`, `md5sums`, `postinst`, `prerm`, `postrm`;
- зависимости `gtk3`, `webkit2gtk-4.1`, `libayatana-appindicator3`;
- `postinst` создаёт группу `m3vpn`, включает helper и добавляет
  `$SUDO_USER` в группу.

Пакет собирается в `dist/m3vpn_<version>-1_amd64.deb`.

### Контрольные суммы

`package-arch.sh` теперь создаёт `dist/SHA256SUMS` для Arch-пакета,
portable-архива и `.deb`. Release workflow загружает `.deb` и `SHA256SUMS`.

### Обновление ядер

**Настройки → Версии ядер**:

- проверяет последние GitHub Releases `XTLS/Xray-core` и
  `hiddify/hiddify-sing-box`;
- выбирает Linux-ассет под текущую архитектуру;
- проверяет SHA-256 из GitHub API;
- распаковывает zip/tar.gz, запускает `version` для проверки;
- атомарно кладёт бинарник в `~/.local/share/m3vpn/cores`;
- `Sidecars` начинает использовать обновлённое ядро.

Проверено вручную:

- Xray: скачивание, распаковка zip, запуск, установка;
- sing-box: скачивание tar.gz, распаковка, запуск, установка.

TUN-ядро helper'а намеренно остаётся встроенным: helper запускает его от
root, поэтому пользовательский бинарник туда не допускается.

### Geo-файлы для России

В профиле маршрутизации добавлен источник geo-данных:

- `Global` — Loyalsoldier;
- `Россия` — `runetfreedom/russia-v2ray-rules-dat`.

Checksum ищется в `.sha256` и `.sha256sum`, поэтому оба источника проверяются
по SHA-256. Кэш sing-box rule-sets очищается после обновления geo.

## Проверки

| Проверка | Результат |
|---|---|
| App tests | 33/33 + 13 ignored |
| Helper tests | 3/3 + 1 ignored |
| Parser tests | 18/18 |
| Xray core update | вручную, OK |
| sing-box core update | вручную, OK |
| `.deb` сборка | OK |
| clippy / typecheck / build | OK |

## Осталось

1. `.rpm` в CI.
2. AppImage.
3. AUR `PKGBUILD` и публикация.
4. minisign/подпись `SHA256SUMS`.
5. Миграция настроек между крупными версиями.
