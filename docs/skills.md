# Agent skills used by m3vpn

Все установленные skills — markdown-инструкции без исполняемых скриптов.
Перед установкой проверены лицензия, содержимое каталога и отсутствие hook'ов.

| Skill | Источник | Лицензия | Применение |
|---|---|---|---|
| `blazing-rust` | `painhardcore/blazing-rust` | MIT | профилирование и оптимизация Rust |

Проверено: только `SKILL.md`, `references/techniques.md`, `LICENSE`.

| Skill | Источник | Лицензия | Применение |
|---|---|---|---|
| `accessibility-auditor` | `patricio0312rev/skills` | MIT | WCAG-аудит, keyboard navigation, ARIA, progressbar, skip link |
| `frontend-refactor-planner` | `patricio0312rev/skills` | MIT | безопасный план рефакторинга, выделение `ServerLatency` |
| `secrets-scanner` | `patricio0312rev/skills` | MIT | gitleaks-аудит и CI secret scan |

Проверено: только `SKILL.md`, без install/pre-commit/CI-скриптов.

## Что изменено после аудита

- `Segmented` переведён на `radiogroup` + `radio`, добавлена навигация
  стрелками, Home/End и roving tabindex.
- Добавлен skip-link к основному содержимому.
- Заголовки карточек переведены с `h3` на `h2` после `h1`.
- Прогресс трафика получил `role="progressbar"` и `aria-valuenow`.
- Показатели пинга получили `role="img"` и текстовые `aria-label`.
- `ServerLatency` выделен из `ServersPage`.
- `gitleaks detect` проверен локально на 27 коммитах: утечек нет; CI запускает
  его перед сборкой (бинарник фиксированной версии, сторонний action не нужен).
- План дальнейшего рефакторинга: [`frontend-refactor-plan.md`](frontend-refactor-plan.md).

## Что не устанавливали

- Playwright/E2E skills — потребуют отдельного `npm`-пакета и браузеров.
