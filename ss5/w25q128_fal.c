#include <board.h>
#include <fal.h>
#include <string.h>
#include "stm32h7xx_ll_bus.h"
#include "stm32h7xx_ll_gpio.h"
#include "w25q128_fal.h"
#include "../modules/qspi_modules.h"

#define W25Q_XIP_BASE       0x90000000UL

volatile int w25q_last_error_stage;
#define W25Q_SIZE           (16U * 1024U * 1024U)
#define W25Q_SECTOR_SIZE    4096U
#define W25Q_PAGE_SIZE      256U
#define W25Q_TIMEOUT_MS     5000U

#define CMD_PAGE_PROGRAM    0x02U
#define CMD_READ_DATA       0x03U
#define CMD_READ_JEDEC_ID   0x9FU
#define CMD_READ_STATUS     0x05U
#define CMD_WRITE_ENABLE    0x06U
#define CMD_SECTOR_ERASE    0x20U

/*
 * JEDEC ID, прочитанный при инициализации Flash.
 * Для W25Q128 ожидается значение 0xEF4018.
 */
volatile rt_uint32_t w25q_jedec_id;
volatile rt_uint8_t w25q_status_test;
volatile rt_uint32_t w25q_gpio_idr_test[3];

static QSPI_HandleTypeDef w25q_qspi;
static struct rt_mutex w25q_lock;
static rt_bool_t w25q_lock_ready;
static rt_bool_t w25q_memory_mapped;

/*
 * Заполняет общие поля команды HAL QSPI для W25Q128 на Bank 2.
 */
static void w25q_command_init(QSPI_CommandTypeDef *command)
{
    memset(command, 0, sizeof(*command));
    command->InstructionMode = QSPI_INSTRUCTION_1_LINE;
    command->AddressMode = QSPI_ADDRESS_NONE;
    command->AddressSize = QSPI_ADDRESS_24_BITS;
    command->AlternateByteMode = QSPI_ALTERNATE_BYTES_NONE;
    command->DataMode = QSPI_DATA_NONE;
    command->DummyCycles = 0U;
    command->DdrMode = QSPI_DDR_MODE_DISABLE;
    command->DdrHoldHalfCycle = QSPI_DDR_HHC_ANALOG_DELAY;
    command->SIOOMode = QSPI_SIOO_INST_EVERY_CMD;
}

/*
 * Настраивает PB2, PC11, PE7 и PE8 на аппаратный QUADSPI Bank 2.
 * Неиспользуемые в режиме 1-1-1 сигналы /WP и /HOLD удерживает в единице.
 */
static void w25q_gpio_init(void)
{
    LL_AHB4_GRP1_EnableClock(LL_AHB4_GRP1_PERIPH_GPIOB |
                             LL_AHB4_GRP1_PERIPH_GPIOC |
                             LL_AHB4_GRP1_PERIPH_GPIOE);

    LL_GPIO_SetPinMode(GPIOB, LL_GPIO_PIN_2, LL_GPIO_MODE_ALTERNATE);
    LL_GPIO_SetPinOutputType(GPIOB, LL_GPIO_PIN_2, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOB, LL_GPIO_PIN_2, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOB, LL_GPIO_PIN_2, LL_GPIO_PULL_NO);
    LL_GPIO_SetAFPin_0_7(GPIOB, LL_GPIO_PIN_2, LL_GPIO_AF_9);

    LL_GPIO_SetPinMode(GPIOC, LL_GPIO_PIN_11, LL_GPIO_MODE_ALTERNATE);
    LL_GPIO_SetPinOutputType(GPIOC, LL_GPIO_PIN_11, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOC, LL_GPIO_PIN_11, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOC, LL_GPIO_PIN_11, LL_GPIO_PULL_NO);
    LL_GPIO_SetAFPin_8_15(GPIOC, LL_GPIO_PIN_11, LL_GPIO_AF_9);

    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_7, LL_GPIO_MODE_ALTERNATE);
    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_7, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_7, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_7, LL_GPIO_PULL_NO);
    LL_GPIO_SetAFPin_0_7(GPIOE, LL_GPIO_PIN_7, LL_GPIO_AF_10);

    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_8, LL_GPIO_MODE_ALTERNATE);
    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_8, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_8, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_8, LL_GPIO_PULL_NO);
    LL_GPIO_SetAFPin_8_15(GPIOE, LL_GPIO_PIN_8, LL_GPIO_AF_10);

    LL_GPIO_SetOutputPin(GPIOE, LL_GPIO_PIN_9 | LL_GPIO_PIN_10);
    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_9, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_9, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_9, LL_GPIO_PULL_NO);
    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_9, LL_GPIO_MODE_OUTPUT);
    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_10, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_10, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_10, LL_GPIO_PULL_NO);
    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_10, LL_GPIO_MODE_OUTPUT);
}

