# Тесты HTTP-панели `ss4`

Адрес панели:

```text
http://127.0.0.1:1880/ss4
```

## 1. Проверка процессов

```powershell
netstat -ano | Select-String ':1883'
netstat -ano | Select-String ':1880'
Get-Process | Where-Object { $_.ProcessName -in @('ss4','node','mosquitto') } | Select-Object ProcessName,Id,Path
```

Ожидаемо:

- `1883` слушает Mosquitto;
- `1880` слушает Node-RED;
- есть процесс `ss4.exe`;
- между `ss4` и Mosquitto есть `ESTABLISHED`;
- между Node-RED и Mosquitto есть `ESTABLISHED`.

## 2. Проверка страницы

Открыть:

```text
http://127.0.0.1:1880/ss4
```

Ожидаемо:

- виден заголовок `ss4 MQTT`;
- виден `ws connected`;
- виден сводный индикатор состояния `OK`, `NO DATA`, `NO LIVE`, `BAD`, `STALE` или `ALARMS`;
- есть поле `filter`;
- есть режимы `all`, `bad`, `stale`;
- есть выбор лимита строк;
- есть выбор порога `stale`;
- есть кнопки `export values` и `export alarms`;
- есть кнопка `reset view`;
- есть кнопка `refresh snapshot`;
- есть карточки `status`, `health`, `visible / filtered / total`, `stale`, `bad quality`, `active alarms`, `alarm states`, `msg/min`, `last msg`, `snapshot`;
- есть блоки `KPZ / group summary`, `Selected value`, `Latest values`, `Alarm states`.

## 3. Тест status/health/values/alarms

Команда публикует тестовые MQTT-сообщения без изменения `ss4` и БД:

```powershell
node -e "const {execFileSync}=require('child_process'); const exe='C:/Program Files/mosquitto/mosquitto_pub.exe'; const health=JSON.stringify({kind:'health_ok',message:'manual panel test'}); const values=JSON.stringify({ts:1777540000,kpz_id:999,kpz_name:'test',values:[{reg_id:1,addr:1,name:'test_ok',group_id:1,tip:3,value:12.34567,quality:'ok'},{reg_id:2,addr:2,name:'test_bad',group_id:1,tip:3,value:98.76543,quality:'bad'}]}); const alarm=JSON.stringify({kpz_id:999,reg_id:2,rule_id:2,event:'start',value:98.76543,severity:2,code:'TEST',message:'manual alarm'}); for (const [topic,payload] of [['ss4/v1/status','online'],['ss4/v1/health',health],['ss4/v1/values/999',values],['ss4/v1/alarms/999/2',alarm]]) execFileSync(exe,['-h','127.0.0.1','-p','1883','-t',topic,'-m',payload]);"
```

Ожидаемо на панели:

- `status = online`;
- `health` содержит `health_ok`;
- `visible / total` показывает тестовые значения;
- `bad quality = 1`;
- `active alarms = 1`, `alarm states = 1`;
- `msg/min` больше `0`;
- сводный индикатор показывает `BAD` или `ALARMS`, потому что тест отправляет плохое качество и аварию;
- в таблице есть `test_ok` и `test_bad`;
- строка `test_bad` подсвечена красным;
- в `Alarm states` есть `manual alarm`.

## 4. Тест фильтра

В поле `filter` ввести:

```text
test_bad
```

Ожидаемо:

- в `Latest values` остается строка `test_bad`;
- `test_ok` скрывается.

Нажать `clear filter`.

Ожидаемо:

- снова видны все строки.

## 5. Тест режимов all/bad/stale

1. Выбрать `bad`.
2. Убедиться, что видны только строки с `quality` не `ok`.
3. Выбрать `all`.
4. Убедиться, что снова видны все строки.
5. Выбрать порог `stale 30s`.
6. Подождать больше 30 секунд и выбрать `stale`.

Ожидаемо:

- в режиме `stale` видны значения, которые давно не обновлялись;
- карточка `stale` становится больше `0`.

## 6. Тест сортировки

Кликнуть заголовки таблицы `Latest values`: `kpz`, `group`, `reg`, `addr`, `name`, `value`, `quality`, `age`.

Ожидаемо:

- строки сортируются по выбранной колонке;
- повторный клик меняет направление сортировки.

## 7. Тест KPZ / group summary

1. Найти строку в `KPZ / group summary`.
2. Кликнуть по ней.

Ожидаемо:

- поле `filter` заполняется выбранными `kpz/group`;
- таблица `Latest values` показывает значения этой группы.

## 8. Тест Selected value и мини-графика

1. Кликнуть любую числовую строку в `Latest values`.
2. Подождать несколько обновлений этого параметра.

Ожидаемо:

- блок `Selected value` показывает выбранный параметр;
- мини-график обновляется;
- видны `min`, `max`, `avg`, `samples`.

Нажать `clear trend`.

Ожидаемо:

- мини-график выбранного параметра очищается;
- после следующего значения `samples` начинается заново.

## 9. Тест pause/resume

1. Нажать `pause`.
2. Опубликовать новое значение:

```powershell
node -e "const {execFileSync}=require('child_process'); const exe='C:/Program Files/mosquitto/mosquitto_pub.exe'; const values=JSON.stringify({ts:1777540001,kpz_id:999,kpz_name:'test',values:[{reg_id:3,addr:3,name:'paused_value',group_id:1,tip:3,value:33.44,quality:'ok'}]}); execFileSync(exe,['-h','127.0.0.1','-p','1883','-t','ss4/v1/values/999','-m',values]);"
```

3. Проверить, что строка `paused_value` еще не появилась.
4. Проверить, что кнопка показывает `resume (1)` или больше, если во время паузы пришло несколько сообщений.
5. Нажать `resume`.

