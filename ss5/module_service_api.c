#include <rtthread.h>
#include <rtdevice.h>
#include <board.h>
#include <string.h>
#include "module_service_api.h"
#include "qspi_objects.h"
#include "../elam_modbus.h"
#include "../arx/arx_example.h"
#include "../arx/holding_flashdb.h"
#include "../thread_profiler.h"
#include "../rs485_master.h"

#define MODULE_RTU7_NAME          "uart7"
#define MODULE_RTU2_NAME          "uart2"
#define MODULE_RTU7_MAX_REGS      125U
#define MODULE_RTU7_RX_MAX        (5U + MODULE_RTU7_MAX_REGS * 2U)
#define MODULE_ARCHIVE_COMMAND    2442U
#define MODULE_ARCHIVE_STATUS     2443U
#define MODULE_ARCHIVE_APPEND     1U

static rt_device_t module_rtu7_device;
static rt_device_t module_rtu2_device;
static struct rt_mutex module_rtu7_lock;
static struct rt_mutex module_rtu2_lock;
static struct rt_semaphore module_rtu7_rx_sem;
static struct rt_semaphore module_rtu2_rx_sem;
static struct rt_mutex module_archive_lock;

static void *module_service_memory_realloc(void *pointer, u32 size)
{
    return rt_realloc(pointer, size);
}

static void module_service_memory_free(void *pointer)
{
    rt_free(pointer);
}

static u32 module_service_tick_get(void)
{
    return (u32)rt_tick_get();
}

static u32 module_service_tick_per_second(void)
{
    return RT_TICK_PER_SECOND;
}

static void module_service_log_write(const char *text)
{
    if (text != RT_NULL)
        rt_kprintf("[lua] %s\n", text);
}

static s32 module_service_object_find_type(
    u16 object_type, u16 ordinal, module_object_info_t *info)
{
    qspi_object_record_t record;
    u16 index;
    u16 match = 0U;
    int result;

    if (info == RT_NULL || object_type == QSPI_OBJECT_NONE)
        return QSPI_OBJECT_ERR_PARAM;
    for (index = 0U; index < QSPI_DIRECTORY_ENTRY_CAPACITY; index++)
    {
        result = qspi_object_get(index, &record);
        if (result == QSPI_OBJECT_ERR_NOT_FOUND)
            return QSPI_OBJECT_ERR_NOT_FOUND;
        if (result != QSPI_OBJECT_OK)
            return result;
        if (record.header.object_type != object_type)
            continue;
        if (match++ != ordinal)
            continue;
        result = qspi_object_verify(&record);
        if (result != QSPI_OBJECT_OK)
            return result;
        memset(info, 0, sizeof(*info));
        info->object_id = record.header.object_id;
        info->generation = record.header.generation;
        info->payload_address =
            QSPI_BLOCK_XIP_ADDRESS(record.first_block) +
            QSPI_OBJECT_HEADER_SIZE;
        info->payload_size = record.header.payload_size;
        info->object_type = record.header.object_type;
        info->flags = record.header.flags;
        memcpy(info->name, record.header.name, sizeof(info->name));
        info->name[sizeof(info->name) - 1U] = '\0';
        return QSPI_OBJECT_OK;
    }
    return QSPI_OBJECT_ERR_NOT_FOUND;
}

/*
 * Вычисляет CRC16 Modbus для запроса или ответа RTU.
 */
static u16 module_service_crc16(const u8 *data, u16 length)
{
    u16 crc = 0xFFFFU;
    u8 bit;

    while (length--)
    {
        crc ^= *data++;
        for (bit = 0; bit < 8U; bit++)
            crc = (crc & 1U) ?
                  (u16)((crc >> 1) ^ 0xA001U) :
                  (u16)(crc >> 1);
    }
    return crc;
}
/*
 * Освобождает семафор при поступлении байтов на прикладной UART7.
 */
static rt_err_t module_rtu7_rx_indicate(rt_device_t device, rt_size_t size)
{
    (void)device;
    (void)size;
    return rt_sem_release(&module_rtu7_rx_sem);
}

