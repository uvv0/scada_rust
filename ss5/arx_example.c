#include "arx_flashdb.h"

typedef struct
{
    rt_uint32_t timestamp;
    rt_uint16_t tii[48];
} tii_archive_record_t;

static tii_archive_record_t tii_record;

/*
 * TIT[390]        - индекс последней записи;
 * TIT[2441]       - индекс требуемой записи, задаёт внутренняя программа;
 * TIT[2442]       - внутренняя команда: 1 записать, 2 подкачать;
 * TIT[2443]       - 0 готово, 1 занято, иначе код ошибки;
 * Адреса 7000..7049 функции 04 перехватываются этим архивом.
 */
static arx_flashdb_config_t tii_archive =
{
    .name          = "tii",
    .partition     = "archive",
    .record        = &tii_record,
    .record_size   = sizeof(tii_record),
    .record_count  = 1000,
    .index_tit     = 390,
    .request_tit   = 2441,
    .command_tit   = 2442,
    .status_tit    = 2443,
    .VirStartAdr   = 7000,
    .VirStopAdr    = 7050
};

/* Вызывается после готовности FAL-раздела storage. */
int arx_example_init(void)
{
    return arx_flashdb_start(&tii_archive);
}

/* Заполняет структуру до выдачи внутренней команды TIT[2442] = 1. */
void arx_example_fill(rt_uint32_t timestamp, const rt_uint16_t *values)
{
    rt_uint16_t i;
    tii_record.timestamp = timestamp;
    for (i = 0; i < 48; i++)
        tii_record.tii[i] = values[i];
}

#define SLOT1_ARCHIVE_MARKER 0x5101U

struct slot1_recent_state
{
    arx_slot1_sample_t *samples;
    rt_uint16_t capacity;
    rt_uint16_t count;
};

static rt_bool_t slot1_recent_visitor(const void *record, rt_uint16_t size,
                                      void *context)
{
    const tii_archive_record_t *source = record;
    struct slot1_recent_state *state = context;
    union { rt_uint32_t bits; float value; } converted;

    if (size != sizeof(*source) || source->tii[0] != SLOT1_ARCHIVE_MARKER)
        return RT_FALSE;
    if (state->count >= state->capacity)
        return RT_TRUE;
    converted.bits = ((rt_uint32_t)source->tii[3] << 16) | source->tii[4];
    state->samples[state->count].timestamp = source->timestamp;
    state->samples[state->count].value = converted.value;
    state->count++;
    return state->count >= state->capacity;
}

rt_uint16_t arx_example_slot1_recent(arx_slot1_sample_t *samples,
                                     rt_uint16_t capacity)
{
    struct slot1_recent_state state;
    if (!samples || !capacity)
        return 0U;
    state.samples = samples;
    state.capacity = capacity;
    state.count = 0U;
    arx_flashdb_visit_recent(1000U, slot1_recent_visitor, &state);
    return state.count;
}
