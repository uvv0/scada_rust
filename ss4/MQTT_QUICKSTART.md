# Быстрый старт MQTT для `ss4`

Документ описывает минимальный локальный контур проверки MQTT-публикации без изменения логики опроса.

## Схема

```text
ss4 -> Mosquitto -> MQTTX / Node-RED / Grafana
```

`ss4` публикует только outbound-события. Команды через MQTT в MVP не принимаются.

Готовые шаблоны лежат в `examples/mqtt/`:
- `mosquitto.conf.example`
- `node_red_flow.json`
- `node_red_dashboard_flow.json`
- `node_red_http_dashboard_flow.json`
- `SS4_HTTP_DASHBOARD_RU.md`
- `SS4_HTTP_DASHBOARD_TESTS_RU.md`
- `MQTT_VISUALIZERS_RU.md`
- `telegraf.conf.example`

## Topics

- `ss4/v1/status` — retained `online`, LWT `offline`
- `ss4/v1/health` — retained JSON health snapshot
- `ss4/v1/values/{kpz_id}` — batch значений с `kpz_name`, `reg_id`, `addr`, `name`, `group_id`, `tip`, `value`, `quality`
- `ss4/v1/alarms/{kpz_id}/{rule_id}` — событие аварии

## Минимальный `ss4.toml`

```toml
[mqtt]
enabled = true
host = "127.0.0.1"
port = 1883
client_id = "ss4"
username_env = "MQTT_USER"
password_env = "MQTT_PASS"
topic_prefix = "ss4/v1"
queue_cap = 1000
qos = 1
retain_health = true
publish_values = true
publish_alarms = true
# value_kpz_ids = [3]
# value_group_ids = [21]
# value_reg_ids = [6001, 6002]
```

Если локальный broker без логина и пароля, `MQTT_USER` и `MQTT_PASS` можно оставить пустыми.

Для проверки live-экрана удобно временно ограничить поток:

```toml
value_kpz_ids = [3]
value_group_ids = [21]
```

Если включены несколько фильтров, они работают вместе: например, пример выше пропустит только значения КПЗ 3 из группы 21.

## Проверка через MQTTX

1. Подключиться к broker `127.0.0.1:1883`.
2. Подписаться на `ss4/v1/#`.
3. Запустить `ss4`.
4. Проверить:
   - retained `ss4/v1/status = online`
   - сообщения `ss4/v1/health`
   - batch-и `ss4/v1/values/{kpz_id}`
   - alarm-события `ss4/v1/alarms/{kpz_id}/{rule_id}` при переходах аварий

## Node-RED Dashboard

Минимальный flow:

```text
mqtt in ss4/v1/values/+ -> json -> function/group by kpz/reg -> ui_chart/ui_gauge
mqtt in ss4/v1/alarms/+/+ -> json -> ui_table/ui_toast
mqtt in ss4/v1/health -> json -> ui_text
```

Можно импортировать стартовый flow из `examples/mqtt/node_red_flow.json`.

Для полноценного live-экрана установите бесплатный dashboard-пакет в каталоге Node-RED:

```powershell
cd $env:USERPROFILE\.node-red
npm install node-red-dashboard
```

После перезапуска Node-RED импортируйте `examples/mqtt/node_red_dashboard_flow.json`. Dashboard обычно открывается по адресу `http://127.0.0.1:1880/ui`.

Для live-экрана лучше читать `values/{kpz_id}` и обновлять последние значения в flow context.

Если старый `node-red-dashboard` плохо отрисовывается, используйте вариант без дополнительных пакетов: импортируйте `examples/mqtt/node_red_http_dashboard_flow.json` и откройте `http://127.0.0.1:1880/ss4`.

Подробное описание панели и тесты лежат в:

- `examples/mqtt/SS4_HTTP_DASHBOARD_RU.md`
- `examples/mqtt/SS4_HTTP_DASHBOARD_TESTS_RU.md`
- `examples/mqtt/MQTT_VISUALIZERS_RU.md`

## Grafana

Grafana не является MQTT historian сама по себе. Для графиков нужен промежуточный storage:

```text
ss4 -> MQTT -> Telegraf или Node-RED -> InfluxDB/TimescaleDB -> Grafana
```

Рекомендуемый путь:
1. MQTT consumer разбирает `values/{kpz_id}`.
2. Каждое значение пишет measurement:
   - `kpz_id`
   - `reg_id`
   - `tip`
   - `value`
   - `quality`
3. Grafana строит dashboards по `kpz_id/reg_id`.

Стартовый Telegraf-конфиг лежит в `examples/mqtt/telegraf.conf.example`.

## Политика отказов

- MQTT broker недоступен: scheduler продолжает опрос.
- Очередь MQTT переполнена: событие отбрасывается с warning в log.
- `status` использует retained `online` и LWT `offline`.
- `health` публикуется retained, чтобы новый subscriber сразу видел последнее состояние.
