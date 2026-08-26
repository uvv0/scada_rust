#ifndef THREAD_PROFILER_H
#define THREAD_PROFILER_H

#include <rtthread.h>

/*
 * 1 - профилировщик потоков включён;
 * 0 - весь код профилировщика исключён при компиляции.
 */
#ifndef THREAD_PROFILER_ENABLE
#define THREAD_PROFILER_ENABLE 1
#endif

#define THREAD_PROFILER_MAX_THREADS 24U
#define THREAD_PROFILER_MB_BASE        8000U
#define THREAD_PROFILER_MB_HEADER_SIZE 10U
#define THREAD_PROFILER_MB_THREAD_SIZE 20U
#define THREAD_PROFILER_MB_END \
    (THREAD_PROFILER_MB_BASE + THREAD_PROFILER_MB_HEADER_SIZE + \
     THREAD_PROFILER_MAX_THREADS * THREAD_PROFILER_MB_THREAD_SIZE)
#define THREAD_PROFILER_MB_CONTROL THREAD_PROFILER_MB_END

struct thread_cpu_stat
{
    rt_thread_t thread;
    char name[RT_NAME_MAX];
    volatile rt_uint16_t cpu_x100;       /* CPU: 10000 = 100.00 %. */
    volatile rt_uint32_t window_us;      /* Время за последний интервал. */
    volatile rt_uint64_t total_us;       /* Суммарное время работы. */
    volatile rt_uint32_t max_run_us;     /* Максимум без переключения. */
    volatile rt_uint32_t switch_count;   /* Число снятий потока с CPU. */

    /* Служебные счётчики тактов DWT. */
    volatile rt_uint64_t total_cycles;
    volatile rt_uint64_t window_cycles;
    volatile rt_uint32_t max_run_cycles;
};

#if THREAD_PROFILER_ENABLE

extern volatile struct thread_cpu_stat
    thread_cpu_stat[THREAD_PROFILER_MAX_THREADS];
extern volatile rt_uint8_t thread_cpu_stat_count;
extern volatile rt_uint16_t cpu_load_x100;
extern volatile rt_uint32_t cpu_profile_window_us;

/* Starts/stops profiling. Profiling is disabled after reset. */
rt_err_t thread_profiler_init(void);
rt_err_t thread_profiler_stop(void);
rt_bool_t thread_profiler_is_active(void);
rt_err_t thread_profiler_snapshot_begin(rt_uint8_t *count,
                                        rt_uint64_t *total_cycles,
                                        rt_uint32_t *cycles_per_us);
rt_err_t thread_profiler_snapshot_item(rt_uint8_t index,
                                       struct thread_cpu_stat *item);
rt_err_t thread_profiler_publish(rt_uint8_t index,
                                 const struct thread_cpu_stat *item,
                                 rt_uint16_t load_x100,
                                 rt_uint32_t window_total_us);

#else

/* При THREAD_PROFILER_ENABLE=0 вызов полностью удаляется оптимизатором. */
static rt_err_t thread_profiler_init(void)
{
    return RT_EOK;
}
static rt_err_t thread_profiler_stop(void) { return RT_EOK; }
static rt_bool_t thread_profiler_is_active(void) { return RT_FALSE; }

#endif

/*
 * Читает виртуальные Holding-регистры профилировщика 8000..8489.
 * Регистры доступны только для Modbus function 03 и не хранятся во FlashDB.
 */
rt_err_t thread_profiler_modbus_read(rt_uint16_t address,
                                     rt_uint16_t *values,
                                     rt_uint16_t count);

#endif
