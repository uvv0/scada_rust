#include "../module_service_api.h"
#include "../qspi_object_format.h"
#include <string.h>
#include "src/lua.h"
#include "src/lauxlib.h"
#include "src/lualib.h"

#define LUA_MEMORY_LIMIT       (128U * 1024U)
#define LUA_HOOK_GRANULARITY   1000
#define LUA_INSTRUCTION_LIMIT  200000UL
#define LUA_TIME_LIMIT_MS      200U
#define LUA_SCHEDULE_LIMIT     32U
#define LUA_SCHEDULE_IDLE_MS   10U
#define LUA_PERIOD_MAX_MS      3600000UL

typedef struct
{
    u32 object_id;
    u32 generation;
    u32 period_ms;
    u32 next_due;
} lua_schedule_t;

typedef struct
{
    const module_service_api_t *api;
    u32 used;
    u32 instruction_count;
    u32 deadline;
} lua_module_context_t;

static lua_module_context_t *lua_context(lua_State *state)
{
    lua_module_context_t *context;

    lua_pushliteral(state, "_module_context");
    lua_rawget(state, LUA_REGISTRYINDEX);
    context = (lua_module_context_t *)lua_touserdata(state, -1);
    lua_pop(state, 1);
    return context;
}

static tag_id_t lua_tag_key(lua_State *state, int first_argument)
{
    lua_Integer port = luaL_checkinteger(state, first_argument);
    lua_Integer device = luaL_checkinteger(state, first_argument + 1);
    lua_Integer sensor = luaL_checkinteger(state, first_argument + 2);

    if (port < 1 || port > TAG_PORT_COUNT)
        luaL_argerror(state, first_argument, "port must be 1..5");
    if (device < 1 || device > TAG_DEVICE_COUNT)
        luaL_argerror(state, first_argument + 1, "device must be 1..30");
    if (sensor < 1 || sensor > TAG_DEVICE_TAG_COUNT)
        luaL_argerror(state, first_argument + 2, "tag id must be 1..30");
    return TAG_ID((u8)(port - 1), (u8)device, (u8)sensor);
}

static void *lua_limited_allocator(void *opaque, void *pointer,
                                   size_t old_size, size_t new_size)
{
    lua_module_context_t *context = (lua_module_context_t *)opaque;
    void *result;

    if (pointer == 0)
        old_size = 0U;
    if (new_size == 0U)
    {
        context->api->memory_free(pointer);
        context->used = old_size <= context->used ?
                        context->used - (u32)old_size : 0U;
        return 0;
    }
    if (new_size > old_size &&
        (u32)(new_size - old_size) > LUA_MEMORY_LIMIT - context->used)
        return 0;
    result = context->api->memory_realloc(pointer, (u32)new_size);
    if (result != 0)
    {
        context->used = context->used - (u32)old_size + (u32)new_size;
    }
    return result;
}

static int lua_tag_find(lua_State *state)
{
    lua_module_context_t *context = lua_context(state);
    tag_id_t id;
    const char *name = luaL_checkstring(state, 1);
    int result = context->api->tag_find(name, &id);

    if (result != 0)
    {
        lua_pushnil(state);
        lua_pushinteger(state, result);
        return 2;
    }
    lua_pushinteger(state, id);
    return 1;
}

static int lua_tag_id(lua_State *state)
{
    lua_pushinteger(state, lua_tag_key(state, 1));
    return 1;
}

static int lua_tag_get(lua_State *state)
{
    lua_module_context_t *context = lua_context(state);
    tag_value_t value;
    tag_id_t id = lua_tag_key(state, 1);
    int result = context->api->tag_read(id, &value);

    if (result != 0)
    {
        lua_pushnil(state);
        lua_pushinteger(state, result);
        return 2;
    }
    if (value.type == TAG_TYPE_FLOAT32)
    {
        float number;
        u32 bits = value.value_bits;
        memcpy(&number, &bits, sizeof(number));
        lua_pushnumber(state, number);
    }
    else if (value.type == TAG_TYPE_BOOL)
        lua_pushboolean(state, value.value_bits != 0U);
    else if (value.type == TAG_TYPE_INT16)
        lua_pushinteger(state, (s16)(value.value_bits & 0xFFFFU));
    else if (value.type == TAG_TYPE_UINT16)
        lua_pushinteger(state, value.value_bits & 0xFFFFU);
    else
        lua_pushinteger(state, (lua_Integer)value.value_bits);
    lua_pushboolean(state, (value.flags & TAG_FLAG_VALID) != 0U);
    return 2;
}

