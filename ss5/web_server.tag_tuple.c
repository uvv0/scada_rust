#include "web_server.h"
#include "elam_modbus.h"
#include "modules/qspi_objects.h"
#include "modules/qspi_modules.h"
#include "tag_registry.h"
#include "arx/arx_example.h"
#include "../libraries/mongoose/mongoose.h"
#include <string.h>

#define DBG_TAG "web"
#define DBG_LVL DBG_INFO
#include <rtdbg.h>

#define WEB_THREAD_STACK_SIZE 4096
#define WEB_THREAD_PRIORITY   25
#define WEB_THREAD_TIMESLICE  10
#define WEB_START_DELAY_MS    5000
#define WEB_LUA_VM_OBJECT_ID  5UL
#define WEB_LUA_MAX_SIZE      (16UL * 1024UL)

/* 0: not started, 1: delay, 2: manager ready, 3: listening, -1: error. */
volatile int web_server_state;
volatile rt_uint32_t web_accept_count;
volatile rt_uint32_t web_http_count;
volatile rt_uint32_t web_error_count;
volatile rt_uint32_t web_close_count;
volatile rt_uint32_t web_qspi_hit_count;

/*
 * const keeps the complete initial web package in internal readonly Flash.
 * It is not copied to RAM and does not use the external W25Q/QSPI slots.
 */
static const char s_index_html[] =
    "<!doctype html><html lang=\"en\"><head>"
    "<meta charset=\"utf-8\"><meta name=\"viewport\" "
    "content=\"width=device-width,initial-scale=1\">"
    "<title>STM32H750</title><style>"
    "body{font:16px system-ui;margin:2rem;max-width:48rem}"
    "h1{color:#17365d}.ok{color:#087830}code{background:#eee;padding:.2rem}"
    "</style></head><body><h1>STM32H750 controller</h1>"
    "<p class=\"ok\">Mongoose web server is running.</p>"
    "<p>Firmware API: <code>/api/status</code></p>"
    "<p><a href=\"/tags\">Текущие значения тегов</a></p>"
    "<p><a href=\"/slot1\">Slot 1 float monitor</a></p>"
    "<p><a href=\"/lua\">Lua editor</a></p>"
    "</body></html>";

/*
 * The browser converts the two IEEE-754 words to float.  This keeps floating
 * point printf code out of internal Flash and preserves the exact bit pattern
 * published by QSPI module slot 1 in TIT[2512..2513].
 */
static const char s_slot1_html[] =
    "<!doctype html><html lang=\"ru\"><head>"
    "<meta charset=\"utf-8\"><meta name=\"viewport\" "
    "content=\"width=device-width,initial-scale=1\">"
    "<title>Slot 1 float</title><style>"
    "body{font:16px system-ui;margin:2rem;max-width:48rem;background:#f5f7fa}"
    ".card{background:#fff;border-radius:12px;padding:1.5rem;"
    "box-shadow:0 2px 12px #0002}.value{font-size:3rem;font-weight:700;"
    "color:#17365d}.ok{color:#087830}.err{color:#b02020}"
    "small,code{color:#59636e}</style></head><body>"
    "<div class=\"card\"><h1>Float слота 1</h1>"
    "<div id=\"value\" class=\"value\">—</div>"
    "<p id=\"state\">Ожидание данных…</p>"
    "<p><small>TIT[2512..2513], обновление каждую секунду</small></p>"
    "<p><a href=\"/\">На главную</a></p></div><script>"
    "function f32(h,l){const b=new ArrayBuffer(4),v=new DataView(b);"
    "v.setUint32(0,((h<<16)>>>0)|l,false);return v.getFloat32(0,false)}"
    "async function poll(){try{const r=await fetch('/api/slot1',{cache:'no-store'});"
    "if(!r.ok)throw Error(r.status);const d=await r.json(),x=f32(d.high,d.low);"
    "document.getElementById('value').textContent=Number.isFinite(x)?"
    "x.toPrecision(7):String(x);const s=document.getElementById('state');"
    "s.className=d.comm_status===0?'ok':'err';s.textContent="
    "'Связь: '+(d.comm_status===0?'OK':'ошибка '+d.comm_status)+"
    "'; успешных чтений: '+d.success_count}catch(e){"
    "const s=document.getElementById('state');s.className='err';"
    "s.textContent='Ошибка HTTP: '+e}}poll();setInterval(poll,1000);"
    "</script></body></html>";

