#include "thread_profiler.h"

#if THREAD_PROFILER_ENABLE

#include <board.h>

volatile struct thread_cpu_stat
    thread_cpu_stat[THREAD_PROFILER_MAX_THREADS];
volatile rt_uint8_t thread_cpu_stat_count;
volatile rt_uint16_t cpu_load_x100;
volatile rt_uint32_t cpu_profile_window_us;

static rt_uint32_t current_start_cycles;
static volatile rt_bool_t profiler_active;

/*
 * Находит статистику потока или создаёт для него новую строку.
 *
 * thread - указатель на поток RT-Thread.
 * Возвращает индекс строки либо -1, если таблица заполнена.
 */
static int find_or_add_thread(rt_thread_t thread)
{
    rt_uint8_t i;

    if (thread == RT_NULL)
        return -1;

    for (i = 0; i < thread_cpu_stat_count; i++)
        if (thread_cpu_stat[i].thread == thread)
            return i;

    if (thread_cpu_stat_count >= THREAD_PROFILER_MAX_THREADS)
        return -1;

    i = thread_cpu_stat_count++;
    thread_cpu_stat[i].thread = thread;
    rt_strncpy((char *)thread_cpu_stat[i].name, thread->name, RT_NAME_MAX);
    thread_cpu_stat[i].name[RT_NAME_MAX - 1U] = '\0';
    return i;
}

/*
 * Учитывает время уходящего потока при каждом переключении контекста.
 *
 * from - поток, который снимается с CPU;
 * to   - поток, которому передаётся CPU.
 */
static void scheduler_hook(rt_thread_t from, rt_thread_t to)
{
    if (!profiler_active)
        return;
    rt_uint32_t now = DWT->CYCCNT;
    rt_uint32_t elapsed = now - current_start_cycles;
    int index = find_or_add_thread(from);

    if (index >= 0)
    {
        thread_cpu_stat[index].total_cycles += elapsed;
        thread_cpu_stat[index].window_cycles += elapsed;
        thread_cpu_stat[index].switch_count++;
        if (elapsed > thread_cpu_stat[index].max_run_cycles)
            thread_cpu_stat[index].max_run_cycles = elapsed;
    }

    (void)find_or_add_thread(to);
    current_start_cycles = now;
}

/*
 * Формирует удобный для IAR Watch снимок статистики раз в секунду.
 *
 * parameter не используется. Поток работает постоянно и не возвращается.
 */
/*
 * Запускает аппаратный счётчик DWT и поток подготовки статистики.
 *
 * Возвращает RT_EOK при успешном запуске либо -RT_ENOMEM, если поток
 * профилировщика создать не удалось.
 */
rt_err_t thread_profiler_init(void)
{
    rt_base_t level;

    if (profiler_active)
        return RT_EOK;

    level = rt_hw_interrupt_disable();
    rt_memset((void *)thread_cpu_stat, 0, sizeof(thread_cpu_stat));
    thread_cpu_stat_count = 0U;
    cpu_load_x100 = 0U;
    cpu_profile_window_us = 0U;
    rt_hw_interrupt_enable(level);

    CoreDebug->DEMCR |= CoreDebug_DEMCR_TRCENA_Msk;
    DWT->CYCCNT = 0U;
    DWT->CTRL |= DWT_CTRL_CYCCNTENA_Msk;

    current_start_cycles = DWT->CYCCNT;
    (void)find_or_add_thread(rt_thread_self());
    profiler_active = RT_TRUE;
    rt_scheduler_sethook(scheduler_hook);
    return RT_EOK;
}

rt_err_t thread_profiler_stop(void)
{
    rt_base_t level;

    level = rt_hw_interrupt_disable();
    rt_scheduler_sethook(RT_NULL);
    profiler_active = RT_FALSE;
    rt_hw_interrupt_enable(level);
    DWT->CTRL &= ~DWT_CTRL_CYCCNTENA_Msk;
    return RT_EOK;
}

rt_bool_t thread_profiler_is_active(void)
{
    return profiler_active;
}

rt_err_t thread_profiler_snapshot_begin(rt_uint8_t *count,
                                        rt_uint64_t *total_cycles,
                                        rt_uint32_t *cycles_per_us)
{
    rt_base_t level;
    rt_uint8_t i;

    if (!profiler_active || count == RT_NULL ||
        total_cycles == RT_NULL || cycles_per_us == RT_NULL)
        return -RT_EINVAL;
    level = rt_hw_interrupt_disable();
    *total_cycles = 0U;
    for (i = 0U; i < thread_cpu_stat_count; i++)
    {
        thread_cpu_stat[i].window_us =
            (rt_uint32_t)thread_cpu_stat[i].window_cycles;
        *total_cycles += thread_cpu_stat[i].window_cycles;
        thread_cpu_stat[i].window_cycles = 0U;
    }
    *count = thread_cpu_stat_count;
    *cycles_per_us = SystemCoreClock / 1000000U;
    if (*cycles_per_us == 0U)
        *cycles_per_us = 1U;
    rt_hw_interrupt_enable(level);
    return RT_EOK;
}