static int lua_tag_set(lua_State *state)
{
    lua_module_context_t *context = lua_context(state);
    tag_id_t id = lua_tag_key(state, 1);
    float number = (float)luaL_checknumber(state, 4);
    int result = context->api->tag_set_float(id, number);

    lua_pushboolean(state, result == 0);
    if (result != 0)
    {
        lua_pushinteger(state, result);
        return 2;
    }
    return 1;
}

static int lua_timer_sleep(lua_State *state)
{
    lua_module_context_t *context = lua_context(state);
    lua_Integer milliseconds = luaL_checkinteger(state, 1);

    if (milliseconds < 0)
        milliseconds = 0;
    if (milliseconds > 60000)
        milliseconds = 60000;
    context->api->delay_ms((u32)milliseconds);
    return 0;
}

static int lua_timer_now(lua_State *state)
{
    lua_module_context_t *context = lua_context(state);
    u32 ticks = context->api->tick_get();
    u32 frequency = context->api->tick_per_second();

    lua_pushinteger(state, frequency ?
                    (lua_Integer)(((unsigned long long)ticks * 1000U) /
                                  frequency) : 0);
    return 1;
}

static int lua_log_write(lua_State *state)
{
    lua_module_context_t *context = lua_context(state);
    const char *text = luaL_checkstring(state, 1);

    context->api->log_write(text);
    return 0;
}

static void lua_limit_hook(lua_State *state, lua_Debug *debug)
{
    lua_module_context_t *context = lua_context(state);
    u32 now;

    (void)debug;
    context->instruction_count += LUA_HOOK_GRANULARITY;
    now = context->api->tick_get();
    if (context->instruction_count > LUA_INSTRUCTION_LIMIT ||
        (s32)(now - context->deadline) >= 0)
        luaL_error(state, "execution limit");
}

static void lua_register_function(lua_State *state, const char *table,
                                  const char *name, lua_CFunction function)
{
    lua_getglobal(state, table);
    lua_pushcfunction(state, function);
    lua_setfield(state, -2, name);
    lua_pop(state, 1);
}

static void lua_open_safe_libraries(lua_State *state)
{
    luaL_requiref(state, "_G", luaopen_base, 1);
    lua_pop(state, 1);
    luaL_requiref(state, LUA_TABLIBNAME, luaopen_table, 1);
    lua_pop(state, 1);
    luaL_requiref(state, LUA_STRLIBNAME, luaopen_string, 1);
    lua_pop(state, 1);
    lua_newtable(state);
    lua_setglobal(state, "tag");
    lua_newtable(state);
    lua_setglobal(state, "timer");
    lua_newtable(state);
    lua_setglobal(state, "log");
    lua_register_function(state, "tag", "find", lua_tag_find);
    lua_register_function(state, "tag", "id", lua_tag_id);
    lua_register_function(state, "tag", "get", lua_tag_get);
    lua_register_function(state, "tag", "set", lua_tag_set);
    lua_register_function(state, "timer", "sleep", lua_timer_sleep);
    lua_register_function(state, "timer", "now", lua_timer_now);
    lua_register_function(state, "log", "write", lua_log_write);
}

static u32 lua_script_period(const char *payload, u32 size)
{
    static const char prefix[] = "-- period_ms=";
    u32 value = 0U;
    u32 start = 0U;
    u32 index;

    while (start + sizeof(prefix) - 1U < size && start < 128U)
    {
        if ((start == 0U || payload[start - 1U] == '\n') &&
            memcmp(payload + start, prefix, sizeof(prefix) - 1U) == 0)
            break;
        start++;
    }
    index = start + sizeof(prefix) - 1U;
    if (index >= size || start >= 128U)
        return 0U;
    while (index < size && payload[index] >= '0' && payload[index] <= '9')
    {
        value = value * 10U + (u32)(payload[index++] - '0');
        if (value > LUA_PERIOD_MAX_MS)
            return LUA_PERIOD_MAX_MS;
    }
    return value;
}

static int lua_execute_script(lua_State *state,
                              lua_module_context_t *context,
                              const module_object_info_t *script)
{
    const char *payload = (const char *)script->payload_address;
    u32 ticks = context->api->tick_get();
    u32 frequency = context->api->tick_per_second();
    int result;

    context->instruction_count = 0U;
    context->deadline = ticks +
        (frequency * LUA_TIME_LIMIT_MS + 999U) / 1000U;
    lua_sethook(state, lua_limit_hook, LUA_MASKCOUNT,
                LUA_HOOK_GRANULARITY);
    result = luaL_loadbufferx(state, payload, script->payload_size,
                              script->name, "t");
    if (result == LUA_OK)
        result = lua_pcall(state, 0, 0, 0);
    lua_sethook(state, 0, 0, 0);
    if (result != LUA_OK)
    {
        const char *error = lua_tostring(state, -1);
        context->api->log_write(error ? error : "unknown error");
        lua_pop(state, 1);
    }
    lua_gc(state, LUA_GCCOLLECT, 0);
    return result;
}