static u32 web_crc32_update(u32 crc, const u8 *data, u32 size)
{
    u8 bit;
    while (size--)
    {
        crc ^= *data++;
        for (bit = 0U; bit < 8U; bit++)
            crc = (crc >> 1U) ^ ((crc & 1U) ? 0xEDB88320UL : 0U);
    }
    return crc;
}

static u32 web_crc32(const u8 *data, u32 size)
{
    return web_crc32_update(0xFFFFFFFFUL, data, size) ^ 0xFFFFFFFFUL;
}

static rt_bool_t web_method_is(const struct mg_http_message *request,
                               const char *method, u16 length)
{
    return request->method.len == length &&
           memcmp(request->method.buf, method, length) == 0;
}

static int web_query_u32(const struct mg_http_message *request,
                         const char *name, u32 *value)
{
    char input[12];
    u32 parsed = 0U;
    int length = mg_http_get_var(&request->query, name, input, sizeof(input));
    int index;

    if (length <= 0)
        return -1;
    for (index = 0; index < length; index++)
    {
        if (input[index] < '0' || input[index] > '9' ||
            parsed > (0xFFFFFFFFUL - (u32)(input[index] - '0')) / 10UL)
            return -1;
        parsed = parsed * 10UL + (u32)(input[index] - '0');
    }
    if (parsed == 0U)
        return -1;
    *value = parsed;
    return 0;
}

static void web_send_lua_script(struct mg_connection *connection,
                                const struct mg_http_message *request)
{
    qspi_object_record_t record;
    const u8 *payload;
    u32 object_id;
    int result;

    if (web_query_u32(request, "id", &object_id) != 0)
    {
        mg_http_reply(connection, 400, "", "missing script id\n");
        return;
    }
    result = qspi_object_find(object_id, &record);
    if (result != QSPI_OBJECT_OK ||
        record.header.object_type != QSPI_OBJECT_LUA_SCRIPT)
    {
        mg_http_reply(connection, 404, "", "script not found\n");
        return;
    }
    payload = (const u8 *)(QSPI_BLOCK_XIP_ADDRESS(record.first_block) +
                           QSPI_OBJECT_HEADER_SIZE);
    mg_printf(connection, "HTTP/1.1 200 OK\r\nContent-Type: text/plain; "
              "charset=utf-8\r\nContent-Length: %lu\r\nCache-Control: "
              "no-store\r\nConnection: close\r\n\r\n",
              (unsigned long)record.header.payload_size);
    (void)mg_send(connection, payload, record.header.payload_size);
    connection->is_draining = 1;
}

static int web_write_lua_script(const struct mg_http_message *request,
                                u32 *generation)
{
    qspi_object_header_t header;
    qspi_object_record_t current;
    qspi_object_commit_info_t info;
    u32 object_id, image_crc, running, written, offset, piece;
    int result;
    char name[sizeof(header.name)];

    if (web_query_u32(request, "id", &object_id) != 0 ||
        request->body.len == 0U || request->body.len > WEB_LUA_MAX_SIZE)
        return QSPI_OBJECT_ERR_PARAM;
    memset(&header, 0, sizeof(header));
    result = qspi_object_find(object_id, &current);
    if (result == QSPI_OBJECT_OK)
    {
        if (current.header.object_type != QSPI_OBJECT_LUA_SCRIPT)
            return QSPI_OBJECT_ERR_TYPE;
        header.generation = current.header.generation + 1U;
    }
    else if (result == QSPI_OBJECT_ERR_NOT_FOUND)
        header.generation = 1U;
    else
        return result;

    memset(name, 0, sizeof(name));
    if (mg_http_get_var(&request->query, "name", name, sizeof(name)) <= 0)
        memcpy(name, "web.lua", 8U);
    name[sizeof(name) - 1U] = '\0';
    header.magic = QSPI_OBJECT_MAGIC;
    header.format_version = QSPI_OBJECT_FORMAT_VERSION;
    header.header_size = QSPI_OBJECT_HEADER_SIZE;
    header.object_type = QSPI_OBJECT_LUA_SCRIPT;
    header.flags = QSPI_OBJECT_FLAG_VALID;
    header.object_id = object_id;
    header.payload_size = (u32)request->body.len;
    header.payload_crc32 = web_crc32((const u8 *)request->body.buf,
                                     header.payload_size);
    memcpy(header.name, name, sizeof(header.name));
    header.header_crc32 = web_crc32((const u8 *)&header,
                                    sizeof(header) - sizeof(u32));
    running = web_crc32_update(0xFFFFFFFFUL, (const u8 *)&header,
                               sizeof(header));
    running = web_crc32_update(running, (const u8 *)request->body.buf,
                               header.payload_size);
    image_crc = running ^ 0xFFFFFFFFUL;

    (void)module_object_stop_if_active(WEB_LUA_VM_OBJECT_ID);
    result = qspi_object_upload_begin(sizeof(header) + header.payload_size,
                                      image_crc);
    if (result == QSPI_OBJECT_OK)
        result = qspi_object_upload_chunk(0U, (const u8 *)&header,
                                          sizeof(header), &written, &running);
    offset = 0U;
    while (result == QSPI_OBJECT_OK && offset < header.payload_size)
    {
        piece = header.payload_size - offset;
        if (piece > QSPI_OBJECT_BLOCK_SIZE)
            piece = QSPI_OBJECT_BLOCK_SIZE;
        result = qspi_object_upload_chunk(sizeof(header) + offset,
            (const u8 *)request->body.buf + offset, (u16)piece,
            &written, &running);
        offset += piece;
    }
    if (result == QSPI_OBJECT_OK)
        result = qspi_object_upload_commit(&info);
    if (result != QSPI_OBJECT_OK)
        qspi_object_upload_abort();
    else
        *generation = header.generation;
    return result;
}

