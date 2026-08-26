#include <rtthread.h>
#include <string.h>
#include "tag_poll_scheduler.h"
#include "rs485_master.h"
#include "elam_modbus.h"

#define POLL_THREAD_STACK   3072U
#define POLL_THREAD_PRIO    18U
#define POLL_IDLE_MS        10U
#define POLL_TIMEOUT_MS     300U

#pragma pack(push, 1)
typedef struct
{
    u16 source_address;
    u16 poll_ms;
    u32 scale_bits;
    u32 offset_bits;
    u32 next_due;
    u8 type;
    u8 source_kind;
    u8 active;
} tag_poll_plan_t;
#pragma pack(pop)

static tag_poll_plan_t plans[TAG_REGISTRY_CAPACITY];
static struct rt_mutex plans_lock;
static rt_thread_t poll_thread;
static u16 poll_cursor;

volatile u32 tag_poll_success_count;
volatile u32 tag_poll_error_count;

static u16 plan_index(u8 port, u8 device, u8 sensor)
{
    return (u16)(((u16)port * TAG_DEVICE_COUNT +
                  ((u16)device - 1U)) * TAG_DEVICE_TAG_COUNT +
                 ((u16)sensor - 1U));
}

static tag_id_t index_tag_id(u16 index)
{
    u16 per_port = TAG_DEVICE_COUNT * TAG_DEVICE_TAG_COUNT;
    u8 port = (u8)(index / per_port);
    u16 rest = (u16)(index % per_port);
    u8 device = (u8)(rest / TAG_DEVICE_TAG_COUNT + 1U);
    u8 sensor = (u8)(rest % TAG_DEVICE_TAG_COUNT + 1U);
    return TAG_ID(port, device, sensor);
}

static u16 swap16(u16 value)
{
    return (u16)((value << 8) | (value >> 8));
}

static u32 words_to_u32(const u16 *words, u8 order)
{
    u16 first = words[0];
    u16 second = words[1];

    if (order == TAG_WORD_ORDER_CDAB ||
        order == TAG_WORD_ORDER_DCBA)
    {
        u16 temporary = first;
        first = second;
        second = temporary;
    }
    if (order == TAG_WORD_ORDER_BADC ||
        order == TAG_WORD_ORDER_DCBA)
    {
        first = swap16(first);
        second = swap16(second);
    }
    return ((u32)first << 16) | second;
}

static void u32_to_words(u32 value, u8 order, u16 *words)
{
    u16 first = (u16)(value >> 16);
    u16 second = (u16)value;

    if (order == TAG_WORD_ORDER_BADC ||
        order == TAG_WORD_ORDER_DCBA)
    {
        first = swap16(first);
        second = swap16(second);
    }
    if (order == TAG_WORD_ORDER_CDAB ||
        order == TAG_WORD_ORDER_DCBA)
    {
        u16 temporary = first;
        first = second;
        second = temporary;
    }
    words[0] = first;
    words[1] = second;
}

static float bits_to_float(u32 bits)
{
    union { u32 bits; float value; } data;
    data.bits = bits;
    return data.value;
}

static u32 float_to_bits(float value)
{
    union { u32 bits; float value; } data;
    data.value = value;
    return data.bits;
}

static u32 convert_value(const tag_poll_plan_t *plan, const u16 *words)
{
    u8 order = (u8)((plan->source_kind & TAG_SOURCE_ORDER_MASK) >>
                    TAG_SOURCE_ORDER_SHIFT);
    u32 raw;
    float value;
    float scale = bits_to_float(plan->scale_bits);
    float offset = bits_to_float(plan->offset_bits);

    if (plan->type == TAG_TYPE_FLOAT32 ||
        plan->type == TAG_TYPE_UINT32 ||
        plan->type == TAG_TYPE_INT32)
        raw = words_to_u32(words, order);
    else
    {
        u16 word = words[0];
        if (order == TAG_WORD_ORDER_BADC ||
            order == TAG_WORD_ORDER_DCBA)
            word = swap16(word);
        raw = word;
    }

    switch (plan->type)
    {
    case TAG_TYPE_FLOAT32:
        value = bits_to_float(raw) * scale + offset;
        return float_to_bits(value);
    case TAG_TYPE_BOOL:
        value = (float)(raw != 0U) * scale + offset;
        return value != 0.0f ? 1U : 0U;
    case TAG_TYPE_UINT16:
        value = (float)(u16)raw * scale + offset;
        if (value < 0.0f) value = 0.0f;
        if (value > 65535.0f) value = 65535.0f;
        return (u16)value;
    case TAG_TYPE_INT16:
        value = (float)(s16)(u16)raw * scale + offset;
        if (value < -32768.0f) value = -32768.0f;
        if (value > 32767.0f) value = 32767.0f;
        return (u32)(s32)(s16)value;
    case TAG_TYPE_UINT32:
        value = (float)raw * scale + offset;
        if (value < 0.0f) value = 0.0f;
        return (u32)value;
    default:
        value = (float)(s32)raw * scale + offset;
        return (u32)(s32)value;
    }
}