/*
 * Выводит QSPI из memory-mapped режима перед программированием или стиранием.
 * Вызывать только при захваченном w25q_lock и остановленных XIP-модулях.
 */
static int w25q_leave_memory_mapped(void)
{
    rt_uint32_t start;
    rt_uint32_t timeout_cycles;

    if (!w25q_memory_mapped)
        return 0;

    /*
     * module_xip_pause_all() has locked the scheduler because no task may
     * execute from 0x90000000 while memory-mapped mode is disabled.
     * HAL_QSPI_Abort() uses HAL_GetTick() for its timeout; on this RT-Thread
     * port that tick cannot advance while the scheduler is locked, turning a
     * QSPI timeout into a permanent controller hang.  Use DWT CYCCNT here.
     */
    CoreDebug->DEMCR |= CoreDebug_DEMCR_TRCENA_Msk;
    DWT->CTRL |= DWT_CTRL_CYCCNTENA_Msk;
    start = DWT->CYCCNT;
    timeout_cycles = SystemCoreClock / 10U; /* 100 ms. */

    CLEAR_BIT(w25q_qspi.Instance->CR, QUADSPI_CR_DMAEN);
    SET_BIT(w25q_qspi.Instance->CR, QUADSPI_CR_ABORT);
    while (__HAL_QSPI_GET_FLAG(&w25q_qspi, QSPI_FLAG_TC) == RESET)
    {
        if ((rt_uint32_t)(DWT->CYCCNT - start) >= timeout_cycles)
            goto force_reset;
    }
    __HAL_QSPI_CLEAR_FLAG(&w25q_qspi, QSPI_FLAG_TC);
    while (__HAL_QSPI_GET_FLAG(&w25q_qspi, QSPI_FLAG_BUSY) != RESET)
    {
        if ((rt_uint32_t)(DWT->CYCCNT - start) >= timeout_cycles)
            goto force_reset;
    }
    w25q_qspi.State = HAL_QSPI_STATE_READY;
    w25q_memory_mapped = RT_FALSE;
    return 0;

force_reset:
    /*
     * Some memory-mapped reads keep BUSY asserted and the hardware never
     * raises TC for ABORT.  A peripheral-only reset is safe here: all XIP
     * threads are paused and the external flash itself is not reset.
     */
    __HAL_RCC_QSPI_FORCE_RESET();
    __DSB();
    __HAL_RCC_QSPI_RELEASE_RESET();
    w25q_qspi.State = HAL_QSPI_STATE_RESET;
    w25q_qspi.Lock = HAL_UNLOCKED;
    if (HAL_QSPI_Init(&w25q_qspi) != HAL_OK)
        return -1;
    w25q_memory_mapped = RT_FALSE;
    return 0;
}

/*
 * Включает непрерывное чтение W25Q128 по адресу CPU 0x90000000.
 * Используется обычная команда 0x03 в режиме 1-1-1 без настройки QE.
 */
static int w25q_enter_memory_mapped(void)
{
    QSPI_CommandTypeDef command;
    QSPI_MemoryMappedTypeDef config;

    w25q_command_init(&command);
    command.Instruction = CMD_READ_DATA;
    command.AddressMode = QSPI_ADDRESS_1_LINE;
    command.DataMode = QSPI_DATA_1_LINE;

    memset(&config, 0, sizeof(config));
    config.TimeOutActivation = QSPI_TIMEOUT_COUNTER_DISABLE;
    if (HAL_QSPI_MemoryMapped(&w25q_qspi, &command, &config) != HAL_OK)
        return -1;
    w25q_memory_mapped = RT_TRUE;
    return 0;
}

/*
 * Передаёт команду без адреса и данных, например Write Enable.
 */
static int w25q_simple_command(rt_uint8_t instruction)
{
    QSPI_CommandTypeDef command;

    w25q_command_init(&command);
    command.Instruction = instruction;
    return HAL_QSPI_Command(&w25q_qspi, &command, W25Q_TIMEOUT_MS) == HAL_OK
           ? 0 : -1;
}

/*
 * Читает регистр состояния W25Q128 в непрямом режиме.
 */