static void web_lua_status(struct mg_connection *connection)
{
    qspi_object_module_status_t status;
    int result = module_object_get_status(WEB_LUA_VM_OBJECT_ID, &status);
    mg_http_reply(connection, result == QSPI_OBJECT_OK ? 200 : 404,
                  "Content-Type: application/json\r\nCache-Control: no-store\r\n",
                  "{\"result\":%d,\"vm_id\":%lu,\"active\":%u,"
                  "\"generation\":%lu,\"last_result\":%ld}\n",
                  result, (unsigned long)WEB_LUA_VM_OBJECT_ID,
                  result == QSPI_OBJECT_OK ? status.active : 0U,
                  result == QSPI_OBJECT_OK ? (unsigned long)status.generation : 0UL,
                  result == QSPI_OBJECT_OK ? (long)status.last_result : 0L);
}

static const char *web_content_type_name(u16 content_type)
{
    switch (content_type)
    {
    case 1U: return "text/html; charset=utf-8";
    case 2U: return "text/css; charset=utf-8";
    case 3U: return "application/javascript; charset=utf-8";
    case 4U: return "application/json";
    case 5U: return "image/png";
    case 6U: return "image/jpeg";
    case 7U: return "image/svg+xml";
    case 9U: return "image/x-icon";
    default: return "text/plain; charset=utf-8";
    }
}

static rt_bool_t web_send_qspi_object(struct mg_connection *connection,
                                      const struct mg_str *uri)
{
    const u8 *data;
    u32 size;
    u16 content_type;

    if (qspi_object_find_web(uri->buf, (u32)uri->len,
                             &data, &size, &content_type,
                             RT_NULL) != QSPI_OBJECT_OK)
        return RT_FALSE;

    mg_printf(connection,
              "HTTP/1.1 200 OK\r\n"
              "Content-Type: %s\r\n"
              "Content-Length: %lu\r\n"
              "Cache-Control: no-cache\r\n"
              "Connection: close\r\n\r\n",
              web_content_type_name(content_type), (unsigned long)size);
    if (!mg_send(connection, data, size))
    {
        web_error_count++;
        return RT_FALSE;
    }
    connection->is_draining = 1;
    web_qspi_hit_count++;
    return RT_TRUE;
}

static void web_json_string(char *output, u16 capacity, const char *input)
{
    static const char hex[] = "0123456789abcdef";
    u16 used = 0U;

    if (capacity < 3U)
        return;
    output[used++] = '"';
    while (*input && used + 2U < capacity)
    {
        u8 value = (u8)*input++;
        if (value == '"' || value == '\\')
        {
            if (used + 3U >= capacity)
                break;
            output[used++] = '\\';
            output[used++] = (char)value;
        }
        else if (value < 0x20U)
        {
            if (used + 7U >= capacity)
                break;
            output[used++] = '\\';
            output[used++] = 'u';
            output[used++] = '0';
            output[used++] = '0';
            output[used++] = hex[value >> 4U];
            output[used++] = hex[value & 0x0FU];
        }
        else
            output[used++] = (char)value;
    }
    output[used++] = '"';
    output[used] = '\0';
}