static s32 read_source(tag_id_t id, const tag_poll_plan_t *plan,
                       u16 *words, u16 count)
{
    u8 source = plan->source_kind & TAG_SOURCE_KIND_MASK;
    u16 index;

    if (source == TAG_SOURCE_MODBUS)
        return rs485_master_read_holding((u8)TAG_PORT(id),
                                         (u8)TAG_DEVICE(id),
                                         plan->source_address, count,
                                         words, POLL_TIMEOUT_MS);
    if (source == TAG_SOURCE_TIT)
    {
        for (index = 0U; index < count; index++)
            if (elam_modbus_read_register(
                    (u16)(plan->source_address + index),
                    &words[index]) != RT_EOK)
                return -RT_ERROR;
        return RT_EOK;
    }
    return -RT_ENOSYS;
}

int tag_poll_scheduler_write_float(tag_id_t id, float value)
{
    tag_poll_plan_t plan;
    u16 words[2];
    u8 source;
    u8 order;
    float scale;
    float offset;
    float raw_value;
    u32 raw;
    u8 port = (u8)TAG_PORT(id);
    u8 device = (u8)TAG_DEVICE(id);
    u8 sensor = (u8)TAG_SENSOR(id);
    s32 result;

    if (port >= TAG_PORT_COUNT || device == 0U ||
        device > TAG_DEVICE_COUNT || sensor == 0U ||
        sensor > TAG_DEVICE_TAG_COUNT)
        return TAG_ERR_PARAM;

    rt_mutex_take(&plans_lock, RT_WAITING_FOREVER);
    plan = plans[plan_index(port, device, sensor)];
    rt_mutex_release(&plans_lock);
    source = plan.source_kind & TAG_SOURCE_KIND_MASK;
    if (plan.type != TAG_TYPE_FLOAT32)
        return TAG_ERR_TYPE;
    if (source == TAG_SOURCE_NONE)
        return TAG_OK;

    scale = bits_to_float(plan.scale_bits);
    offset = bits_to_float(plan.offset_bits);
    if (scale == 0.0f)
        return TAG_ERR_PARAM;
    raw_value = (value - offset) / scale;
    raw = float_to_bits(raw_value);
    order = (u8)((plan.source_kind & TAG_SOURCE_ORDER_MASK) >>
                 TAG_SOURCE_ORDER_SHIFT);
    u32_to_words(raw, order, words);

    if (source == TAG_SOURCE_MODBUS)
        result = rs485_master_write_holding(port, device,
                                            plan.source_address, 2U,
                                            words, POLL_TIMEOUT_MS);
    else if (source == TAG_SOURCE_TIT)
    {
        result = elam_modbus_write_register(plan.source_address, words[0]);
        if (result == RT_EOK)
            result = elam_modbus_write_register(
                (u16)(plan.source_address + 1U), words[1]);
    }
    else
        return TAG_ERR_STATE;

    return result == RT_EOK ? TAG_OK : TAG_ERR_STATE;
}

