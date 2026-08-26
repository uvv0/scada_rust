#include <rtthread.h>
#include <board.h>
#include <string.h>
#include "qspi_modules.h"
#include "qspi_objects.h"
#include "module_service_api.h"
#include "../arx/w25q128_fal.h"
#include "../thread_profiler.h"

#define MODULE_THREAD_STACK_SIZE  2048U
#define MODULE_THREAD_PRIORITY    18U
#define MODULE_THREAD_TICK        10U
#define LUA_THREAD_STACK_SIZE     24576U
#define LUA_THREAD_PRIORITY       25U
#define PROFILER_MODULE_SLOT      2U
#define PROFILER_OBJECT_ID        2UL

#if defined(__ICCARM__)
#define MODULE_ROOT __root
#else
#define MODULE_ROOT
#endif

typedef struct
{
    rt_thread_t thread;
    qspi_module_status_t public_status;
} module_runtime_t;

static struct rt_mutex module_lock;
static rt_bool_t module_lock_ready;
static rt_bool_t module_version_check;
static u16 module_min_type;
static u16 module_min_num;
static module_runtime_t module_runtime[QSPI_MODULE_SLOT_COUNT];
static rt_bool_t module_xip_scheduler_locked;

typedef struct
{
    rt_thread_t thread;
    qspi_object_module_status_t status;
} object_module_runtime_t;

static object_module_runtime_t
    object_module_runtime[QSPI_MODULE_SLOT_COUNT];

/*
 * Обновляет CRC16 Modbus одним байтом, полином 0xA001, начальное значение
 * перед первым байтом должно быть 0xFFFF.
 */
static u16 module_crc16_byte(u16 crc, u8 value)
{
    u8 bit;

    crc ^= value;
    for (bit = 0; bit < 8U; bit++)
    {
        if (crc & 1U)
            crc = (u16)((crc >> 1) ^ 0xA001U);
        else
            crc >>= 1;
    }
    return crc;
}

/*
 * Вычисляет CRC16 Modbus непрерывного блока данных.
 */
static u16 module_crc16(const u8 *data, u32 length)
{
    u16 crc = 0xFFFFU;

    while (length--)
        crc = module_crc16_byte(crc, *data++);
    return crc;
}

/*
 * Возвращает фактическую длину тела из заголовка.
 * При size=0 телом считается весь остаток 4-КБ слота.
 */
static u32 module_body_size(const qspi_module_hdr_t *header)
{
    if (header->size != 0U)
        return header->size;
    return QSPI_MODULE_SLOT_SIZE - header->addr;
}

/*
 * Возвращает адрес байта сразу после защищенной CRC области модуля.
 */
static u32 module_image_end(const qspi_module_hdr_t *header)
{
    return header->addr + module_body_size(header);
}

/*
 * Проверяет номер слота и геометрию тела внутри одного 4-КБ сектора.
 */
static int module_check_header(u8 slot, const qspi_module_hdr_t *header)
{
    u32 body_size;

    if (slot >= QSPI_MODULE_SLOT_COUNT)
        return MODULE_ERR_SLOT;
    if (header->addr == QSPI_MODULE_EMPTY_ADDR)
        return MODULE_EMPTY;
    if (header->addr < sizeof(qspi_module_hdr_t) ||
        header->addr >= QSPI_MODULE_SLOT_SIZE)
        return MODULE_ERR_HEADER;

    body_size = module_body_size(header);
    if (body_size == 0U ||
        body_size > QSPI_MODULE_SLOT_SIZE - header->addr)
        return MODULE_ERR_SIZE;

    if (module_version_check &&
        (header->type != module_min_type || header->num < module_min_num))
        return MODULE_ERR_VERSION;
    return MODULE_OK;
}

/*
 * Сохраняет результат последней операции в диагностической таблице.
 */
static int module_store_result(u8 slot, int result)
{
    if (slot < QSPI_MODULE_SLOT_COUNT)
        module_runtime[slot].public_status.last_result = result;
    return result;
}

