# Новая сессия: универсальные теги, Modbus, WebSocket и Lua XIP

## Проект

- Проект: `D:\picoC\4`
- Контроллер: STM32H750VB
- ОС: RT-Thread
- Сеть: lwIP + Mongoose
- Хранилище: W25Q128 QSPI, FAL, FlashDB
- Сборка: IAR `project.ewp`
- Конфигурация `Release` формирует:
  - `internal_flash.hex`
  - `module_slot0.bin`
  - `module_slot1.bin`
  - `module_slot2.bin`
- `prompt.md` изменён пользователем. Не редактировать, не откатывать и не включать в commit.

## Текущее работающее состояние

Нельзя потерять:

- UART2 Modbus float;
- UART7, если используется;
- Qt;
- Ethernet;
- ping/ICMP;
- UDP;
- Web API;
- Web-slot загрузку через Qt;
- FlashDB;
- QSPI module loader;
- profiler slot2;
- безопасный запуск после reset.

DNS и RAW сейчас отключены. Без необходимости не возвращать.

HTML/CSS/JS хранить в QSPI, а не во внутренней Flash.
Не использовать `printf` с float во внутренней прошивке.

## Цель

Реализовать конфигурируемую систему, позволяющую добавлять Modbus-устройства и датчики без перепрограммирования контроллера.

Требования:

- до 5 рабочих портов с возможностью расширения до 8;
- до 30 устройств на порт;
- до 30 датчиков на устройство;
- собственный период опроса каждого устройства;
- настройка устройств и датчиков через Web;
- дерево `порт → устройство → датчик`;
- выбор тегов галочками;
- текущие значения выбранных тегов через WebSocket;
- график архива за выбранный интервал;
- FlashDB-архив;
- простой безопасный байткод выражений;
- в дальнейшем Lua отдельным расширяемым QSPI XIP-модулем.

## Идентификатор тега

Постоянный `tag_id` занимает 16 бит:

```text
15..13 — порт, 3 бита, 0..7
12..8  — устройство, 5 бит, 0..31
7..0   — датчик, 8 бит, 0..255
```

Достоверность не входит в ID, потому что ID не должен меняться при потере связи.

Не использовать C bitfield. Использовать маски и сдвиги:

```c
typedef uint16_t tag_id_t;

#define TAG_ID(port, device, sensor) \
    ((((uint16_t)(port)   & 0x07U) << 13) | \
     (((uint16_t)(device) & 0x1FU) << 8)  | \
      ((uint16_t)(sensor) & 0xFFU))

#define TAG_PORT(id)    (((id) >> 13) & 0x07U)
#define TAG_DEVICE(id)  (((id) >> 8)  & 0x1FU)
#define TAG_SENSOR(id)  ((id) & 0xFFU)
```

## Значение тега

Оставить универсальное 32-битное поле `value_bits` для аналоговых и дискретных значений:

```c
typedef enum
{
    TAG_TYPE_FLOAT32 = 0,
    TAG_TYPE_BOOL    = 1,
    TAG_TYPE_UINT16  = 2,
    TAG_TYPE_INT16   = 3,
    TAG_TYPE_UINT32  = 4,
    TAG_TYPE_INT32   = 5
} tag_type_t;

enum
{
    TAG_FLAG_VALID       = 0x01,
    TAG_FLAG_WRITABLE    = 0x02,
    TAG_FLAG_ARCHIVE     = 0x04,
    TAG_FLAG_COMM_ERROR  = 0x08
};

#pragma pack(push, 1)
typedef struct
{
    tag_id_t id;
    uint8_t type;
    uint8_t flags;
    uint32_t value_bits;
} tag_value_t;
#pragma pack(pop)

static_assert(sizeof(tag_value_t) == 8, "invalid tag_value_t size");
```

Интерпретация `value_bits`:

- `FLOAT32` — IEEE-754;
- `BOOL` — `0` или `1`;
- `UINT16/INT16` — младшие 16 бит;
- `UINT32/INT32` — все 32 бита.

В сетевых форматах явно задавать порядок байтов. Не полагаться на endian компилятора.

## Стабильный Tag API

Основная прошивка должна предоставить единый API:

```c
int tag_read(tag_id_t id, tag_value_t *value);
int tag_write(tag_id_t id, const tag_value_t *value);
int tag_get_float(tag_id_t id, float *value);
int tag_set_float(tag_id_t id, float value);
int tag_set_valid(tag_id_t id, bool valid);
int tag_get_info(tag_id_t id, tag_info_t *info);
int tag_find(const char *name, tag_id_t *id);
int tag_subscribe(...);
```

Через этот API работают:

- Modbus-опрос;
- WebSocket;
- архив;
- байткод;
- Lua;
- будущие XIP-модули.

Основной Tag API должен оставаться коротким и безопасным во внутренней Flash.

## Конфигурация устройств

Для каждого устройства хранить:

- `enabled`;
- порт;
- Modbus slave address;
- имя;
- период опроса;
- timeout;
- retry count;
- до 30 датчиков;
- состояние связи;
- timestamp последнего успешного опроса;
- счётчики успешных опросов и ошибок.

Для каждого датчика:

- `enabled`;
- номер датчика;
- имя;
- единица измерения;
- Modbus function `03` или `04`;
- адрес регистра;
- количество регистров;
- тип данных;
- порядок слов `ABCD/CDAB/BADC/DCBA`;
- scale;
- offset;
- `archive_enabled`;
- отдельный период архива, если он отличается от периода опроса.

Конфигурацию хранить в надёжном FlashDB KVDB либо в согласованном QSPI-объекте. Нужны:

- версия формата;
- CRC;
- строгая валидация;
- значения по умолчанию;
- атомарное обновление;
- безопасное восстановление после повреждения.

Сначала изучить текущую карту FAL/FlashDB и исключить пересечения.

## Периоды опроса

Сохранить совместимость:

```text
0  — выключено
1  — 60 секунд
2  — 120 секунд
3  — 300 секунд
4  — 600 секунд
5  — 1200 секунд
6  — 1800 секунд
7  — 3600 секунд
8  — 7200 секунд
9  — 14400 секунд
10 — 21600 секунд
11 — 28800 секунд
12 — 43200 секунд
13 — 86400 секунд
14 — 10 секунд
15 — 20 секунд
16 — 30 секунд
17 — 1 секунда
```

Реализовать таблицей периодов, а не большим `switch`.

Каждое устройство имеет собственный `next_poll`.

После долгой остановки нельзя выполнять подряд все пропущенные циклы. Назначать следующий опрос как `now + period`.

Опрос не должен зависеть от отдельных полей RTC `Second/Minute/Hour`.

## Планировщик Modbus

Не создавать поток на каждое устройство.

Использовать один планировщик на порт или общий диспетчер портов.

Соседние регистры одного устройства объединять:

- одинаковая функция `03/04`;
- непрерывные или допустимо близкие адреса;
- соблюдение максимума регистров Modbus;
- один ответ декодирует несколько датчиков.

При ошибке:

- относящиеся теги получают `VALID=0`;
- устанавливается `COMM_ERROR`;
- остальные устройства продолжают опрашиваться;
- один timeout не блокирует систему надолго.

Сначала реализовать UART2, но структуры сразу сделать масштабируемыми.

## Архив FlashDB

FlashDB TSDB сам хранит timestamp. Не дублировать timestamp внутри payload.

Не создавать отдельную FlashDB-запись на каждый тег.

Сохранять снимок устройства:

```c
#pragma pack(push, 1)
typedef struct
{
    uint8_t count;
    tag_value_t values[30];
} device_snapshot_t;
#pragma pack(pop)
```

Максимальный payload:

```text
1 + 30 × 8 = 241 байт
```