static void poll_one(u16 index, const tag_poll_plan_t *plan)
{
    tag_id_t id = index_tag_id(index);
    tag_value_t current;
    u16 words[2];
    u16 count = (plan->type == TAG_TYPE_FLOAT32 ||
                 plan->type == TAG_TYPE_UINT32 ||
                 plan->type == TAG_TYPE_INT32) ? 2U : 1U;

    /* Writable tags are outputs.  Their value is owned by tag_write()/Lua
     * and must not be replaced by a periodic readback of a stale device
     * register.  tag_set_float() still sends the physical write below via
     * tag_poll_scheduler_write_float(). */
    if (tag_read(id, &current) == TAG_OK &&
        (current.flags & TAG_FLAG_WRITABLE) != 0U)
        return;

    if (read_source(id, plan, words, count) == RT_EOK)
    {
        if (tag_publish(id, plan->type,
                        convert_value(plan, words)) == TAG_OK)
            tag_poll_success_count++;
        else
            tag_poll_error_count++;
    }
    else
    {
        (void)tag_set_valid(id, 0U);
        tag_poll_error_count++;
    }
}

static void scheduler_thread(void *parameter)
{
    (void)parameter;

    while (1)
    {
        tag_poll_plan_t plan;
        u16 selected = TAG_REGISTRY_CAPACITY;
        u16 checked;
        u32 now = (u32)rt_tick_get();

        rt_mutex_take(&plans_lock, RT_WAITING_FOREVER);
        for (checked = 0U; checked < TAG_REGISTRY_CAPACITY; checked++)
        {
            u16 index = (u16)((poll_cursor + checked) %
                              TAG_REGISTRY_CAPACITY);
            if (plans[index].active &&
                (s32)(now - plans[index].next_due) >= 0)
            {
                selected = index;
                plan = plans[index];
                plans[index].next_due = now +
                    (u32)((plan.poll_ms * RT_TICK_PER_SECOND + 999U) /
                          1000U);
                poll_cursor = (u16)((index + 1U) %
                                    TAG_REGISTRY_CAPACITY);
                break;
            }
        }
        rt_mutex_release(&plans_lock);

        if (selected < TAG_REGISTRY_CAPACITY)
            poll_one(selected, &plan);
        else
            rt_thread_mdelay(POLL_IDLE_MS);
    }
}

int tag_poll_scheduler_apply_device(
    u8 port, u8 device,
    const tag_device_config_record_t *records, u8 count)
{
    u8 sensor;
    u8 index;
    u16 base;
    u32 now;

    if (port >= TAG_PORT_COUNT || device == 0U ||
        device > TAG_DEVICE_COUNT ||
        (!records && count != 0U) ||
        count > TAG_DEVICE_CONFIG_MAX_TAGS)
        return TAG_ERR_PARAM;

    base = plan_index(port, device, 1U);
    now = (u32)rt_tick_get();
    rt_mutex_take(&plans_lock, RT_WAITING_FOREVER);
    memset(&plans[base], 0,
           TAG_DEVICE_TAG_COUNT * sizeof(plans[0]));
    for (index = 0U; index < count; index++)
    {
        const tag_device_config_record_t *record = &records[index];
        tag_poll_plan_t *plan;
        sensor = record->sensor;
        if (sensor == 0U || sensor > TAG_DEVICE_TAG_COUNT)
            continue;
        plan = &plans[plan_index(port, device, sensor)];
        plan->source_address = record->source_address;
        plan->poll_ms = record->poll_ms;
        plan->scale_bits = record->scale_bits;
        plan->offset_bits = record->offset_bits;
        plan->next_due = now;
        plan->type = record->type;
        plan->source_kind = record->source_kind;
        plan->active =
            ((record->source_kind & TAG_SOURCE_KIND_MASK) !=
             TAG_SOURCE_NONE && record->poll_ms != 0U) ? 1U : 0U;
    }
    rt_mutex_release(&plans_lock);
    return TAG_OK;
}

int tag_poll_scheduler_init(void)
{
    memset(plans, 0, sizeof(plans));
    tag_poll_success_count = 0U;
    tag_poll_error_count = 0U;
    poll_cursor = 0U;
    if (rt_mutex_init(&plans_lock, "tag_poll",
                      RT_IPC_FLAG_PRIO) != RT_EOK)
        return -RT_ERROR;
    poll_thread = rt_thread_create("tag_poll", scheduler_thread,
                                   RT_NULL, POLL_THREAD_STACK,
                                   POLL_THREAD_PRIO, 10U);
    if (!poll_thread)
        return -RT_ENOMEM;
    return rt_thread_startup(poll_thread);
}