/*
 * Releases the USART2 receive semaphore whenever RT-Thread receives bytes.
 */
static rt_err_t module_rtu2_rx_indicate(rt_device_t device, rt_size_t size)
{
    (void)device;
    (void)size;
    return rt_sem_release(&module_rtu2_rx_sem);
}

/*
 * Удаляет из приемного кольца UART7 байты предыдущего незавершенного кадра.
 */
static void module_rtu7_flush(void)
{
    u8 buffer[32];

    while (rt_device_read(module_rtu7_device, 0,
                          buffer, sizeof(buffer)) != 0U)
    {
    }
    while (rt_sem_take(&module_rtu7_rx_sem, 0) == RT_EOK)
    {
    }
}

/*
 * Removes bytes and stale notifications left by the previous USART2 frame.
 */
static void module_rtu2_flush(void)
{
    u8 buffer[32];

    while (rt_device_read(module_rtu2_device, 0,
                          buffer, sizeof(buffer)) != 0U)
    {
    }
    while (rt_sem_take(&module_rtu2_rx_sem, 0) == RT_EOK)
    {
    }
}

/*
 * Читает holding-регистры Modbus RTU функцией 03 через UART7.
 * UART7 постоянно настроен как 19200 8N1 без управления DE.
 */
static s32 module_rtu7_read_holding(u8 slave, u16 address, u16 count,
                                    u16 *values, u32 timeout_ms)
{
    (void)slave;
    (void)address;
    (void)count;
    (void)values;
    (void)timeout_ms;
    return -RT_ENOSYS;
#if 0
    u8 request[8];
    u8 response[MODULE_RTU7_RX_MAX];
    u16 request_crc;
    u16 response_crc;
    u16 expected;
    u16 used = 0U;
    u16 index;
    rt_tick_t start;
    rt_tick_t timeout_ticks;
    rt_size_t received;
    s32 result = -RT_ERROR;

    if (module_rtu7_device == RT_NULL || slave == 0U ||
        values == RT_NULL || count == 0U ||
        count > MODULE_RTU7_MAX_REGS)
        return -RT_EINVAL;

    expected = (u16)(5U + count * 2U);
    request[0] = slave;
    request[1] = 3U;
    request[2] = (u8)(address >> 8);
    request[3] = (u8)address;
    request[4] = (u8)(count >> 8);
    request[5] = (u8)count;
    request_crc = module_service_crc16(request, 6U);
    request[6] = (u8)request_crc;
    request[7] = (u8)(request_crc >> 8);

    timeout_ticks = (rt_tick_t)
        ((timeout_ms * RT_TICK_PER_SECOND + 999U) / 1000U);
    if (timeout_ticks == 0U)
        timeout_ticks = 1U;

    rt_mutex_take(&module_rtu7_lock, RT_WAITING_FOREVER);
    module_rtu7_flush();
    if (rt_device_write(module_rtu7_device, 0,
                        request, sizeof(request)) != sizeof(request))
        goto exit;

    start = rt_tick_get();
    while (used < expected && rt_tick_get() - start < timeout_ticks)
    {
        received = rt_device_read(module_rtu7_device, 0,
                                  response + used,
                                  sizeof(response) - used);
        used = (u16)(used + received);
        if (used >= expected)
            break;
        rt_sem_take(&module_rtu7_rx_sem, 1U);
    }

    if (used != expected || response[0] != slave ||
        response[1] != 3U || response[2] != count * 2U)
        goto exit;
    response_crc = module_service_crc16(response, (u16)(expected - 2U));
    if (response[expected - 2U] != (u8)response_crc ||
        response[expected - 1U] != (u8)(response_crc >> 8))
        goto exit;

    for (index = 0; index < count; index++)
        values[index] = (u16)(((u16)response[3U + index * 2U] << 8) |
                              response[4U + index * 2U]);
    result = RT_EOK;

exit:
    rt_mutex_release(&module_rtu7_lock);
    return result;
#endif
}