Архивировать только теги с `TAG_FLAG_ARCHIVE`.

История выбирается по:

- `tag_id`;
- `from`;
- `to`;
- `limit`.

Для длинного интервала делать downsampling:

- минимум;
- максимум;
- среднее;
- либо ограниченную выборку каждой N-й точки.

Перед реализацией рассчитать расход Flash для разных интервалов и числа устройств.

FlashDB archive и KVDB должны оставаться отдельными фиксированными FAL-разделами с wear-level и rollover.

## WebSocket

WebSocket используется для текущих значений выбранных тегов.

Клиент передаёт подписку:

```json
{
  "subscribe": [101, 102, 307]
}
```

Предпочтительный ответ — бинарный пакет:

- версия;
- timestamp;
- count;
- массив `tag_value_t`.

Ограничить количество одновременно выбранных тегов после расчёта RAM, ориентировочно 16–32.

Медленный WebSocket-клиент не должен блокировать Modbus. Старый live-пакет можно заменить новым.

При отключении клиента подписку освобождать.

## Web

HTML/CSS/JS хранить в QSPI.

Интерфейс:

- дерево портов, устройств и датчиков;
- checkbox тегов;
- текущие значения;
- график;
- интервал: час, 6 часов, сутки, неделя, произвольный;
- состояние достоверности;
- легенда и единицы измерения;
- настройка устройств и датчиков;
- позднее редактор формул и Lua.

Не использовать внешние CDN.

Минимальные API:

```text
GET  /api/config/tree
GET  /api/devices
POST /api/devices
PUT  /api/devices/{id}
DELETE /api/devices/{id}

GET  /api/devices/{id}/sensors
POST /api/devices/{id}/sensors
PUT  /api/sensors/{id}
DELETE /api/sensors/{id}

GET /api/history?ids=...&from=...&to=...&limit=...
```

Все размеры, строки, диапазоны и количество элементов строго проверять.

## Простой байткод

Первым скриптовым уровнем сделать небольшой безопасный интерпретатор:

- чтение и запись тегов;
- арифметика;
- сравнения;
- логические операции;
- условия;
- min/max/abs;
- проверка достоверности;
- таймеры.

Ограничить:

- число инструкций;
- размер стека;
- глубину вызовов;
- время одного запуска;
- память.

Ошибка скрипта не должна влиять на Modbus, Web и архив.

## Lua

Lua реализовать позднее отдельным QSPI XIP-модулем поверх того же Tag API.

Lua работает:

- в отдельном низкоприоритетном потоке;
- с ограниченным allocator;
- с instruction hook;
- с лимитом времени;
- под watchdog;
- без прямого доступа к оборудованию и памяти.

Исключить ненужные библиотеки:

- `io`;
- `os`;
- `package`;
- `debug`;
- файловую систему;
- динамические библиотеки.

Оставить необходимые:

- base;
- table;
- string;
- math по необходимости;
- `tag.*`;
- `timer.*`;
- `log.*`.

Режимы запуска:

- `on_change`;
- `periodic`;
- `manual`.

Изменения тегов собирать в пакет. Не вызывать Lua отдельно на каждое изменение.

При сбое Lua должны продолжать работать UART, Modbus, Web, архив и остальные модули.

Размер Lua заранее жёстко не ограничивать. Он занимает необходимое число последовательных QSPI-блоков по 4 КБ.

## Единый QSPI-формат OBJ1

Использовать единый формат для загружаемых объектов:

- XIP-модуль;
- Web-файл;
- Lua VM;
- Lua-скрипт;
- байткод;
- профиль устройства;
- конфигурация;
- словарь тегов.

Минимальная единица размещения — 4 КБ:

```c
#define QSPI_OBJECT_BLOCK_SIZE 4096U
#define QSPI_OBJECT_MAGIC      0x314A424FUL /* "OBJ1" */
```

