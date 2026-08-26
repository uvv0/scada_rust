#ifndef ARX_EXAMPLE_H
#define ARX_EXAMPLE_H

#include <rtthread.h>

int arx_example_init(void);
void arx_example_fill(rt_uint32_t timestamp, const rt_uint16_t *values);
typedef struct
{
    rt_uint32_t timestamp;
    float value;
} arx_slot1_sample_t;
rt_uint16_t arx_example_slot1_recent(arx_slot1_sample_t *samples,
                                     rt_uint16_t capacity);

#endif