/*
 * Reads Modbus holding registers with function 03 through USART2/RS-485.
 * USART2 is fixed at 19200 8N1; its hardware DE output is PD4/AF7.
 */
static s32 module_rtu2_read_holding(u8 slave, u16 address, u16 count,
                                    u16 *values, u32 timeout_ms)
{
    return rs485_master_read_holding(1U, slave, address, count,
                                     values, timeout_ms);
#if 0
    u8 request[8];
    u8 response[MODULE_RTU7_RX_MAX];
    u16 request_crc;
    u16 response_crc;
    u16 expected;
    u16 used = 0U;
    u16 index;
    rt_tick_t start;
    rt_tick_t timeout_ticks;
    rt_size_t received;
    s32 result = -RT_ERROR;

    if (module_rtu2_device == RT_NULL || slave == 0U ||
        values == RT_NULL || count == 0U ||
        count > MODULE_RTU7_MAX_REGS)
        return -RT_EINVAL;

    expected = (u16)(5U + count * 2U);
    request[0] = slave;
    request[1] = 3U;
    request[2] = (u8)(address >> 8);
    request[3] = (u8)address;
    request[4] = (u8)(count >> 8);
    request[5] = (u8)count;
    request_crc = module_service_crc16(request, 6U);
    request[6] = (u8)request_crc;
    request[7] = (u8)(request_crc >> 8);

    timeout_ticks = (rt_tick_t)
        ((timeout_ms * RT_TICK_PER_SECOND + 999U) / 1000U);
    if (timeout_ticks == 0U)
        timeout_ticks = 1U;

    rt_mutex_take(&module_rtu2_lock, RT_WAITING_FOREVER);
    module_rtu2_flush();
    if (rt_device_write(module_rtu2_device, 0,
                        request, sizeof(request)) != sizeof(request))
        goto exit;

    start = rt_tick_get();
    while (used < expected && rt_tick_get() - start < timeout_ticks)
    {
        received = rt_device_read(module_rtu2_device, 0,
                                  response + used,
                                  sizeof(response) - used);
        used = (u16)(used + received);
        if (used >= expected)
            break;
        rt_sem_take(&module_rtu2_rx_sem, 1U);
    }

    if (used != expected || response[0] != slave ||
        response[1] != 3U || response[2] != count * 2U)
        goto exit;
    response_crc = module_service_crc16(response, (u16)(expected - 2U));
    if (response[expected - 2U] != (u8)response_crc ||
        response[expected - 1U] != (u8)(response_crc >> 8))
        goto exit;

    for (index = 0; index < count; index++)
        values[index] = (u16)(((u16)response[3U + index * 2U] << 8) |
                              response[4U + index * 2U]);
    result = RT_EOK;

exit:
    rt_mutex_release(&module_rtu2_lock);
    return result;
#endif
}

/*
 * Безопасно читает один внутренний регистр TIT.
 */
static s32 module_service_tit_read(u16 address, u16 *value)
{
    return elam_modbus_read_register(address, value);
}

/*
 * Безопасно записывает один внутренний регистр TIT.
 */
static s32 module_service_tit_write(u16 address, u16 value)
{
    return elam_modbus_write_register(address, value);
}

/*
 * Reads persistent configuration registers from the internal Holding KVDB.
 * Modules use this during entry initialization, not in their polling loop.
 */
static s32 module_service_holding_read(u16 address, u16 *values, u16 count)
{
    return holding_flashdb_read(address, values, count);
}

static s32 module_service_profiler_start(void)
{
    return (s32)thread_profiler_init();
}

static s32 module_service_profiler_stop(void)
{
    return (s32)thread_profiler_stop();
}

static s32 module_service_profiler_snapshot_begin(u8 *count,
                                                   unsigned long long *total_cycles,
                                                   u32 *cycles_per_us)
{
    return (s32)thread_profiler_snapshot_begin(count, total_cycles,
                                               cycles_per_us);
}

static s32 module_service_profiler_snapshot_item(u8 index, void *item)
{
    return (s32)thread_profiler_snapshot_item(
        index, (struct thread_cpu_stat *)item);
}

