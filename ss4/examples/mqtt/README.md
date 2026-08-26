# Примеры MQTT-визуализации

Папка содержит минимальные шаблоны для бесплатного self-hosted контура:

```text
ss4 -> Mosquitto -> Node-RED live dashboard
ss4 -> Mosquitto -> Telegraf -> InfluxDB -> Grafana
```

## Файлы

- `mosquitto.conf.example` — локальный broker без авторизации для стенда.
- `telegraf.conf.example` — ingestion `ss4/v1/values/+` в InfluxDB 2.x.
- `node_red_flow.json` — стартовый flow для просмотра `status`, `health`, `values`, `alarms`.
- `node_red_dashboard_flow.json` — live-экран для `node-red-dashboard`: статус, health и таблица последних значений.
- `node_red_http_dashboard_flow.json` — надежная HTTP-панель на core-узлах Node-RED, без dashboard-пакетов.
- `SS4_HTTP_DASHBOARD_RU.md` — подробное описание HTTP-панели `/ss4`.
- `SS4_HTTP_DASHBOARD_TESTS_RU.md` — ручные и командные тесты панели.
- `SS4_OPERATIONS_CHECKLIST_RU.md` - быстрый чеклист запуска и диагностики Mosquitto, `ss4`, Node-RED и панели `/ss4`.
- `MQTT_VISUALIZERS_RU.md` — сравнение MQTT-визуализаторов для `ss4`.

## Быстрая проверка topics

После включения `[mqtt]` в `ss4.toml` подпишитесь на:

```text
ss4/v1/#
```

Ожидаемые сообщения:

- `ss4/v1/status` -> `online`
- `ss4/v1/health` -> JSON с `kind` и `message`
- `ss4/v1/values/{kpz_id}` -> JSON batch значений с именами КПЗ/регистров и адресами
- `ss4/v1/alarms/{kpz_id}/{rule_id}` -> JSON alarm event

## Безопасность

`mosquitto.conf.example` предназначен только для локального стенда. Для сети включите:

- `allow_anonymous false`
- password file
- TLS listener
- ACL на topics `ss4/v1/#`