/*
 * Обертка потока вызывает точку входа загруженного модуля.
 * Младший бит адреса устанавливается для режима Thumb.
 */
static void module_thread_entry(void *parameter)
{
    u8 slot = (u8)(rt_ubase_t)parameter;
    u32 entry_address;
    qspi_module_entry_t entry;

    entry_address = module_xip_slot_address(slot) +
                    module_runtime[slot].public_status.header.addr;
    entry = (qspi_module_entry_t)(entry_address | 1U);
    entry();

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    module_runtime[slot].public_status.active = 0U;
    module_runtime[slot].thread = RT_NULL;
    rt_mutex_release(&module_lock);
}

static void object_module_thread_entry(void *parameter)
{
    object_module_runtime_t *runtime =
        (object_module_runtime_t *)parameter;
    qspi_module_entry_t entry;
    u32 entry_address;

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    entry_address = runtime->status.entry_address;
    rt_mutex_release(&module_lock);
    entry = (qspi_module_entry_t)(entry_address | 1U);
    entry();

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    runtime->status.active = 0U;
    runtime->thread = RT_NULL;
    rt_mutex_release(&module_lock);
}

/*
 * Инициализирует mutex и таблицу состояния загрузчика.
 * Содержимое модульных слотов при этом не изменяется.
 */
MODULE_ROOT int module_manager_init(void)
{
    u8 slot;

    if (module_lock_ready)
        return MODULE_OK;
    if (rt_mutex_init(&module_lock, "modules", RT_IPC_FLAG_PRIO) != RT_EOK)
        return MODULE_ERR_BUSY;

    memset(module_runtime, 0, sizeof(module_runtime));
    memset(object_module_runtime, 0, sizeof(object_module_runtime));
    for (slot = 0; slot < QSPI_MODULE_SLOT_COUNT; slot++)
    {
        module_runtime[slot].public_status.flash_address =
            module_flash_slot_address(slot);
        module_runtime[slot].public_status.xip_address =
            module_xip_slot_address(slot);
    }
    module_lock_ready = RT_TRUE;
    return MODULE_OK;
}

/*
 * Возвращает физическое смещение начала слота во W25Q128.
 */
MODULE_ROOT u32 module_flash_slot_address(u8 slot)
{
    return QSPI_MODULE_FLASH_BASE + (u32)slot * QSPI_MODULE_SLOT_SIZE;
}

/*
 * Возвращает адрес CPU начала 4-КБ слота в окне QSPI/XIP.
 */
MODULE_ROOT u32 module_xip_slot_address(u8 slot)
{
    return QSPI_MODULE_XIP_BASE + (u32)slot * QSPI_MODULE_SLOT_SIZE;
}

/*
 * Читает десятибайтный заголовок модуля из W25Q128.
 */
MODULE_ROOT int module_read_header(u8 slot, qspi_module_hdr_t *header)
{
    if (slot >= QSPI_MODULE_SLOT_COUNT || header == RT_NULL)
        return MODULE_ERR_SLOT;
    if (w25q128_read(module_flash_slot_address(slot), (u8 *)header,
                     sizeof(*header)) != (int)sizeof(*header))
        return MODULE_ERR_FLASH;
    return header->addr == QSPI_MODULE_EMPTY_ADDR ? MODULE_EMPTY : MODULE_OK;
}

/*
 * Включает проверку типа и минимального номера прошивки.
 */
MODULE_ROOT void module_set_min_version(u16 type, u16 num)
{
    module_min_type = type;
    module_min_num = num;
    module_version_check = RT_TRUE;
}

/*
 * Проверяет границы, версию и общий CRC16 заголовка с телом во Flash.
 */
