# Эксплуатационный чеклист MQTT/Node-RED для `ss4`

Этот файл нужен для быстрого запуска и диагностики стенда:

```text
ss4 -> Mosquitto -> Node-RED -> http://127.0.0.1:1880/ss4
```

## 1. Что должно быть запущено

Минимальный набор:

- Mosquitto broker на `127.0.0.1:1883`;
- `ss4.exe` с включенным `[mqtt]`;
- Node-RED на `127.0.0.1:1880`;
- flow `examples/mqtt/node_red_http_dashboard_flow.json`;
- браузер открыт на `http://127.0.0.1:1880/ss4`.

## 2. Проверка портов

```powershell
netstat -ano | Select-String ':1883'
netstat -ano | Select-String ':1880'
```

Ожидаемо:

- `:1883` слушает Mosquitto;
- `:1880` слушает Node-RED.

Если `1883` не слушает, MQTT broker не запущен.

Если `1880` не слушает, Node-RED не запущен, и браузер покажет `ERR_CONNECTION_REFUSED`.

## 3. Проверка процессов

```powershell
Get-Process | Where-Object { $_.ProcessName -in @('ss4','node','mosquitto') } | Select-Object ProcessName,Id,Path
```

Ожидаемо:

- `mosquitto` - MQTT broker;
- `ss4` - основной сервис;
- `node` - Node-RED.

## 4. Запуск Mosquitto вручную

Если Mosquitto не запущен как служба:

```powershell
& "C:\Program Files\mosquitto\mosquitto.exe" -c "C:\andr\my2\ss4\examples\mqtt\mosquitto.conf.example" -v
```

Если появляется ошибка вида "обычно разрешается только одно использование адреса сокета", порт `1883` уже занят. Это часто означает, что Mosquitto уже запущен как служба.

## 5. Запуск Node-RED

```powershell
node-red
```

После запуска открыть:

```text
http://127.0.0.1:1880/
```

Если браузер показывает `ERR_CONNECTION_REFUSED`, Node-RED не слушает порт `1880` или запущен на другом порту.

## 6. Импорт flow

В Node-RED:

1. Открыть меню справа сверху.
2. Выбрать `Import`.
3. Вставить JSON из файла:

```text
C:\andr\my2\ss4\examples\mqtt\node_red_http_dashboard_flow.json
```

4. Нажать `Import`.
5. Нажать `Deploy`.

После этого открыть:

```text
http://127.0.0.1:1880/ss4
```

## 7. Проверка MQTT через подписку

```powershell
& "C:\Program Files\mosquitto\mosquitto_sub.exe" -h 127.0.0.1 -p 1883 -t "ss4/v1/#" -v
```

Ожидаемо приходят topics:

- `ss4/v1/status`;
- `ss4/v1/health`;
- `ss4/v1/values/{kpz_id}`;
- `ss4/v1/alarms/{kpz_id}/{rule_id}`, если есть аварии.

Если подписка подключилась, но сообщений нет, проверьте настройки `[mqtt]` в `ss4.toml` и запущен ли `ss4.exe`.

## 8. Быстрая ручная публикация

Команда отправляет тестовое значение в MQTT:

```powershell
node -e "const {execFileSync}=require('child_process'); const exe='C:/Program Files/mosquitto/mosquitto_pub.exe'; const values=JSON.stringify({ts:1777540000,kpz_id:999,kpz_name:'test',values:[{reg_id:1,addr:1,name:'test_ok',group_id:1,tip:3,value:12.34567,quality:'ok'}]}); execFileSync(exe,['-h','127.0.0.1','-p','1883','-t','ss4/v1/values/999','-m',values]);"
```

Ожидаемо:

- в MQTTX или `mosquitto_sub` видно сообщение;
- на `/ss4` появляется строка `test_ok`;
- карточка `msg/min` увеличивается.

## 9. Если `/ss4` открывается, но данных нет

Проверьте по порядку:

1. Есть ли `ws connected` в верхней строке панели.
2. Есть ли MQTT-сообщения в MQTTX или `mosquitto_sub`.
3. Есть ли connection status у MQTT-узлов в Node-RED.
4. Нажат ли `Deploy` после импорта flow.
5. Совпадают ли broker/port в flow: `127.0.0.1:1883`.

## 10. Если MQTTX не подключается

Проверить:

- host: `127.0.0.1`;
- port: `1883`;
- protocol: MQTT, не WebSocket;
- username/password пустые для локального тестового `mosquitto.conf.example`;
- Clean Start включен;
- topic подписки: `ss4/v1/#`.

## 11. Если сообщения приходят hex-строкой

В MQTTX выберите отображение payload как `Plaintext` или `JSON`. Hex вида:

```text
7b22 6b69 6e64 ...
```

это обычный JSON, показанный в шестнадцатеричном виде.

## 12. Если `ss4` публикует слишком много параметров

Для 10000 параметров:

- не открывайте постоянно все строки без фильтра;
- используйте режимы `bad` и `stale`;
- ставьте лимит `500` или `1000`;
- для общей истории используйте БД или `Telegraf -> InfluxDB/Grafana`;
- MQTT payload лучше держать batch-сообщениями по КПЗ, как сейчас `ss4/v1/values/{kpz_id}`.

## 13. Безопасность

`mosquitto.conf.example` подходит только для локального стенда. Для сети включите:

- `allow_anonymous false`;
- password file;
- ACL на topics `ss4/v1/#`;
- TLS listener, если broker доступен вне локальной машины.

## 14. Быстрый диагноз

| Симптом | Вероятная причина | Что проверить |
| --- | --- | --- |
| `ERR_CONNECTION_REFUSED` на `:1880` | Node-RED не запущен | `netstat -ano | Select-String ':1880'` |
| MQTTX не подключается к `:1883` | Mosquitto не запущен | `netstat -ano | Select-String ':1883'` |
| `/ss4` открыт, но пусто | Нет MQTT-сообщений или flow не deployed | `mosquitto_sub`, Node-RED `Deploy` |
| Видны `health`, но нет `values` | `ss4` не публикует значения или фильтры `[mqtt]` слишком узкие | настройки `value_*` в `ss4.toml` |
| Все строки быстро становятся `stale` | данные перестали обновляться | процесс `ss4`, связь с оборудованием, broker |
| Hex вместо JSON | режим отображения в MQTTX | переключить payload на `JSON` или `Plaintext` |