Объект занимает столько последовательных блоков, сколько требуется:

```c
block_count =
    (header_size + payload_size + 4095U) / 4096U;
```

Примеры:

```text
Web 1837 байт → 1 блок = 4 КБ
модуль 9 КБ   → 3 блока = 12 КБ
Lua 105 КБ    → 27 блоков = 108 КБ
Lua 170 КБ    → 43 блока = 172 КБ
```

Общий заголовок 128 байт:

```c
#define QSPI_OBJECT_HEADER_SIZE 128U

#pragma pack(push, 1)
typedef struct
{
    uint32_t magic;

    uint16_t format_version;
    uint16_t header_size;

    uint16_t object_type;
    uint16_t flags;

    uint32_t object_id;
    uint32_t generation;

    uint32_t payload_size;
    uint32_t payload_crc32;

    uint16_t required_api_version;
    uint16_t content_type;

    uint32_t entry_offset;
    uint32_t link_address;

    char name[40];

    uint8_t reserved[40];

    uint32_t header_crc32;
} qspi_object_header_t;
#pragma pack(pop)

static_assert(sizeof(qspi_object_header_t) == 128,
              "invalid QSPI object header");
```

Типы:

```c
typedef enum
{
    QSPI_OBJECT_NONE           = 0,
    QSPI_OBJECT_XIP_MODULE     = 1,
    QSPI_OBJECT_WEB_FILE       = 2,
    QSPI_OBJECT_LUA_VM         = 3,
    QSPI_OBJECT_LUA_SCRIPT     = 4,
    QSPI_OBJECT_BYTECODE       = 5,
    QSPI_OBJECT_DEVICE_PROFILE = 6,
    QSPI_OBJECT_CONFIGURATION  = 7,
    QSPI_OBJECT_TAG_DICTIONARY = 8
} qspi_object_type_t;
```

Флаги:

```c
enum
{
    QSPI_OBJECT_FLAG_VALID       = 0x0001,
    QSPI_OBJECT_FLAG_EXECUTABLE  = 0x0002,
    QSPI_OBJECT_FLAG_AUTOSTART   = 0x0004,
    QSPI_OBJECT_FLAG_READONLY    = 0x0008,
    QSPI_OBJECT_FLAG_COMPRESSED  = 0x0010,
    QSPI_OBJECT_FLAG_SYSTEM      = 0x0020,
    QSPI_OBJECT_FLAG_RECOVERY    = 0x0040
};
```

Для XIP:

- `entry_offset`;
- `link_address`;
- `required_api_version`;
- `EXECUTABLE`;
- `AUTOSTART` при необходимости.

Для Web:

- `name` содержит URL;
- `content_type` содержит HTML/JS/CSS/PNG;
- `entry_offset=0`;
- `link_address=0`.

## Каталог объектов

Использовать общий каталог:

```c
#pragma pack(push, 1)
typedef struct
{
    uint32_t object_id;
    uint16_t first_block;
    uint16_t block_count;
    uint16_t object_type;
    uint16_t flags;
    uint32_t generation;
    uint32_t name_hash;
} qspi_directory_entry_t;
#pragma pack(pop)
```

Каталог хранить в двух копиях A/B.

Каждая копия содержит:

- magic;
- generation;
- CRC;
- количество объектов;
- bitmap занятых блоков;
- entries.

При запуске выбирать корректную копию с максимальным `generation`.

## Атомарное обновление

Нельзя стирать рабочий объект до проверки нового.

Порядок:

1. найти свободный непрерывный диапазон;
2. записать новый OBJ1;
3. прочитать обратно;
4. проверить payload CRC;
5. проверить header CRC;
6. для XIP проверить адрес и entry point;
7. записать новую копию каталога;
8. только после этого освободить старые блоки.

После потери питания должен остаться либо старый, либо новый объект.

## XIP и фрагментация

XIP-объект занимает непрерывные блоки.