Ожидаемо:

- после `resume` строка `paused_value` появляется.
- счетчик на кнопке возвращается к обычному `pause`.

## 10. Тест clear alarms и агрегации

1. Опубликовать несколько событий по одному ключу `kpz/reg/rule`, например `on/off/on`.
2. Убедиться, что в таблице `Alarm states` есть одна строка по этому ключу, а `repeats` больше `1`.
3. Кликнуть строку аварии.
4. Проверить, что поле `filter` заполнилось по `kpz/reg`, а `Selected value` показывает связанный параметр, если он уже есть в `Latest values`.
5. Нажать `clear alarms`.

Ожидаемо:

- таблица `Alarm states` очищается;
- карточки `active alarms` и `alarm states` показывают `0`;
- это очищение только на экране, MQTT/БД не меняются.

## 11. Проверка flow-файлов

## 11. Тест CSV-экспорта

1. Убедиться, что в `Latest values` есть строки.
2. Нажать `export values`.
3. Убедиться, что браузер предлагает CSV-файл с именем вида `ss4-values-...csv`.
4. Убедиться, что в `Alarm states` есть хотя бы одна строка.
5. Нажать `export alarms`.
6. Убедиться, что браузер предлагает CSV-файл с именем вида `ss4-alarm-states-...csv`.

Ожидаемо:

- экспорт `values` содержит только видимые строки с учетом фильтра, режима и лимита;
- экспорт `alarms` содержит текущие агрегированные состояния аварий на экране;
- данные никуда не отправляются, CSV создается локально в браузере.

## 12. Проверка flow-файлов

## 12. Тест snapshot после перезагрузки

1. Убедиться, что в `Latest values` уже есть данные.
2. Перезагрузить страницу `/ss4`.

Ожидаемо:

- таблица не остается пустой до следующего MQTT-сообщения;
- последние значения подтягиваются через `/ss4api`;
- после этого live-обновления продолжают приходить через WebSocket.
- при переподключении WebSocket страница снова подтягивает `/ss4api`, чтобы догнать пропущенные сообщения.
- кнопка `refresh snapshot` вручную подтягивает тот же snapshot без перезагрузки страницы.
- карточка `snapshot` показывает время последней загрузки и счетчики `values/alarm states`.

Проверить API отдельно:

```powershell
Invoke-RestMethod -Uri 'http://127.0.0.1:1880/ss4api'
```

Ожидаемо:

- JSON содержит `status`, `health`, `values`, `alarms`, `alarm_states`.

## 13. Проверка flow-файлов

```powershell
node -e "for (const f of ['examples/mqtt/node_red_flow.json','examples/mqtt/node_red_dashboard_flow.json','examples/mqtt/node_red_http_dashboard_flow.json']) { JSON.parse(require('fs').readFileSync(f,'utf8')); console.log(f + ' OK'); }"
```

Ожидаемо:

```text
examples/mqtt/node_red_flow.json OK
examples/mqtt/node_red_dashboard_flow.json OK
examples/mqtt/node_red_http_dashboard_flow.json OK
```

## 14. Проверка JavaScript страницы

```powershell
$html = (Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:1880/ss4' -TimeoutSec 5).Content
$script = [regex]::Match($html, '<script>([\s\S]*?)</script>').Groups[1].Value
Set-Content -Path $env:TEMP\ss4_page_script.js -Value $script -Encoding UTF8
node --check $env:TEMP\ss4_page_script.js
```

Ожидаемо:

```text
Syntax check passed
```

`node --check` обычно не печатает текст при успехе. Главное - код выхода `0`.

## 15. Тест частых MQTT-обновлений

Команда быстро публикует серию значений:

```powershell
node -e "const {execFileSync}=require('child_process'); const exe='C:/Program Files/mosquitto/mosquitto_pub.exe'; for (let i=0;i<50;i++){ const values=JSON.stringify({ts:1777541000+i,kpz_id:997,kpz_name:'burst-test',values:[{reg_id:1,addr:1,name:'burst_value',group_id:1,tip:3,value:i,quality:'ok'}]}); execFileSync(exe,['-h','127.0.0.1','-p','1883','-t','ss4/v1/values/997','-m',values]); }"
```

Ожидаемо:

- страница остается отзывчивой;
- `msg/min` растет;
- `burst_value` обновляется до последнего опубликованного значения;
- таблица не должна заметно дергаться при каждом отдельном MQTT-сообщении.

## 16. Тест сохранения настроек просмотра

1. Ввести в `filter` любое значение, например `997`.
2. Выбрать режим `bad` или `stale`.
3. Выбрать лимит строк, например `1000`.
4. Выбрать порог `stale 120s`.
5. Кликнуть по заголовку любой колонки для сортировки.
6. Перезагрузить страницу.

Ожидаемо:

- `filter` восстановился;
- режим `all/bad/stale` восстановился;
- лимит строк восстановился;
- порог `stale` восстановился;
- сортировка осталась выбранной.

Настройки хранятся только локально в браузере через `localStorage`.

Нажать `reset view`.

Ожидаемо:

- `filter` очищен;
- режим стал `all`;
- лимит строк стал `500`;
- порог `stale` стал `30s`;
- сортировка вернулась к `kpz`.

## 17. Проверка Rust-кода MQTT

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Ожидаемо:

- форматирование проходит;
- clippy без warning;
- unit tests проходят.

## 18. Что не тестируется этой панелью

- Долгая история значений.
- Управление оборудованием.
- Запись команд.
- Подтверждение действий оператора.

Это намеренно: текущая панель только для мониторинга.