static s32 module_service_profiler_publish(u8 index, const void *item,
                                           u16 load_x100,
                                           u32 window_total_us)
{
    return (s32)thread_profiler_publish(
        index, (const struct thread_cpu_stat *)item,
        load_x100, window_total_us);
}

/*
 * Возвращает время в секундах от запуска контроллера.
 * После подключения RTC эту функцию нужно заменить Unix-временем.
 */
static u32 module_service_timestamp_get(void)
{
    return (u32)(rt_tick_get() / RT_TICK_PER_SECOND);
}

/*
 * Приостанавливает текущий поток модуля на заданное число миллисекунд.
 */
static void module_service_delay_ms(u32 milliseconds)
{
    rt_thread_mdelay((rt_int32_t)milliseconds);
}

/*
 * Заполняет структуру демонстрационного архива и выдает внутреннюю
 * команду TIT[2442]=1 только при свободном архивном потоке.
 */
static s32 module_service_archive_tii_append(u32 timestamp,
                                             const u16 *values,
                                             u16 count)
{
    u16 record_values[48];
    u16 status;
    u16 command;
    u16 index;
    s32 result = -RT_EBUSY;

    if (values == RT_NULL || count > 48U)
        return -RT_EINVAL;

    rt_mutex_take(&module_archive_lock, RT_WAITING_FOREVER);
    if (elam_modbus_read_register(MODULE_ARCHIVE_STATUS,
                                  &status) != RT_EOK ||
        elam_modbus_read_register(MODULE_ARCHIVE_COMMAND,
                                  &command) != RT_EOK ||
        status != 0U || command != 0U)
        goto exit;

    memset(record_values, 0, sizeof(record_values));
    for (index = 0; index < count; index++)
        record_values[index] = values[index];
    arx_example_fill(timestamp, record_values);
    result = elam_modbus_write_register(MODULE_ARCHIVE_COMMAND,
                                        MODULE_ARCHIVE_APPEND);

exit:
    rt_mutex_release(&module_archive_lock);
    return result;
}

/*
 * Configures PD4 as the native USART2_DE signal and enables automatic
 * active-high driver control. This reproduces the USART2 scheme from picoC/16.
 */
static void module_rtu2_hardware_de_init(void)
{
    GPIO_InitTypeDef gpio = {0};
    u32 usart_was_enabled;

    __HAL_RCC_GPIOD_CLK_ENABLE();
    gpio.Pin = GPIO_PIN_4;
    gpio.Mode = GPIO_MODE_AF_PP;
    gpio.Pull = GPIO_NOPULL;
    gpio.Speed = GPIO_SPEED_FREQ_LOW;
    gpio.Alternate = GPIO_AF7_USART2;
    HAL_GPIO_Init(GPIOD, &gpio);

    /*
     * DEM/DEP and DE timing fields are protected while USART is enabled.
     * HAL_UART_Init() has already set UE, therefore configure RS-485 mode
     * with UE temporarily cleared and then restore the peripheral.
     */
    usart_was_enabled = READ_BIT(USART2->CR1, USART_CR1_UE);
    CLEAR_BIT(USART2->CR1, USART_CR1_UE);
    CLEAR_BIT(USART2->CR1, USART_CR1_DEAT_Msk | USART_CR1_DEDT_Msk);
    CLEAR_BIT(USART2->CR3, USART_CR3_DEP);
    SET_BIT(USART2->CR3, USART_CR3_DEM);
    if (usart_was_enabled != 0U)
        SET_BIT(USART2->CR1, USART_CR1_UE);
}

/*
 * Настраивает UART7 как прикладной Modbus RTU master 19200 8N1 и
 * публикует стабильную таблицу функций для загружаемых модулей.
 */
