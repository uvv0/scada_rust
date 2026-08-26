#include "arx_flashdb.h"
#include "../elam_modbus.h"

#ifdef PKG_USING_FLASHDB
#include <flashdb.h>
#endif

#define ARX_COMMAND_APPEND 1U
#define ARX_COMMAND_LOAD   2U
#define ARX_STATUS_BUSY    1U

struct arx_context
{
    arx_flashdb_config_t *cfg;
    struct rt_mutex record_lock;
#ifdef PKG_USING_FLASHDB
    struct fdb_tsdb db;
#endif
};

static struct arx_context arx;

/* Проверяет структуру, управляющие регистры и размер окна holding. */
static rt_err_t validate_config(const arx_flashdb_config_t *cfg)
{
    rt_uint16_t words;
    if (!cfg || !cfg->record || cfg->record_size < sizeof(rt_uint32_t) ||
        !cfg->record_count)
        return -RT_EINVAL;
    if (cfg->VirStartAdr < ELAM_HOLDING_REGISTER_MAX ||
        cfg->VirStartAdr >= cfg->VirStopAdr ||
        cfg->VirStopAdr > 0x10000UL ||
        cfg->index_tit >= ELAM_MODBUS_REGISTER_MAX ||
        cfg->request_tit >= ELAM_MODBUS_REGISTER_MAX ||
        cfg->command_tit >= ELAM_MODBUS_REGISTER_MAX ||
        cfg->status_tit >= ELAM_MODBUS_REGISTER_MAX)
        return -RT_EINVAL;
    words = (rt_uint16_t)((cfg->record_size + 1U) / 2U);
    return (cfg->VirStopAdr - cfg->VirStartAdr) >= words ?
           RT_EOK : -RT_EFULL;
}

#ifdef PKG_USING_FLASHDB
/* Возвращает timestamp из начала пользовательской структуры. */
static fdb_time_t archive_time(void)
{
    return (fdb_time_t)(*(rt_uint32_t *)arx.cfg->record);
}

/* Считает корректные записи при старте для восстановления индекса TIT. */
static bool count_callback(fdb_tsl_t tsl, void *arg)
{
    rt_uint32_t *count = (rt_uint32_t *)arg;
    if (tsl->status == FDB_TSL_WRITE)
        (*count)++;
    return false;
}

/* Восстанавливает циклический u16-индекс по содержимому TSDB. */
static void restore_index(void)
{
    rt_uint32_t count = 0;
    fdb_tsl_iter(&arx.db, count_callback, &count);
    TIT[arx.cfg->index_tit] = count ?
        (rt_uint16_t)((count - 1U) % arx.cfg->record_count) : 0xffffU;
}

/* Добавляет заполненную структуру и обновляет u16-индекс в TIT. */
static rt_err_t append_record(void)
{
    struct fdb_blob blob;
    if (fdb_tsl_append(&arx.db, fdb_blob_make(
            &blob, arx.cfg->record, arx.cfg->record_size)) != FDB_NO_ERR)
        return -RT_ERROR;
    TIT[arx.cfg->index_tit] =
        (rt_uint16_t)((TIT[arx.cfg->index_tit] + 1U) %
                      arx.cfg->record_count);
    return RT_EOK;
}

struct load_state
{
    rt_uint16_t skip;
    rt_bool_t loaded;
};

/* Читает выбранную TSDB-запись и копирует её в TIT/holding. */
static bool load_callback(fdb_tsl_t tsl, void *arg)
{
    struct load_state *state = (struct load_state *)arg;
    struct fdb_blob blob;
    if (state->skip)
    {
        state->skip--;
        return false;
    }
    rt_mutex_take(&arx.record_lock, RT_WAITING_FOREVER);
    fdb_blob_read((fdb_db_t)&arx.db, fdb_tsl_to_blob(
        tsl, fdb_blob_make(&blob, arx.cfg->record, arx.cfg->record_size)));
    rt_mutex_release(&arx.record_lock);
    state->loaded = RT_TRUE;
    return true;
}

/* Находит требуемый логический индекс обратным обходом FlashDB. */
static rt_err_t load_record(void)
{
    struct load_state state;
    rt_uint16_t last = TIT[arx.cfg->index_tit];
    rt_uint16_t wanted = TIT[arx.cfg->request_tit];
    if (wanted >= arx.cfg->record_count)
        return -RT_EINVAL;
    state.skip = (rt_uint16_t)((last + arx.cfg->record_count - wanted) %
                               arx.cfg->record_count);
    state.loaded = RT_FALSE;
    fdb_tsl_iter_reverse(&arx.db, load_callback, &state);
    return state.loaded ? RT_EOK : -RT_ENOSYS;
}
#endif

