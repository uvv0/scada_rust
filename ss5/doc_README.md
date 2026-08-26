# STM32H750VB + LAN8720 + RT-Thread/lwIP

## Lua

- `LUA_XIP.md` — Lua 5.4 VM, доступные библиотеки, Tag API и лимиты исполнения;
- `LUA_WEB.md` — Web-редактор `/lua`, атомарная запись скриптов, запуск VM,
  HTTP API, диагностика и безопасность.

Дополнительная документация:

```text
applications/arx/README.md                         ELAM Modbus, Holding FlashDB и архив
applications/modules/MODULE_FORMAT.md              формат загружаемого модуля
applications/modules/MODULE_IAR_UART7_EXAMPLE.md   пример модуля UART7
applications/modules/MODULE_SLOT1_USART2.md        пример модуля USART2/RS-485
applications/modules/QSPI_XIP_FLASHDB.md           раздельные QSPI XIP и SPI4 FlashDB
applications/arx/SPI4_W25Q128_FLASHDB.md           U4, SPI4, проверка и диагностика
applications/modules/MODULE_UPLOAD_ELAM.md         загрузка модулей через ELAM Modbus
OBJ1_STAGE1_MEMORY_AUDIT.md                        новая карта, формат OBJ1 и расчёты памяти
OBJ1_STAGE2_LOADER.md                              приёмник, allocator, атомарный каталог A/B и host-тест
PROGRAM_EXTERNAL_FLASH.md                          запись модулей через J-Link
WEB_SLOTS.md                                       загрузка веб-файлов в QSPI
TAG_REGISTRY.md                                    универсальные теги и API
```

## Текущее состояние

Состояние на 30.07.2026: Ethernet работает. Плата получает и передаёт
RMII-пакеты и отвечает на `ping 192.168.1.100` с компьютера, подключённого
непосредственно по Ethernet.

Реализованы:

- ELAM Modbus на UART8;
- FlashDB для Holding-регистров и архива;
- первая W25Q128 через QSPI/FAL для XIP-модулей и веб-области;
- вторая W25Q128FVSG (U4) через SPI4/FAL для Holding и архива FlashDB;
- 20 исполняемых непосредственно из QSPI модулей по 4 КиБ;
- загрузка модулей через J-Link или Modbus;
- 254 веб-слота по 64 КиБ с загрузкой через Qt/ELAM и проверкой CRC32;
- запуск и остановка модулей через Modbus;
- сервисы UART7 и USART2/RS-485 для модулей;
- профилировщик потоков RT-Thread.

Рабочая конфигурация:

- MCU: STM32H750VBT6;
- PHY: LAN8720A, RMII;
- RTOS: RT-Thread;
- TCP/IP: lwIP 2.0.2;
- драйвер ETH: официальный STM32H7 HAL ETH;
- компилятор/отладчик: IAR EWARM 9.30.1;
- программатор: J-Link, SWD 1 МГц;
- программа выполняется из внутренней Flash STM32H750;
- основная программа и таблица векторов находятся во внутренней Flash;
- модули выполняются непосредственно из W25Q128 через QSPI/XIP.

## Сборка и загрузка

Открыть `project.eww`, выбрать конфигурацию `Release`, выполнить:

```text
Project -> Rebuild All
Project -> Download and Debug
```

Выходные файлы:

```text
Release/Exe/eth_official.out
Release/Exe/project.hex
Release/Exe/internal_flash.hex
Release/Exe/internal_flash.bin
Release/Exe/module_slot0.bin
Release/Exe/module_slot1.bin
Release/List/eth_official.map
```

Для записи основной Release-прошивки через J-Link использовать:

```powershell
& "C:\Program Files\SEGGER\JLink\JLink.exe" -CommanderScript "D:\picoC\4\program_internal_flash.jlink"
```

Скрипт загружает `Release/Exe/internal_flash.hex` по адресу `0x08000000`.
Команда `loadfile` сама выполняет программирование и проверку. Отдельная
команда `verifyfile` в используемой версии J-Link Commander не требуется.

`eth_official.out` является ELF-файлом IAR и поддерживается J-Link, но этот
файл содержит одновременно внутренние секции STM32 и XIP-секции модулей по
адресам `0x9000....`. Поэтому для записи только основной программы следует
использовать `internal_flash.hex`, а `.out` — для отладки IAR.

Область программы:

```text
Flash: 0x08000000..0x0801FFFF (128 KiB)
RAM:   0x24000000..0x2407CFFF (500 KiB для основной программы)
VTOR:  0x08000000
```

Верхние 12 КиБ AXI SRAM зарезервированы:

```text
0x2407D000..0x2407DFFF   образ модуля, передаваемый через J-Link
0x2407E000..0x2407EFFF   управление загрузкой через J-Link
0x2407F000..0x2407FFFF   API исполняемых модулей
```

Release-сборка после добавления SPI4 FlashDB создана 30.07.2026. По
map-файлу:

```text
readonly code:            108 276 байт
readonly data:             14 998 байт
read/write data:           79 020 байт
absolute RAM:               6 304 байта
```

Сумма readonly code/data — 123 274 байта; до границы 128 КиБ остаётся
примерно 7,6 КиБ. При расширении основной прошивки необходимо контролировать
итоговый размер map-файла.