MODULE_ROOT int module_validate(u8 slot)
{
    qspi_module_hdr_t header;
    u8 buffer[128];
    u32 address;
    u32 remaining;
    u32 chunk;
    u16 crc = 0xFFFFU;
    u32 index;
    int result;

    result = module_read_header(slot, &header);
    if (result != MODULE_OK)
        return module_store_result(slot, result);

    result = module_check_header(slot, &header);
    if (result != MODULE_OK)
        return module_store_result(slot, result);

    /*
     * Поле crc занимает байты 0..1 и в расчет не входит.
     * CRC идет непрерывно от addr-поля (байт 2) до конца тела.
     */
    address = module_flash_slot_address(slot) + sizeof(header.crc);
    remaining = module_image_end(&header) - sizeof(header.crc);
    while (remaining)
    {
        chunk = remaining > sizeof(buffer) ? sizeof(buffer) : remaining;
        if (w25q128_read(address, buffer, chunk) != (int)chunk)
            return module_store_result(slot, MODULE_ERR_FLASH);
        for (index = 0; index < chunk; index++)
            crc = module_crc16_byte(crc, buffer[index]);
        address += chunk;
        remaining -= chunk;
    }

    if (crc != header.crc)
        return module_store_result(slot, MODULE_ERR_CRC);

    module_runtime[slot].public_status.header = header;
    return module_store_result(slot, MODULE_OK);
}

/*
 * Проверяет готовый блок в RAM до записи: заголовок, границы, версию и CRC.
 * При size=0 отсутствующий хвост слота считается заполненным 0xFF.
 */
MODULE_ROOT int module_validate_buf(u8 slot, const u8 *input, u32 length,
                                    u16 *calculated_crc)
{
    qspi_module_hdr_t header;
    u32 image_end;
    u32 index;
    u16 crc;
    int result;

    if (slot >= QSPI_MODULE_SLOT_COUNT || input == RT_NULL)
        return MODULE_ERR_SLOT;
    if (length < sizeof(header) || length > QSPI_MODULE_SLOT_SIZE)
        return MODULE_ERR_SIZE;

    memcpy(&header, input, sizeof(header));
    result = module_check_header(slot, &header);
    if (result != MODULE_OK)
        return result;
    if (header.addr >= length)
        return MODULE_ERR_SIZE;

    image_end = module_image_end(&header);
    if (header.size != 0U && length != image_end)
        return MODULE_ERR_SIZE;

    /*
     * Один CRC закрывает оставшиеся поля заголовка, промежуток до addr
     * и тело. Само первое поле crc в расчет не включается.
     */
    crc = module_crc16(input + sizeof(header.crc),
                       length - sizeof(header.crc));
    if (header.size == 0U)
        for (index = length; index < image_end; index++)
            crc = module_crc16_byte(crc, 0xFFU);

    if (calculated_crc != RT_NULL)
        *calculated_crc = crc;
    if (header.crc != 0U && header.crc != crc)
        return MODULE_ERR_CRC;
    return MODULE_OK;
}

/*
 * Стирает слот и загружает готовый бинарный блок из RAM.
 * CRC в записываемом заголовке закрывает остальную шапку и тело.
 */
MODULE_ROOT int module_load_slot_buf(u8 slot, const u8 *input, u32 length)
{
    qspi_module_hdr_t header;
    u16 crc;
    int result;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (slot >= QSPI_MODULE_SLOT_COUNT || input == RT_NULL)
        return MODULE_ERR_SLOT;
    result = module_validate_buf(slot, input, length, &crc);
    if (result != MODULE_OK)
        return module_store_result(slot, result);
    memcpy(&header, input, sizeof(header));
    header.crc = crc;

    /*
     * Целевой поток нельзя возобновлять после изменения его кода.
     * Останавливаем его до захвата общей блокировки Flash.
     */
    result = module_stop_slot(slot);
    if (result != MODULE_OK)
        return module_store_result(slot, result);
    result = w25q128_replace_sector(module_flash_slot_address(slot),
                                    (const u8 *)&header, sizeof(header),
                                    input + sizeof(header),
                                    length - sizeof(header));

    if (result < 0)
        return module_store_result(slot, MODULE_ERR_FLASH);
    return module_validate(slot);
}