static void web_send_tags(struct mg_connection *connection)
{
    tag_snapshot_t items[16];
    u16 total = tag_count();
    u16 offset = 0U;
    rt_bool_t first = RT_TRUE;

    mg_printf(connection,
              "HTTP/1.1 200 OK\r\n"
              "Content-Type: application/json\r\n"
              "Cache-Control: no-store\r\n"
              "Connection: close\r\n\r\n"
              "{\"count\":%u,\"tags\":[",
              (unsigned int)total);
    while (offset < total)
    {
        u16 copied = 0U;
        u16 index;

        if (tag_snapshot(offset, items, 16U, &copied) != TAG_OK ||
            copied == 0U)
            break;
        for (index = 0U; index < copied; index++)
        {
            const tag_value_t *value = &items[index].value;
            tag_info_t info;
            char name_json[TAG_NAME_SIZE * 6U + 3U];
            char unit_json[TAG_UNIT_SIZE * 6U + 3U];

            if (tag_get_info(value->id, &info) == TAG_OK)
            {
                web_json_string(name_json, sizeof(name_json), info.name);
                web_json_string(unit_json, sizeof(unit_json), info.unit);
            }
            else
            {
                name_json[0] = '"'; name_json[1] = '"';
                name_json[2] = '\0';
                unit_json[0] = '"'; unit_json[1] = '"';
                unit_json[2] = '\0';
            }
            mg_printf(connection,
                      "%s{\"key\":%u,\"port\":%u,\"device\":%u,"
                      "\"id\":%u,\"type\":%u,\"flags\":%u,"
                      "\"value_bits\":%lu,\"updated_at\":%lu,"
                      "\"name\":%s,\"unit\":%s}",
                      first ? "" : ",",
                      (unsigned int)value->id,
                      (unsigned int)(TAG_PORT(value->id) + 1U),
                      (unsigned int)TAG_DEVICE(value->id),
                      (unsigned int)TAG_SENSOR(value->id),
                      (unsigned int)value->type,
                      (unsigned int)value->flags,
                      (unsigned long)value->value_bits,
                      (unsigned long)items[index].updated_at,
                      name_json, unit_json);
            first = RT_FALSE;
        }
        offset = (u16)(offset + copied);
    }
    mg_printf(connection, "]}\n");
    connection->is_draining = 1;
}

