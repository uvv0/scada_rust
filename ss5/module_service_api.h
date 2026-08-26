#ifndef MODULE_SERVICE_API_H
#define MODULE_SERVICE_API_H

#include "../types.h"
#include "../tag_registry.h"

#define MODULE_SERVICE_API_ADDRESS  0x2407F000UL
#define MODULE_SERVICE_API_MAGIC    0x4950414DUL
#define MODULE_SERVICE_API_VERSION  5U

#define MODULE_LUA_SLOT_COUNT       32U
#define MODULE_LUA_STATUS_MB_BASE   32000U
#define MODULE_LUA_STATUS_MB_WORDS  (4U + MODULE_LUA_SLOT_COUNT * 3U)

#define MODULE_LUA_SLOT_EMPTY       0U
#define MODULE_LUA_SLOT_RUNNING     1U
#define MODULE_LUA_SLOT_COMPLETE    2U
#define MODULE_LUA_SLOT_ERROR       3U

#define MODULE_HOLDING_FLOAT_ORDER_BASE  2550U
#define MODULE_HOLDING_FLOAT_ORDER_COUNT 20U

#define MODULE_FLOAT_ORDER_ABCD 0U
#define MODULE_FLOAT_ORDER_CDAB 1U
#define MODULE_FLOAT_ORDER_BADC 2U
#define MODULE_FLOAT_ORDER_DCBA 3U

#pragma pack(push, 1)
typedef struct
{
    u32 object_id;
    u32 generation;
    u32 payload_address;
    u32 payload_size;
    u16 object_type;
    u16 flags;
    char name[40];
} module_object_info_t;
#pragma pack(pop)

typedef struct
{
    u32 magic;
    u16 version;
    u16 size;

    s32 (*rtu7_read_holding)(u8 slave, u16 address, u16 count,
                             u16 *values, u32 timeout_ms);
    s32 (*tit_read)(u16 address, u16 *value);
    s32 (*tit_write)(u16 address, u16 value);
    u32 (*timestamp_get)(void);
    void (*delay_ms)(u32 milliseconds);
    s32 (*archive_tii_append)(u32 timestamp,
                              const u16 *values,
                              u16 count);
    s32 (*rtu2_read_holding)(u8 slave, u16 address, u16 count,
                             u16 *values, u32 timeout_ms);
    s32 (*holding_read)(u16 address, u16 *values, u16 count);
    s32 (*profiler_start)(void);
    s32 (*profiler_stop)(void);
    s32 (*profiler_snapshot_begin)(u8 *count, unsigned long long *total_cycles,
                                   u32 *cycles_per_us);
    s32 (*profiler_snapshot_item)(u8 index, void *item);
    s32 (*profiler_publish)(u8 index, const void *item,
                            u16 load_x100, u32 window_total_us);
    s32 (*tag_read)(tag_id_t id, tag_value_t *value);
    s32 (*tag_write)(tag_id_t id, const tag_value_t *value);
    s32 (*tag_get_float)(tag_id_t id, float *value);
    s32 (*tag_set_float)(tag_id_t id, float value);
    s32 (*tag_set_valid)(tag_id_t id, u8 valid);
    s32 (*tag_get_info)(tag_id_t id, tag_info_t *info);
    s32 (*tag_find)(const char *name, tag_id_t *id);
    void *(*memory_realloc)(void *pointer, u32 size);
    void (*memory_free)(void *pointer);
    u32 (*tick_get)(void);
    u32 (*tick_per_second)(void);
    void (*log_write)(const char *text);
    s32 (*object_find_type)(u16 object_type, u16 ordinal,
                            module_object_info_t *info);
    void (*lua_status_reset)(void);
    void (*lua_status_update)(u8 slot, u8 state, s32 result);
} module_service_api_t;

void module_lua_status_reset(void);
void module_lua_status_update(u8 slot, u8 state, s32 result);
s32 module_lua_status_read(u16 offset, u16 *values, u16 count);

/*
 * Настраивает UART7 как прикладной Modbus RTU master 19200 8N1 и
 * публикует таблицу сервисов по фиксированному адресу 0x2407F000.
 */
int module_service_init(void);

#endif