/*
 * Проверяет Flash и готовит слот к исполнению непосредственно из QSPI.
 */
MODULE_ROOT int module_prepare_slot_xip(u8 slot)
{
    int result;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    result = module_validate(slot);
    if (result != MODULE_OK)
        return result;

    SCB_InvalidateICache();
    __DSB();
    __ISB();

    module_runtime[slot].public_status.loaded = 1U;
    return module_store_result(slot, MODULE_OK);
}

/*
 * Создаёт отдельный поток RT-Thread и запускает точку входа из QSPI/XIP.
 */
MODULE_ROOT int module_start_slot(u8 slot)
{
    char name[RT_NAME_MAX];
    rt_thread_t thread;
    int result;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (slot >= QSPI_MODULE_SLOT_COUNT)
        return MODULE_ERR_SLOT;
    if (module_runtime[slot].public_status.active)
        return MODULE_ERR_BUSY;

    result = module_prepare_slot_xip(slot);
    if (result != MODULE_OK)
        return result;

    rt_snprintf(name, sizeof(name), "module%u", slot);
    thread = rt_thread_create(name, module_thread_entry,
                              (void *)(rt_ubase_t)slot,
                              MODULE_THREAD_STACK_SIZE,
                              MODULE_THREAD_PRIORITY,
                              MODULE_THREAD_TICK);
    if (thread == RT_NULL)
        return module_store_result(slot, MODULE_ERR_THREAD);

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    module_runtime[slot].thread = thread;
    module_runtime[slot].public_status.active = 1U;
    rt_mutex_release(&module_lock);

    if (rt_thread_startup(thread) != RT_EOK)
    {
        module_stop_slot(slot);
        return module_store_result(slot, MODULE_ERR_THREAD);
    }
    return module_store_result(slot, MODULE_OK);
}

/*
 * Останавливает поток модуля, если он был запущен загрузчиком.
 */
MODULE_ROOT int module_stop_slot(u8 slot)
{
    rt_thread_t thread;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (slot >= QSPI_MODULE_SLOT_COUNT)
        return MODULE_ERR_SLOT;

    /* Remove the internal scheduler hook before deleting its XIP owner. */
    if (slot == PROFILER_MODULE_SLOT)
        thread_profiler_stop();

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    thread = module_runtime[slot].thread;
    module_runtime[slot].thread = RT_NULL;
    module_runtime[slot].public_status.active = 0U;
    rt_mutex_release(&module_lock);

    if (thread != RT_NULL)
    {
        if (thread == rt_thread_self())
            return module_store_result(slot, MODULE_ERR_BUSY);
        rt_thread_delete(thread);
    }
    return module_store_result(slot, MODULE_OK);
}

/*
 * Выполняет полный цикл обновления: остановка, запись, проверка и запуск.
 */
MODULE_ROOT int module_update_from_buf(u8 slot, const u8 *input, u32 length)
{
    int result;

    result = module_stop_slot(slot);
    if (result != MODULE_OK)
        return result;
    result = module_load_slot_buf(slot, input, length);
    if (result != MODULE_OK)
        return result;
    return module_start_slot(slot);
}

/*
 * Копирует текущее состояние выбранного слота для диагностики.
 */
MODULE_ROOT int module_get_status(u8 slot, qspi_module_status_t *status)
{
    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (slot >= QSPI_MODULE_SLOT_COUNT || status == RT_NULL)
        return MODULE_ERR_SLOT;

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    *status = module_runtime[slot].public_status;
    rt_mutex_release(&module_lock);
    return MODULE_OK;
}

