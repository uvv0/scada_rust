#include "../module_service_api.h"
#include "../../thread_profiler.h"

#define MODULE_BODY_OFFSET       12U
#define MODULE_TYPE_PROFILER     3U
#define MODULE_VERSION           1U

#pragma pack(push, 1)
typedef struct
{
    u16 crc;
    u16 addr;
    u16 size;
    u16 type;
    u16 num;
} profiler_module_header_t;
#pragma pack(pop)

#pragma location=".module2_header"
__root const profiler_module_header_t module_header_slot2 =
{
    0U,
    MODULE_BODY_OFFSET,
    0U,
    MODULE_TYPE_PROFILER,
    MODULE_VERSION
};

/*
 * Slot 2 owns only the profiler lifetime. The scheduler hook and counters
 * remain in internal memory, so stopping or replacing this XIP image cannot
 * leave a callback pointing into QSPI.
 */
#pragma location=".module2_entry"
__root void module_entry_slot2(void)
{
    const module_service_api_t *api =
        (const module_service_api_t *)MODULE_SERVICE_API_ADDRESS;

    if (api->magic != MODULE_SERVICE_API_MAGIC ||
        api->version != MODULE_SERVICE_API_VERSION ||
        api->size < sizeof(module_service_api_t) ||
        api->profiler_start == 0 ||
        api->profiler_stop == 0 ||
        api->profiler_snapshot_begin == 0 ||
        api->profiler_snapshot_item == 0 ||
        api->profiler_publish == 0)
        return;

    if (api->profiler_start() != 0)
        return;

    for (;;)
    {
        struct thread_cpu_stat item;
        unsigned long long total_cycles;
        unsigned long long idle_cycles = 0U;
        u32 cycles_per_us;
        u32 total_us;
        u16 load_x100;
        u8 count;
        u8 index;

        api->delay_ms(1000U);
        if (api->profiler_snapshot_begin(&count, &total_cycles,
                                         &cycles_per_us) != 0)
            continue;
        for (index = 0U; index < count; index++)
        {
            if (api->profiler_snapshot_item(index, &item) == 0 &&
                item.name[0] == 't' && item.name[1] == 'i' &&
                item.name[2] == 'd' && item.name[3] == 'l' &&
                item.name[4] == 'e')
                idle_cycles += item.window_us;
        }
        total_us = (u32)(total_cycles / cycles_per_us);
        load_x100 = total_cycles ?
            (u16)(10000U - (u16)((idle_cycles * 10000U) /
                                  total_cycles)) : 0U;
        for (index = 0U; index < count; index++)
        {
            unsigned long long window_cycles;
            if (api->profiler_snapshot_item(index, &item) != 0)
                continue;
            window_cycles = item.window_us;
            item.cpu_x100 = total_cycles ?
                (u16)((window_cycles * 10000U) / total_cycles) : 0U;
            item.window_us = (u32)(window_cycles / cycles_per_us);
            item.total_us = item.total_cycles / cycles_per_us;
            item.max_run_us = item.max_run_cycles / cycles_per_us;
            api->profiler_publish(index, &item, load_x100, total_us);
        }
    }
}