static int w25q_read_status_raw(rt_uint8_t *status)
{
    QSPI_CommandTypeDef command;

    w25q_command_init(&command);
    command.Instruction = CMD_READ_STATUS;
    command.DataMode = QSPI_DATA_1_LINE;
    command.NbData = 1U;
    if (HAL_QSPI_Command(&w25q_qspi, &command, W25Q_TIMEOUT_MS) != HAL_OK)
        return -1;
    return HAL_QSPI_Receive(&w25q_qspi, status, W25Q_TIMEOUT_MS) == HAL_OK
           ? 0 : -1;
}

/*
 * Ожидает завершения стирания или программирования с тайм-аутом 5 секунд.
 */
static int w25q_wait_ready_raw(void)
{
    rt_tick_t start = rt_tick_get();
    rt_uint8_t status;

    do
    {
        if (w25q_read_status_raw(&status) != 0)
            return -1;
        if ((status & 1U) == 0U)
            return 0;
        if (rt_tick_get() - start > RT_TICK_PER_SECOND * 5U)
            return -1;
        /*
         * Переключение задач запрещено, пока memory-mapped окно отключено.
         * Системный тик работает в прерывании, поэтому здесь нужен busy wait.
         */
    } while (1);
}

/*
 * Читает три байта JEDEC ID в непрямом режиме.
 */
static rt_uint32_t w25q_read_jedec_raw(void)
{
    QSPI_CommandTypeDef command;
    rt_uint8_t id[3];

    w25q_command_init(&command);
    command.Instruction = CMD_READ_JEDEC_ID;
    command.DataMode = QSPI_DATA_1_LINE;
    command.NbData = sizeof(id);
    if (HAL_QSPI_Command(&w25q_qspi, &command, W25Q_TIMEOUT_MS) != HAL_OK)
        return 0U;
    if (HAL_QSPI_Receive(&w25q_qspi, id, W25Q_TIMEOUT_MS) != HAL_OK)
        return 0U;
    return ((rt_uint32_t)id[0] << 16) |
           ((rt_uint32_t)id[1] << 8) | id[2];
}

/*
 * Программирует данные постранично. QSPI должен быть в непрямом режиме.
 */
static int w25q_write_raw(rt_uint32_t offset,
                          const rt_uint8_t *buffer, size_t size)
{
    size_t done = 0U;

    if (offset + size > W25Q_SIZE)
        return -1;
    while (done < size)
    {
        QSPI_CommandTypeDef command;
        size_t page_left = W25Q_PAGE_SIZE -
                           ((offset + done) % W25Q_PAGE_SIZE);
        size_t chunk = size - done < page_left ? size - done : page_left;

        if (w25q_wait_ready_raw() != 0 ||
            w25q_simple_command(CMD_WRITE_ENABLE) != 0)
            return -1;
        w25q_command_init(&command);
        command.Instruction = CMD_PAGE_PROGRAM;
        command.AddressMode = QSPI_ADDRESS_1_LINE;
        command.Address = offset + done;
        command.DataMode = QSPI_DATA_1_LINE;
        command.NbData = chunk;
        if (HAL_QSPI_Command(&w25q_qspi, &command, W25Q_TIMEOUT_MS) != HAL_OK ||
            HAL_QSPI_Transmit(&w25q_qspi,
                              (rt_uint8_t *)(buffer + done),
                              W25Q_TIMEOUT_MS) != HAL_OK)
            return -1;
        done += chunk;
    }
    return w25q_wait_ready_raw() == 0 ? (int)size : -1;
}

/*
 * Стирает все 4-КБ сектора, пересекающиеся с заданным диапазоном.
 * QSPI должен быть в непрямом режиме.
 */
static int w25q_erase_raw(rt_uint32_t offset, size_t size)
{
    rt_uint32_t address;
    rt_uint32_t end;

    if (offset + size > W25Q_SIZE)
        return -1;
    address = offset & ~(W25Q_SECTOR_SIZE - 1U);
    end = (offset + size + W25Q_SECTOR_SIZE - 1U) &
          ~(W25Q_SECTOR_SIZE - 1U);
    while (address < end)
    {
        QSPI_CommandTypeDef command;

        if (w25q_wait_ready_raw() != 0 ||
            w25q_simple_command(CMD_WRITE_ENABLE) != 0)
            return -1;
        w25q_command_init(&command);
        command.Instruction = CMD_SECTOR_ERASE;
        command.AddressMode = QSPI_ADDRESS_1_LINE;
        command.Address = address;
        if (HAL_QSPI_Command(&w25q_qspi, &command,
                             W25Q_TIMEOUT_MS) != HAL_OK)
            return -1;
        address += W25Q_SECTOR_SIZE;
    }
    return w25q_wait_ready_raw() == 0 ? (int)size : -1;
}