rt_err_t thread_profiler_snapshot_item(rt_uint8_t index,
                                       struct thread_cpu_stat *item)
{
    rt_base_t level;
    if (item == RT_NULL || index >= thread_cpu_stat_count)
        return -RT_EINVAL;
    level = rt_hw_interrupt_disable();
    *item = thread_cpu_stat[index];
    rt_hw_interrupt_enable(level);
    return RT_EOK;
}

rt_err_t thread_profiler_publish(rt_uint8_t index,
                                 const struct thread_cpu_stat *item,
                                 rt_uint16_t load_x100,
                                 rt_uint32_t window_total_us)
{
    rt_base_t level;
    if (item == RT_NULL || index >= thread_cpu_stat_count)
        return -RT_EINVAL;
    level = rt_hw_interrupt_disable();
    thread_cpu_stat[index].cpu_x100 = item->cpu_x100;
    thread_cpu_stat[index].window_us = item->window_us;
    thread_cpu_stat[index].total_us = item->total_us;
    thread_cpu_stat[index].max_run_us = item->max_run_us;
    cpu_load_x100 = load_x100;
    cpu_profile_window_us = window_total_us;
    rt_hw_interrupt_enable(level);
    return RT_EOK;
}

#endif

/*
 * Возвращает одно 16-битное слово виртуальной Holding-области.
 *
 * address - абсолютный Modbus-адрес 8000..8489.
 * Старшие части многословных значений передаются первыми.
 */
static rt_uint16_t profiler_modbus_word(rt_uint16_t address)
{
    rt_uint16_t offset = address - THREAD_PROFILER_MB_BASE;

    if (offset == 0U)
        return 0x5052U; /* ASCII "PR". */
    if (offset == 1U)
        return 1U;
    if (offset == 2U)
        return thread_profiler_is_active() ? 1U : 0U;
    if (offset == 7U)
        return THREAD_PROFILER_MAX_THREADS;
    if (offset == 8U)
        return THREAD_PROFILER_MB_THREAD_SIZE;
    if (offset == 9U)
        return THREAD_PROFILER_MB_BASE + THREAD_PROFILER_MB_HEADER_SIZE;

#if THREAD_PROFILER_ENABLE
    if (offset == 3U)
        return thread_cpu_stat_count;
    if (offset == 4U)
        return cpu_load_x100;
    if (offset == 5U)
        return (rt_uint16_t)(cpu_profile_window_us >> 16);
    if (offset == 6U)
        return (rt_uint16_t)cpu_profile_window_us;

    if (offset >= THREAD_PROFILER_MB_HEADER_SIZE)
    {
        rt_uint16_t item = offset - THREAD_PROFILER_MB_HEADER_SIZE;
        rt_uint16_t index = item / THREAD_PROFILER_MB_THREAD_SIZE;
        rt_uint16_t field = item % THREAD_PROFILER_MB_THREAD_SIZE;
        volatile struct thread_cpu_stat *stat;

        if (index >= thread_cpu_stat_count)
            return 0U;
        stat = &thread_cpu_stat[index];

        if (field < 8U)
        {
            rt_uint16_t char_index = field * 2U;
            rt_uint8_t first = (rt_uint8_t)stat->name[char_index];
            rt_uint8_t second = (rt_uint8_t)stat->name[char_index + 1U];
            return (rt_uint16_t)
                (((rt_uint16_t)first << 8) | second);
        }
        if (field == 8U)
            return stat->cpu_x100;
        if (field == 9U)
            return (rt_uint16_t)(stat->window_us >> 16);
        if (field == 10U)
            return (rt_uint16_t)stat->window_us;
        if (field >= 11U && field <= 14U)
        {
            rt_uint16_t shift = (rt_uint16_t)((14U - field) * 16U);
            return (rt_uint16_t)(stat->total_us >> shift);
        }
        if (field == 15U)
            return (rt_uint16_t)(stat->max_run_us >> 16);
        if (field == 16U)
            return (rt_uint16_t)stat->max_run_us;
        if (field == 17U)
            return (rt_uint16_t)(stat->switch_count >> 16);
        if (field == 18U)
            return (rt_uint16_t)stat->switch_count;
        if (field == 19U)
        {
            rt_thread_t thread = stat->thread;
            return (rt_uint16_t)
                (((rt_uint16_t)thread->current_priority << 8) |
                 thread->stat);
        }
    }
#endif

    return 0U;
}

/*
 * Читает диапазон виртуальных Holding-регистров профилировщика.
 *
 * address - первый адрес, допустимы 8000..8489;
 * values  - массив назначения;
 * count   - количество 16-битных регистров.
 *
 * Возвращает RT_EOK либо -RT_EINVAL при ошибке диапазона или указателя.
 */
rt_err_t thread_profiler_modbus_read(rt_uint16_t address,
                                     rt_uint16_t *values,
                                     rt_uint16_t count)
{
    rt_base_t level;
    rt_uint16_t i;

    if (values == RT_NULL || count == 0U ||
        address < THREAD_PROFILER_MB_BASE ||
        address >= THREAD_PROFILER_MB_END ||
        count > THREAD_PROFILER_MB_END - address)
        return -RT_EINVAL;

    level = rt_hw_interrupt_disable();
    for (i = 0; i < count; i++)
        values[i] = profiler_modbus_word(address + i);
    rt_hw_interrupt_enable(level);
    return RT_EOK;
}