MODULE_ROOT int module_object_start(u32 object_id)
{
    qspi_object_record_t record;
    object_module_runtime_t *runtime = RT_NULL;
    rt_thread_t thread;
    char name[RT_NAME_MAX];
    u8 index;
    int result;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (object_id == 0U)
        return QSPI_OBJECT_ERR_PARAM;

    result = qspi_object_find(object_id, &record);
    if (result != QSPI_OBJECT_OK)
        return result;
    if ((record.header.object_type != QSPI_OBJECT_XIP_MODULE &&
         record.header.object_type != QSPI_OBJECT_LUA_VM) ||
        (record.header.flags & QSPI_OBJECT_FLAG_EXECUTABLE) == 0U)
        return QSPI_OBJECT_ERR_TYPE;
    result = qspi_object_verify(&record);
    if (result != QSPI_OBJECT_OK)
        return result;

    SCB_InvalidateICache();
    __DSB();
    __ISB();

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    for (index = 0U; index < QSPI_MODULE_SLOT_COUNT; index++)
    {
        if (object_module_runtime[index].status.object_id == object_id)
        {
            if (object_module_runtime[index].thread != RT_NULL)
            {
                rt_mutex_release(&module_lock);
                return MODULE_ERR_BUSY;
            }
            runtime = &object_module_runtime[index];
            break;
        }
        if (runtime == RT_NULL &&
            object_module_runtime[index].thread == RT_NULL)
            runtime = &object_module_runtime[index];
    }
    if (runtime == RT_NULL)
    {
        rt_mutex_release(&module_lock);
        return MODULE_ERR_BUSY;
    }
    rt_mutex_release(&module_lock);

    rt_snprintf(name, sizeof(name), "obj%04x",
                (unsigned int)(object_id & 0xFFFFU));
    thread = rt_thread_create(name, object_module_thread_entry, runtime,
                              record.header.object_type == QSPI_OBJECT_LUA_VM ?
                                  LUA_THREAD_STACK_SIZE :
                                  MODULE_THREAD_STACK_SIZE,
                              record.header.object_type == QSPI_OBJECT_LUA_VM ?
                                  LUA_THREAD_PRIORITY :
                                  MODULE_THREAD_PRIORITY,
                              MODULE_THREAD_TICK);
    if (thread == RT_NULL)
        return QSPI_OBJECT_ERR_THREAD;

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    runtime->thread = thread;
    runtime->status.active = 1U;
    runtime->status.loaded = 1U;
    runtime->status.last_result = QSPI_OBJECT_OK;
    runtime->status.object_id = object_id;
    runtime->status.generation = record.header.generation;
    runtime->status.entry_address =
        record.header.link_address + record.header.entry_offset;
    runtime->status.first_block = record.first_block;
    runtime->status.block_count = record.block_count;
    runtime->status.directory_generation =
        record.directory_generation;
    rt_mutex_release(&module_lock);

    if (rt_thread_startup(thread) != RT_EOK)
    {
        module_object_stop(object_id);
        return QSPI_OBJECT_ERR_THREAD;
    }
    return QSPI_OBJECT_OK;
}

MODULE_ROOT int module_object_stop(u32 object_id)
{
    object_module_runtime_t *runtime = RT_NULL;
    qspi_object_record_t record;
    rt_thread_t thread;
    u8 index;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (object_id == 0U)
        return QSPI_OBJECT_ERR_PARAM;

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    for (index = 0U; index < QSPI_MODULE_SLOT_COUNT; index++)
    {
        if (object_module_runtime[index].status.object_id == object_id)
        {
            runtime = &object_module_runtime[index];
            break;
        }
    }
    if (runtime == RT_NULL || runtime->thread == RT_NULL)
    {
        rt_mutex_release(&module_lock);
        return QSPI_OBJECT_OK;
    }
    thread = runtime->thread;
    if (thread == rt_thread_self())
    {
        rt_mutex_release(&module_lock);
        return MODULE_ERR_BUSY;
    }
    runtime->thread = RT_NULL;
    runtime->status.active = 0U;
    runtime->status.last_result = QSPI_OBJECT_OK;
    rt_mutex_release(&module_lock);

    if (object_id == PROFILER_OBJECT_ID)
        thread_profiler_stop();
    rt_thread_delete(thread);
    if (qspi_object_find(object_id, &record) == QSPI_OBJECT_OK &&
        record.header.object_type == QSPI_OBJECT_LUA_VM)
        module_lua_status_reset();
    return QSPI_OBJECT_OK;
}