/* Отдельный поток выполняет запись и подкачку без блокировки Modbus. */
static void archive_thread(void *parameter)
{
    arx_flashdb_config_t *cfg = (arx_flashdb_config_t *)parameter;
    rt_uint16_t command;
    rt_err_t result;
    for (;;)
    {
        command = TIT[cfg->command_tit];
        if (!command)
        {
            rt_thread_mdelay(10);
            continue;
        }
        TIT[cfg->status_tit] = ARX_STATUS_BUSY;
#ifdef PKG_USING_FLASHDB
        result = command == ARX_COMMAND_APPEND ? append_record() :
                 command == ARX_COMMAND_LOAD ? load_record() : -RT_EINVAL;
#else
        result = -RT_ENOSYS;
#endif
        TIT[cfg->status_tit] = result == RT_EOK ? 0U : (rt_uint16_t)(-result);
        TIT[cfg->command_tit] = 0U;
    }
}

/* Инициализирует TSDB и создаёт отдельный поток arx_fdb. */
rt_err_t arx_flashdb_start(arx_flashdb_config_t *config)
{
    rt_thread_t thread;
    rt_err_t result = validate_config(config);
    if (result != RT_EOK)
        return result;
    arx.cfg = config;
    rt_mutex_init(&arx.record_lock, "arx_rec", RT_IPC_FLAG_PRIO);
#ifdef PKG_USING_FLASHDB
    if (fdb_tsdb_init(&arx.db, config->name, config->partition,
                      archive_time, config->record_size, RT_NULL) != FDB_NO_ERR)
        return -RT_ERROR;
    {
        bool rollover = true;
        fdb_tsdb_control(&arx.db, FDB_TSDB_CTRL_SET_ROLLOVER, &rollover);
    }
    restore_index();
#endif
    thread = rt_thread_create("arx_fdb", archive_thread, config, 2048, 18, 10);
    return thread ? rt_thread_startup(thread) : -RT_ENOMEM;
}

/*
 * Возвращает слово архивной структуры для адресов holding_begin..holding_end.
 * Адреса вне диапазона архива должен обработать обычный holding.
 */
rt_bool_t arx_flashdb_read_holding(rt_uint16_t address, rt_uint16_t *value)
{
    rt_uint16_t offset;
    rt_uint16_t words;
    if (!arx.cfg || !value ||
        (rt_uint32_t)address < arx.cfg->VirStartAdr ||
        (rt_uint32_t)address >= arx.cfg->VirStopAdr)
        return RT_FALSE;
    offset = (rt_uint16_t)((rt_uint32_t)address - arx.cfg->VirStartAdr);
    words = (rt_uint16_t)((arx.cfg->record_size + 1U) / 2U);
    if (offset >= words)
    {
        *value = 0U;
        return RT_TRUE;
    }
    rt_mutex_take(&arx.record_lock, RT_WAITING_FOREVER);
    *value = ((const rt_uint16_t *)arx.cfg->record)[offset];
    rt_mutex_release(&arx.record_lock);
    return RT_TRUE;
}

#ifdef PKG_USING_FLASHDB
struct recent_visit_state
{
    rt_uint16_t remaining;
    rt_uint16_t visited;
    arx_flashdb_visitor_t visitor;
    void *context;
};

static bool recent_visit_callback(fdb_tsl_t tsl, void *arg)
{
    struct recent_visit_state *state = arg;
    struct fdb_blob blob;

    if (!state->remaining || tsl->status != FDB_TSL_WRITE)
        return state->remaining == 0U;
    fdb_blob_read((fdb_db_t)&arx.db, fdb_tsl_to_blob(
        tsl, fdb_blob_make(&blob, arx.cfg->record, arx.cfg->record_size)));
    state->visited++;
    state->remaining--;
    return state->visitor(arx.cfg->record, arx.cfg->record_size,
                          state->context) || state->remaining == 0U;
}
#endif

rt_uint16_t arx_flashdb_visit_recent(rt_uint16_t limit,
                                     arx_flashdb_visitor_t visitor,
                                     void *context)
{
#ifdef PKG_USING_FLASHDB
    struct recent_visit_state state;
    if (!arx.cfg || !visitor || !limit)
        return 0U;
    state.remaining = limit;
    state.visited = 0U;
    state.visitor = visitor;
    state.context = context;
    rt_mutex_take(&arx.record_lock, RT_WAITING_FOREVER);
    fdb_tsl_iter_reverse(&arx.db, recent_visit_callback, &state);
    rt_mutex_release(&arx.record_lock);
    return state.visited;
#else
    return 0U;
#endif
}