/*
 * Инвалидирует строки кэша внешней Flash после её изменения.
 */
static void w25q_invalidate_cache(rt_uint32_t offset, size_t size)
{
    rt_uint32_t start = (W25Q_XIP_BASE + offset) & ~31UL;
    rt_uint32_t end = (W25Q_XIP_BASE + offset + size + 31U) & ~31UL;

    SCB_InvalidateDCache_by_Addr((rt_uint32_t *)start, (int32_t)(end - start));
    SCB_InvalidateICache();
    __DSB();
    __ISB();
}

/*
 * Начинает изменение Flash: блокирует доступ FlashDB, останавливает
 * все XIP-модули и выключает memory-mapped режим.
 */
static int w25q_modify_begin(void)
{
    if (module_xip_pause_all() != MODULE_OK)
        return -1;
    if (w25q_leave_memory_mapped() != 0)
    {
        module_xip_resume_all();
        return -1;
    }
    return 0;
}

/*
 * Завершает изменение Flash: восстанавливает memory-mapped режим,
 * очищает кэш изменённого диапазона и возобновляет XIP-модули.
 */
static int w25q_modify_end(rt_uint32_t offset, size_t size, int result)
{
    int map_result = w25q_enter_memory_mapped();

    if (map_result != 0)
    {
        /*
         * Продолжать планирование нельзя: сохранённые контексты модулей
         * могут указывать в недоступное окно 0x90000000.
         */
        NVIC_SystemReset();
#pragma diag_suppress=Pe111
        return -1;
#pragma diag_default=Pe111
    }
    w25q_invalidate_cache(offset, size);
    module_xip_resume_all();
    return result;
}

/*
 * Инициализирует аппаратный QUADSPI, проверяет JEDEC ID и включает XIP.
 */
static int w25q_init(void)
{
    rt_uint8_t status = 0xFFU;

    if (!w25q_lock_ready)
    {
        if (rt_mutex_init(&w25q_lock, "w25q", RT_IPC_FLAG_PRIO) != RT_EOK)
            return -1;
        w25q_lock_ready = RT_TRUE;
    }

    w25q_gpio_init();
    __HAL_RCC_QSPI_CLK_ENABLE();
    __HAL_RCC_QSPI_FORCE_RESET();
    __HAL_RCC_QSPI_RELEASE_RESET();

    memset(&w25q_qspi, 0, sizeof(w25q_qspi));
    w25q_qspi.Instance = QUADSPI;
    w25q_qspi.Init.ClockPrescaler = 7U;
    w25q_qspi.Init.FifoThreshold = 4U;
    w25q_qspi.Init.SampleShifting = QSPI_SAMPLE_SHIFTING_HALFCYCLE;
    w25q_qspi.Init.FlashSize = 23U;
    w25q_qspi.Init.ChipSelectHighTime = QSPI_CS_HIGH_TIME_2_CYCLE;
    w25q_qspi.Init.ClockMode = QSPI_CLOCK_MODE_0;
    w25q_qspi.Init.FlashID = QSPI_FLASH_ID_2;
    w25q_qspi.Init.DualFlash = QSPI_DUALFLASH_DISABLE;
    if (HAL_QSPI_Init(&w25q_qspi) != HAL_OK)
        return -1;

    rt_thread_mdelay(10U);
    w25q_gpio_idr_test[0] = GPIOB->IDR;
    w25q_gpio_idr_test[1] = GPIOC->IDR;
    w25q_gpio_idr_test[2] = GPIOE->IDR;
    w25q_jedec_id = w25q_read_jedec_raw();
    if (w25q_read_status_raw(&status) == 0)
        w25q_status_test = status;
    __NOP(); /* Точка останова: w25q_jedec_id должен быть 0xEF4018. */

    if (w25q_jedec_id != 0xEF4018UL || w25q_wait_ready_raw() != 0)
        return -1;
    return w25q_enter_memory_mapped();
}

/*
 * Читает Flash через memory-mapped окно 0x90000000 под общей блокировкой.
 */
static int w25q_read(long offset, rt_uint8_t *buffer, size_t size)
{
    if (!w25q_lock_ready || offset < 0 ||
        (rt_uint32_t)offset + size > W25Q_SIZE || buffer == RT_NULL)
        return -1;

    rt_mutex_take(&w25q_lock, RT_WAITING_FOREVER);
    if (!w25q_memory_mapped)
    {
        rt_mutex_release(&w25q_lock);
        return -1;
    }
    memcpy(buffer, (const void *)(W25Q_XIP_BASE + (rt_uint32_t)offset), size);
    rt_mutex_release(&w25q_lock);
    return (int)size;
}