MODULE_ROOT int module_object_stop_if_active(u32 object_id)
{
    rt_bool_t matches;
    u8 index;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    matches = RT_FALSE;
    for (index = 0U; index < QSPI_MODULE_SLOT_COUNT; index++)
    {
        if (object_module_runtime[index].thread != RT_NULL &&
            object_module_runtime[index].status.object_id == object_id)
        {
            matches = RT_TRUE;
            break;
        }
    }
    rt_mutex_release(&module_lock);
    return matches ? module_object_stop(object_id) : MODULE_OK;
}

MODULE_ROOT int module_object_get_status(
    u32 object_id, qspi_object_module_status_t *status)
{
    qspi_object_record_t record;
    u8 index;
    int result;

    if (!module_lock_ready)
        return MODULE_ERR_BUSY;
    if (object_id == 0U || status == RT_NULL)
        return QSPI_OBJECT_ERR_PARAM;
    result = qspi_object_find(object_id, &record);
    if (result != QSPI_OBJECT_OK)
        return result;

    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    memset(status, 0, sizeof(*status));
    status->loaded = 1U;
    status->last_result = QSPI_OBJECT_OK;
    for (index = 0U; index < QSPI_MODULE_SLOT_COUNT; index++)
    {
        if (object_module_runtime[index].status.object_id == object_id)
        {
            status->active =
                object_module_runtime[index].thread != RT_NULL;
            status->last_result =
                object_module_runtime[index].status.last_result;
            break;
        }
    }
    status->object_id = object_id;
    status->generation = record.header.generation;
    status->entry_address =
        record.header.link_address + record.header.entry_offset;
    status->first_block = record.first_block;
    status->block_count = record.block_count;
    status->directory_generation = record.directory_generation;
    rt_mutex_release(&module_lock);
    return QSPI_OBJECT_OK;
}

/*
 * Запрещает переключение задач перед выходом QSPI из memory-mapped режима.
 * Вызывающий поток не должен быть XIP-модулем.
 */
MODULE_ROOT int module_xip_pause_all(void)
{
    u8 slot;
    rt_thread_t self;

    if (!module_lock_ready)
    {
        rt_enter_critical();
        module_xip_scheduler_locked = RT_TRUE;
        return MODULE_OK;
    }
    self = rt_thread_self();
    rt_mutex_take(&module_lock, RT_WAITING_FOREVER);
    for (slot = 0U; slot < QSPI_MODULE_SLOT_COUNT; slot++)
    {
        rt_thread_t thread = module_runtime[slot].thread;

        if (thread == self &&
            module_runtime[slot].public_status.active != 0U)
        {
            rt_mutex_release(&module_lock);
            return MODULE_ERR_BUSY;
        }
    }
    for (slot = 0U; slot < QSPI_MODULE_SLOT_COUNT; slot++)
    {
        if (object_module_runtime[slot].thread == self &&
            object_module_runtime[slot].status.active != 0U)
        {
            rt_mutex_release(&module_lock);
            return MODULE_ERR_BUSY;
        }
    }
    rt_mutex_release(&module_lock);
    rt_enter_critical();
    module_xip_scheduler_locked = RT_TRUE;
    return MODULE_OK;
}

/*
 * Возвращает планирование задач после восстановления memory-mapped QSPI.
 */
MODULE_ROOT void module_xip_resume_all(void)
{
    if (module_xip_scheduler_locked)
    {
        module_xip_scheduler_locked = RT_FALSE;
        rt_exit_critical();
    }
}
