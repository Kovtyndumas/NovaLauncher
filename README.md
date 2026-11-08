# Nova Launcher

Десктопный лаунчер Minecraft на Electron.

- Инстансы Vanilla, Fabric, Quilt, Forge и NeoForge.
- Каталог официальных релизов от 1.0 и автоматический подбор Java.
- Поиск контента Modrinth, управление модами, ресурспаками и шейдерами.
- Копирование инстансов, резервные копии миров, откат модов и история запусков.
- Discord Rich Presence и три темы оформления.
- Версия инстанса 26.3 RUST с автоматической установкой Horizon.

## Запуск

Установите Node.js LTS, затем в папке проекта:

```sh
npm ci
npm start
```

## Проверки

```sh
npm test
npm run test:ui
```

Проверка настоящего main/preload с временной базой и доступом к каталогу Mojang: `npm run test:main`.

## Сборка

Собирайте на соответствующей операционной системе:

```sh
npm run dist:mac
npm run dist:win
npm run dist:linux
```

Инструкции по Releases, подписи приложений, обновлениям и настройке Horizon: [BUILD.md](BUILD.md).

Для 26.3 RUST Nova скачивает Horizon; перед игрой нужен вход в аккаунт Microsoft с Java Edition через кнопку «Войти в Horizon». Наличие версии Minecraft в каталоге не означает поддержку всех загрузчиков и всех платформ. Для старых версий на Apple Silicon может потребоваться Rosetta 2.

Created By Ktvx
