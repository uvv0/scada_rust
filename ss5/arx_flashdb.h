#ifndef ARX_FLASHDB_H
#define ARX_FLASHDB_H

#include <rtthread.h>

typedef struct
{
    const char *name;
    const char *partition;
    void *record;
    rt_uint16_t record_size;
    rt_uint16_t record_count;
    rt_uint16_t index_tit;
    rt_uint16_t request_tit;
    rt_uint16_t command_tit;
    rt_uint16_t status_tit;
    rt_uint32_t VirStartAdr;       /* первый виртуальный адрес архива */
    rt_uint32_t VirStopAdr;        /* адрес после последнего слова */
    rt_uint32_t flash_begin;
    rt_uint32_t flash_end;
} arx_flashdb_config_t;

rt_err_t arx_flashdb_start(arx_flashdb_config_t *config);

/*
 * Перехватывает чтение функции 04 внутри диапазона архива.
 * Возвращает RT_TRUE, если адрес принадлежит архиву.
 */
rt_bool_t arx_flashdb_read_holding(rt_uint16_t address, rt_uint16_t *value);
typedef rt_bool_t (*arx_flashdb_visitor_t)(const void *record,
                                           rt_uint16_t size,
                                           void *context);
rt_uint16_t arx_flashdb_visit_recent(rt_uint16_t limit,
                                     arx_flashdb_visitor_t visitor,
                                     void *context);

#endif