static u8 lua_script_slot(const module_object_info_t *script)
{
    u32 prefix = script->object_id & 0xFFFFFF00UL;
    u8 slot = (u8)script->object_id;
    return prefix == 0x4C530000UL && slot >= 1U &&
           slot <= LUA_SCHEDULE_LIMIT ? slot : 0U;
}

static void lua_run_scripts(lua_State *state, lua_module_context_t *context)
{
    lua_schedule_t schedules[LUA_SCHEDULE_LIMIT];
    u8 schedule_count = 0U;

    memset(schedules, 0, sizeof(schedules));
    for (;;)
    {
        module_object_info_t script;
        u16 ordinal = 0U;
        u8 periodic = 0U;
        u32 seen_mask = 0U;

        while (context->api->object_find_type(QSPI_OBJECT_LUA_SCRIPT,
                                               ordinal++, &script) == 0)
        {
            lua_schedule_t *schedule = 0;
            u32 now = context->api->tick_get();
            u32 frequency = context->api->tick_per_second();
            u32 period = lua_script_period(
                (const char *)script.payload_address, script.payload_size);
            u8 index;
            u8 slot = lua_script_slot(&script);

            if (slot != 0U)
                seen_mask |= 1UL << (slot - 1U);

            for (index = 0U; index < schedule_count; index++)
                if (schedules[index].object_id == script.object_id)
                {
                    schedule = &schedules[index];
                    break;
                }
            if (schedule == 0)
            {
                if (schedule_count >= LUA_SCHEDULE_LIMIT)
                {
                    context->api->log_write("too many Lua scripts");
                    break;
                }
                schedule = &schedules[schedule_count++];
                schedule->object_id = script.object_id;
                schedule->generation = script.generation;
                schedule->period_ms = period;
                schedule->next_due = now;
            }
            else if (schedule->generation != script.generation)
            {
                schedule->generation = script.generation;
                schedule->period_ms = period;
                schedule->next_due = now;
            }
            if (schedule->period_ms != 0U)
                periodic = 1U;
            if (!(schedule->period_ms == 0U &&
                  schedule->next_due == 0xFFFFFFFFUL) &&
                (s32)(now - schedule->next_due) >= 0)
            {
                int execute_result;
                if (slot != 0U)
                    context->api->lua_status_update(
                        slot, MODULE_LUA_SLOT_RUNNING, 0);
                execute_result = lua_execute_script(state, context, &script);
                if (slot != 0U)
                    context->api->lua_status_update(
                        slot,
                        execute_result == LUA_OK ?
                            (schedule->period_ms != 0U ?
                                MODULE_LUA_SLOT_RUNNING :
                                MODULE_LUA_SLOT_COMPLETE) :
                            MODULE_LUA_SLOT_ERROR,
                        execute_result);
                if (schedule->period_ms == 0U)
                    schedule->next_due = 0xFFFFFFFFUL;
                else
                    schedule->next_due = now + (u32)
                        (((unsigned long long)frequency *
                          schedule->period_ms + 999U) / 1000U);
            }
        }
        for (ordinal = 1U; ordinal <= LUA_SCHEDULE_LIMIT; ordinal++)
            if ((seen_mask & (1UL << (ordinal - 1U))) == 0U)
                context->api->lua_status_update(
                    (u8)ordinal, MODULE_LUA_SLOT_EMPTY, 0);
        if (!periodic)
            break;
        context->api->delay_ms(LUA_SCHEDULE_IDLE_MS);
    }
}

void lua_vm_run(const module_service_api_t *api)
{
    lua_module_context_t context;
    lua_State *state;

    if (api->magic != MODULE_SERVICE_API_MAGIC ||
        api->version != MODULE_SERVICE_API_VERSION ||
        api->size < sizeof(module_service_api_t) ||
        api->tag_read == 0 || api->tag_set_float == 0 ||
        api->memory_realloc == 0 || api->memory_free == 0 ||
        api->tick_get == 0 || api->tick_per_second == 0 ||
        api->log_write == 0 || api->object_find_type == 0)
        return;

    if (api->lua_status_reset == 0 || api->lua_status_update == 0)
        return;

    memset(&context, 0, sizeof(context));
    context.api = api;
    api->lua_status_reset();
    state = lua_newstate(lua_limited_allocator, &context);
    if (state == 0)
    {
        api->log_write("cannot create state");
        return;
    }
    lua_pushliteral(state, "_module_context");
    lua_pushlightuserdata(state, &context);
    lua_rawset(state, LUA_REGISTRYINDEX);
    lua_open_safe_libraries(state);
    lua_run_scripts(state, &context);
    lua_close(state);
}