static void web_event_handler(struct mg_connection *connection, int event,
                              void *event_data)
{
    if (event == MG_EV_ACCEPT)
        web_accept_count++;
    else if (event == MG_EV_ERROR)
        web_error_count++;
    else if (event == MG_EV_CLOSE)
        web_close_count++;

    if (event == MG_EV_HTTP_MSG)
    {
        struct mg_http_message *request = (struct mg_http_message *) event_data;
        web_http_count++;

        if (mg_match(request->uri, mg_str("/api/lua/script"), RT_NULL) &&
            web_method_is(request, "GET", 3U))
        {
            web_send_lua_script(connection, request);
        }
        else if (mg_match(request->uri, mg_str("/api/lua/script"), RT_NULL) &&
                 web_method_is(request, "PUT", 3U))
        {
            u32 generation = 0U;
            int result = web_write_lua_script(request, &generation);
            mg_http_reply(connection, result == QSPI_OBJECT_OK ? 200 : 400,
                          "Content-Type: application/json\r\n",
                          "{\"result\":%d,\"generation\":%lu}\n",
                          result, (unsigned long)generation);
        }
        else if (mg_match(request->uri, mg_str("/api/lua/run"), RT_NULL) &&
                 web_method_is(request, "POST", 4U))
        {
            int result = module_object_stop_if_active(WEB_LUA_VM_OBJECT_ID);
            if (result == MODULE_OK)
                result = module_object_start(WEB_LUA_VM_OBJECT_ID);
            mg_http_reply(connection, result == QSPI_OBJECT_OK ? 200 : 400,
                          "Content-Type: application/json\r\n",
                          "{\"result\":%d,\"vm_id\":%lu}\n", result,
                          (unsigned long)WEB_LUA_VM_OBJECT_ID);
        }
        else if (mg_match(request->uri, mg_str("/api/lua/status"), RT_NULL))
        {
            web_lua_status(connection);
        }
        else if (mg_match(request->uri, mg_str("/api/status"), RT_NULL))
        {
            mg_http_reply(connection, 200,
                          "Content-Type: application/json\r\n"
                          "Cache-Control: no-store\r\n",
                          "{\"ok\":true,\"uptime_ms\":%llu}\n",
                          mg_millis());
        }
        else if (mg_match(request->uri, mg_str("/api/tags"), RT_NULL))
        {
            web_send_tags(connection);
        }
        else if (mg_match(request->uri, mg_str("/api/slot1"), RT_NULL))
        {
            rt_uint16_t high = 0U, low = 0U, status = 0U, success = 0U;

            (void) elam_modbus_read_register(2512U, &high);
            (void) elam_modbus_read_register(2513U, &low);
            (void) elam_modbus_read_register(2522U, &status);
            (void) elam_modbus_read_register(2523U, &success);
            mg_http_reply(connection, 200,
                          "Content-Type: application/json\r\n"
                          "Cache-Control: no-store\r\n",
                          "{\"high\":%u,\"low\":%u,\"comm_status\":%d,"
                          "\"success_count\":%u}\n",
                          (unsigned int) high, (unsigned int) low,
                          (int) (rt_int16_t) status,
                          (unsigned int) success);
        }
        else if (mg_match(request->uri,
                          mg_str("/api/slot1/history"), RT_NULL))
        {
            arx_slot1_sample_t samples[60];
            rt_uint16_t count = arx_example_slot1_recent(samples, 60U);
            rt_uint16_t i;

            mg_printf(connection,
                      "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n"
                      "Cache-Control: no-store\r\nConnection: close\r\n\r\n"
                      "{\"points\":[");
            for (i = 0U; i < count; i++)
                mg_printf(connection, "%s[%lu,%lu]", i ? "," : "",
                          (unsigned long)samples[i].timestamp,
                          (unsigned long)samples[i].value_bits);
            mg_printf(connection, "]}\n");
            connection->is_draining = 1;
        }
        else if (mg_match(request->uri, mg_str("/health"), RT_NULL))
        {
            mg_http_reply(connection, 200, "Content-Type: text/plain\r\n",
                          "ok\n");
        }
        else if (web_send_qspi_object(connection, &request->uri))
        {
            /* Uploaded QSPI page was sent. */
        }
        else if (mg_match(request->uri, mg_str("/slot1"), RT_NULL))
        {
            mg_http_reply(connection, 200,
                          "Content-Type: text/html; charset=utf-8\r\n"
                          "Cache-Control: no-cache\r\n",
                          "%s", s_slot1_html);
        }
        else if (mg_match(request->uri, mg_str("/"), RT_NULL))
        {
            mg_http_reply(connection, 200,
                          "Content-Type: text/html; charset=utf-8\r\n"
                          "Cache-Control: no-cache\r\n",
                          "%s", s_index_html);
        }
        else
        {
            mg_http_reply(connection, 404, "Content-Type: text/plain\r\n",
                          "not found\n");
        }
    }
}

static void web_thread_entry(void *parameter)
{
    struct mg_mgr manager;

    (void) parameter;

    /*
     * UART8/Modbus is a critical interface and is started before this thread.
     * Do not enter the lwIP socket API during early board/network
     * initialization.  The lower thread priority also guarantees that ELAM
     * processing (priority 15) wins over HTTP work (priority 25).
     */
    web_server_state = 1;
    rt_thread_mdelay(WEB_START_DELAY_MS);

    mg_mgr_init(&manager);
    web_server_state = 2;

    if (mg_http_listen(&manager, "http://0.0.0.0:80",
                       web_event_handler, RT_NULL) == RT_NULL)
    {
        web_server_state = -1;
        LOG_E("cannot listen on TCP port 80");
        mg_mgr_free(&manager);
        return;
    }

    web_server_state = 3;
    LOG_I("Mongoose %s listening on http://0.0.0.0:80", MG_VERSION);
    for (;;)
        mg_mgr_poll(&manager, 20);
}

rt_err_t web_server_start(void)
{
    rt_thread_t thread = rt_thread_create("mongoose", web_thread_entry, RT_NULL,
                                          WEB_THREAD_STACK_SIZE,
                                          WEB_THREAD_PRIORITY,
                                          WEB_THREAD_TIMESLICE);
    if (thread == RT_NULL)
        return -RT_ENOMEM;

    return rt_thread_startup(thread);
}