## Внешняя Flash

Используются две независимые микросхемы по 16 МиБ.

```text
QSPI, FAL-устройство norflash0:
0x000000..0x013FFF   modules: 20 слотов по 4 КиБ
0x014000..0xFFFFFF   web

SPI4 U4, FAL-устройство dbflash:
0x000000..0x0FFFFF   holding: FlashDB KVDB, 1 МиБ
0x100000..0xFFFFFF   archive: FlashDB TSDB, 15 МиБ
```

CPU видит слоты модулей через XIP-окно
`0x90000000..0x90013FFF`. Каждый слот содержит 10-байтный заголовок,
за которым с адреса `слот + 0x0C` располагается точка входа и код.

Запись и проверка модулей описаны в `PROGRAM_EXTERNAL_FLASH.md`.

## Сеть

Статическая конфигурация платы:

```text
IP:      192.168.1.100
Mask:    255.255.255.0
Gateway: 192.168.1.1
DHCP:    выключен
```

Для прямого подключения компьютеру назначить:

```text
IP:      192.168.1.10
Mask:    255.255.255.0
Gateway: пусто
```

Проверка:

```text
ping 192.168.1.100
```

## Тактирование

- внешний кварц HSE: 12 МГц;
- PLL1: M=2, N=100, P=4;
- SYSCLK: 150 МГц;
- MCO2 на PC9: SYSCLK/6 = 25 МГц;
- PC9 подключён к входу XTAL1/CLKIN LAN8720;
- после запуска на PC9 измеряется 25 МГц.

## RMII и управление PHY

```text
PC1  -> ETH_MDC
PA1  -> ETH_REF_CLK, 50 МГц от LAN8720
PA2  -> ETH_MDIO
PA7  -> ETH_CRS_DV
PC4  -> ETH_RXD0
PC5  -> ETH_RXD1
PB11 -> ETH_TX_EN
PB12 -> ETH_TXD0
PB13 -> ETH_TXD1
PB10 -> LAN8720 nRST
PC9  -> LAN8720 XTAL1/CLKIN, 25 МГц MCO2
```

Проверенный PHY ID/BMSR:

```text
PHY ID1: 0x0007
PHY ID2: 0xC0F1
BMSR:    0x7809
```

## Критические исправления

1. Старый проект устанавливал `SCB->VTOR = QSPI_BASE` (`0x90000000`).
   При первом переключении потока RT-Thread это вызывало HardFault.
   В рабочем проекте VTOR установлен в `FLASH_BANK1_BASE`.
2. ROM перенесён с QSPI `0x90000000` во внутреннюю Flash `0x08000000`.
3. В IAR для Debug и Release выбран STM32H750VB и драйвер J-Link.
4. Для обеих конфигураций указан Flash Loader
   `FlashSTM32H750xB.board`.
5. Автоматический `Run to main` отключён; C-SPY может останавливаться
   непосредственно после reset.
6. PHY получает 25 МГц на PC9, а MCU получает RMII REF_CLK 50 МГц на PA1.

Не возвращать VTOR на `0x90000000`, даже если QSPI используется для
веб-страниц.

## Ограничения и дальнейшее развитие

- TCP включён и может использоваться веб-сервером и Modbus TCP;
- FlashDB подключена через FAL и размещена на отдельной U4 через SPI4;
- код и таблицу векторов оставить во внутренней Flash;
- не переносить VTOR в QSPI;
- при записи слота QSPI необходимо приостанавливать XIP-модуль этого слота;
- операции FlashDB на SPI4 не требуют остановки XIP-модулей;
- временные метки архива пока считаются от запуска контроллера; для
  абсолютного времени требуется подключить RTC;
- после изменений контролировать `Release/List/eth_official.map`.

## Основные файлы

```text
applications/main.c                         точка входа приложения
applications/modules/qspi_objects.c         каталог OBJ1 и WEB_FILE в QSPI
applications/elam_modbus.c                  ELAM Modbus и таблица TIT
applications/arx/                           Holding FlashDB и архив
applications/arx/spi4_w25q128_fal.c         FAL-драйвер U4/SPI4 для FlashDB
applications/modules/qspi_modules.c         загрузка и выполнение QSPI-модулей
applications/modules/module_modbus.c        загрузка и управление через Modbus
applications/modules/module_service_api.c   API UART7/USART2 и сервисы модулей
applications/fal_cfg.h                      карта разделов QSPI и SPI4 Flash
applications/thread_profiler.c              профилировщик потоков
drivers/board.c                             RMII GPIO, PHY reset, MPU, MCO2
drivers/drv_clk.c                           PLL и системное тактирование
drivers/drv_common.c                        VTOR, HAL и системная инициализация
drivers/drv_eth.c                           адаптер HAL ETH <-> RT-Thread/lwIP
drivers/board.h                             карта ROM/RAM и выбор LAN8720
rtconfig.h                                  конфигурация RT-Thread/lwIP и IP
linkscripts/STM32H750VBTx/link.icf          внутренняя Flash и AXI SRAM
libraries/STM32H7xx_HAL_Driver/Src/
  stm32h7xx_hal_eth.c                       официальный HAL ETH
```