Существующие IAR-модули имеют абсолютные адреса. На первом этапе назначать стартовый блок до сборки и линковать образ для этого адреса.

Позднее можно рассмотреть position-independent код.

Системные XIP-модули располагать в начале объектной области. Web, скрипты и профили — после XIP.

Работающий XIP-модуль автоматически не перемещать.

Для Lua оставлять запас последовательных свободных блоков, но формат должен позволять расширение до фактически требуемого размера.

## Legacy

Сохранить первые 20 старых module-slot по 4 КБ:

```text
legacy slots 0..19 = 20 × 4 КБ
```

Новый загрузчик сначала ищет OBJ1, затем при необходимости legacy-module.

Текущий старый Web-раздел начинается близко к legacy-области. Нельзя менять адреса без миграции.

Сначала составить полную новую карту W25Q128:

```text
legacy modules
OBJ1 directory A/B
OBJ1 object area
FlashDB configuration
FlashDB archive
storage
резерв восстановления/обновления
```

Точные адреса определять только после аудита текущей FAL-карты.

## Универсальный загрузчик

Qt должен использовать один протокол для всех OBJ1:

1. выбрать файл;
2. выбрать тип объекта;
3. сформировать заголовок;
4. передать заголовок и payload;
5. контроллер распределяет блоки;
6. контроллер пишет и проверяет CRC;
7. контроллер атомарно переключает каталог.

Типы в Qt:

- XIP module;
- Web file;
- Lua VM;
- Lua script;
- bytecode;
- device profile;
- configuration;
- tag dictionary.

Номер фиксированного Web-slot больше не должен определять URL. Web ищет объект по hash URL и затем выполняет точное сравнение `name`.

## Безопасность XIP

Перед запуском проверить:

- magic;
- format version;
- header size;
- object type;
- payload size;
- block count;
- payload CRC;
- header CRC;
- API version;
- link address;
- entry offset;
- отсутствие выхода за диапазон;
- отсутствие пересечений;
- executable flag.

При остановке удалить hook модуля до удаления его потока.

Сбой XIP-модуля не должен повреждать основную прошивку и другие модули.

## Этапы реализации

### Этап 1

- аудит FAL/QSPI/FlashDB;
- полная карта W25Q128;
- расчёт RAM и Flash;
- формат OBJ1;
- каталог A/B;
- документ миграции;
- без прошивки оборудования.

### Этап 2

- Tag Registry;
- Tag API;
- тесты packing/unpacking;
- совместимость с текущим UART2 float.

### Этап 3

- конфигурация устройств;
- KVDB;
- универсальный планировщик UART2;
- два тестовых устройства;
- несколько типов датчиков.

### Этап 4

- до 30 устройств;
- группировка регистров;
- достоверность;
- диагностика.

### Этап 5

- FlashDB archive;
- history API;
- фильтрация и downsampling.

### Этап 6

- Web tree;
- WebSocket;
- график;
- редактор конфигурации.

### Этап 7

- простой байткод выражений.

### Этап 8

- Lua VM как расширяемый OBJ1 XIP;
- Lua Tag API;
- allocator, instruction limit, watchdog.

## Правила выполнения

Перед изменениями:

1. изучить существующий код;
2. проверить `git status`;
3. сохранить пользовательские изменения;
4. не трогать `prompt.md`;
5. представить короткий план файлов и рисков.

После каждого этапа:

1. полная сборка IAR Release;
2. проверить readonly code и RAM;
3. не прошивать при ошибках или переполнении;
4. прошивать только необходимые образы;
5. проверить UART, Qt, Web, ping, UDP, FlashDB и QSPI;
6. отдельный Git commit;
7. `prompt.md` не включать в commit.

Не пытаться реализовать всё сразу.

Начать с этапа 1: аудит и точная карта памяти. Затем сделать минимальный вертикальный срез Tag API + UART2, оставляя контроллер рабочим после каждого изменения.