/*
 * Записывает Flash для FAL. На всё время записи FlashDB заблокирована,
 * а все задачи, исполняемые из QSPI, приостановлены.
 */
static int w25q_write(long offset, const rt_uint8_t *buffer, size_t size)
{
    int result;

    w25q_last_error_stage = 0;
    if (!w25q_lock_ready || offset < 0 || buffer == RT_NULL ||
        (rt_uint32_t)offset + size > W25Q_SIZE)
        return -1;
    rt_mutex_take(&w25q_lock, RT_WAITING_FOREVER);
    if (w25q_modify_begin() != 0)
    {
        w25q_last_error_stage = 1;
        result = -1;
    }
    else
    {
        result = w25q_write_raw((rt_uint32_t)offset, buffer, size);
        if (result < 0)
            w25q_last_error_stage = 2;
        result = w25q_modify_end((rt_uint32_t)offset, size, result);
        if (result < 0 && w25q_last_error_stage == 0)
            w25q_last_error_stage = 3;
    }
    rt_mutex_release(&w25q_lock);
    return result;
}

/*
 * Стирает Flash для FAL. На всё время стирания FlashDB заблокирована,
 * а все задачи, исполняемые из QSPI, приостановлены.
 */
static int w25q_erase(long offset, size_t size)
{
    int result;

    w25q_last_error_stage = 0;
    if (!w25q_lock_ready || offset < 0 ||
        (rt_uint32_t)offset + size > W25Q_SIZE)
        return -1;
    rt_mutex_take(&w25q_lock, RT_WAITING_FOREVER);
    if (w25q_modify_begin() != 0)
    {
        w25q_last_error_stage = 11;
        result = -1;
    }
    else
    {
        result = w25q_erase_raw((rt_uint32_t)offset, size);
        if (result < 0)
            w25q_last_error_stage = 12;
        result = w25q_modify_end((rt_uint32_t)offset, size, result);
        if (result < 0 && w25q_last_error_stage == 0)
            w25q_last_error_stage = 13;
    }
    rt_mutex_release(&w25q_lock);
    return result;
}

/*
 * Предоставляет загрузчику модулей синхронизированное чтение W25Q128.
 */
int w25q128_read(u32 offset, u8 *buffer, u32 size)
{
    return w25q_read((long)offset, buffer, (size_t)size);
}

/*
 * Предоставляет загрузчику модулей синхронизированную запись W25Q128.
 */
int w25q128_write(u32 offset, const u8 *buffer, u32 size)
{
    return w25q_write((long)offset, buffer, (size_t)size);
}

/*
 * Предоставляет загрузчику модулей синхронизированное стирание W25Q128.
 */
int w25q128_erase(u32 offset, u32 size)
{
    return w25q_erase((long)offset, (size_t)size);
}

/*
 * Атомарно заменяет один модульный сектор: FlashDB не получает доступ
 * между стиранием, записью заголовка и записью тела модуля.
 */
int w25q128_replace_sector(u32 offset,
                           const u8 *header, u32 header_size,
                           const u8 *body, u32 body_size)
{
    int result;

    if (!w25q_lock_ready || header == RT_NULL ||
        offset % W25Q_SECTOR_SIZE != 0U ||
        header_size + body_size > W25Q_SECTOR_SIZE ||
        offset + W25Q_SECTOR_SIZE > W25Q_SIZE)
        return -1;

    rt_mutex_take(&w25q_lock, RT_WAITING_FOREVER);
    if (w25q_modify_begin() != 0)
        result = -1;
    else
    {
        result = w25q_erase_raw(offset, W25Q_SECTOR_SIZE);
        if (result == (int)W25Q_SECTOR_SIZE)
            result = w25q_write_raw(offset, header, header_size);
        if (result == (int)header_size && body_size != 0U)
            result = w25q_write_raw(offset + header_size, body, body_size);
        result = w25q_modify_end(offset, W25Q_SECTOR_SIZE, result);
    }
    rt_mutex_release(&w25q_lock);
    return result < 0 ? -1 : (int)(header_size + body_size);
}

struct fal_flash_dev nor_flash0 =
{
    .name = "norflash0",
    .addr = 0,
    .len = W25Q_SIZE,
    .blk_size = W25Q_SECTOR_SIZE,
    .ops = {w25q_init, w25q_read, w25q_write, w25q_erase},
    .write_gran = 1
};