int module_service_init(void)
{
    module_service_api_t api;

    if (rt_mutex_init(&module_archive_lock, "mod_arx",
                      RT_IPC_FLAG_PRIO) != RT_EOK)
        return -RT_ERROR;
    if (rs485_master_init() != RT_EOK)
        return -RT_ERROR;

    memset(&api, 0, sizeof(api));
    api.magic = MODULE_SERVICE_API_MAGIC;
    api.version = MODULE_SERVICE_API_VERSION;
    api.size = sizeof(api);
    api.rtu7_read_holding = module_rtu7_read_holding;
    api.tit_read = module_service_tit_read;
    api.tit_write = module_service_tit_write;
    api.timestamp_get = module_service_timestamp_get;
    api.delay_ms = module_service_delay_ms;
    api.archive_tii_append = module_service_archive_tii_append;
    api.rtu2_read_holding = module_rtu2_read_holding;
    api.holding_read = module_service_holding_read;
    api.profiler_start = module_service_profiler_start;
    api.profiler_stop = module_service_profiler_stop;
    api.profiler_snapshot_begin = module_service_profiler_snapshot_begin;
    api.profiler_snapshot_item = module_service_profiler_snapshot_item;
    api.profiler_publish = module_service_profiler_publish;
    api.tag_read = tag_read;
    api.tag_write = tag_write;
    api.tag_get_float = tag_get_float;
    api.tag_set_float = tag_set_float;
    api.tag_set_valid = tag_set_valid;
    api.tag_get_info = tag_get_info;
    api.tag_find = tag_find;
    api.memory_realloc = module_service_memory_realloc;
    api.memory_free = module_service_memory_free;
    api.tick_get = module_service_tick_get;
    api.tick_per_second = module_service_tick_per_second;
    api.log_write = module_service_log_write;
    api.object_find_type = module_service_object_find_type;
    api.lua_status_reset = module_lua_status_reset;
    api.lua_status_update = module_lua_status_update;

    memcpy((void *)MODULE_SERVICE_API_ADDRESS, &api, sizeof(api));
    SCB_CleanDCache_by_Addr((u32 *)MODULE_SERVICE_API_ADDRESS,
                            sizeof(api));
    __DSB();
    return RT_EOK;
}
typedef struct
{
    volatile u8 state;
    u8 reserved[3];
    volatile s32 result;
} module_lua_slot_status_t;

static module_lua_slot_status_t module_lua_slots[MODULE_LUA_SLOT_COUNT];

void module_lua_status_reset(void)
{
    memset(module_lua_slots, 0, sizeof(module_lua_slots));
}

void module_lua_status_update(u8 slot, u8 state, s32 result)
{
    if (slot == 0U || slot > MODULE_LUA_SLOT_COUNT ||
        state > MODULE_LUA_SLOT_ERROR)
        return;
    module_lua_slots[slot - 1U].result = result;
    module_lua_slots[slot - 1U].state = state;
}

s32 module_lua_status_read(u16 offset, u16 *values, u16 count)
{
    u32 active_mask = 0U;
    u16 active_count = 0U;
    u16 index;

    if (values == 0 || count == 0U ||
        (u32)offset + count > MODULE_LUA_STATUS_MB_WORDS)
        return -1;
    for (index = 0U; index < MODULE_LUA_SLOT_COUNT; index++)
    {
        if (module_lua_slots[index].state == MODULE_LUA_SLOT_RUNNING)
        {
            active_mask |= 1UL << index;
            active_count++;
        }
    }
    for (index = 0U; index < count; index++)
    {
        u16 word = (u16)(offset + index);
        if (word == 0U) values[index] = 1U;
        else if (word == 1U) values[index] = active_count;
        else if (word == 2U) values[index] = (u16)(active_mask >> 16);
        else if (word == 3U) values[index] = (u16)active_mask;
        else
        {
            u16 relative = (u16)(word - 4U);
            u16 slot = (u16)(relative / 3U);
            u16 field = (u16)(relative % 3U);
            s32 result = module_lua_slots[slot].result;
            if (field == 0U) values[index] = module_lua_slots[slot].state;
            else if (field == 1U) values[index] = (u16)((u32)result >> 16);
            else values[index] = (u16)result;
        }
    }
    return 0;
}
