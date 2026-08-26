#include "web_server.h"
#include "elam_modbus.h"
#include "web_slots.h"
#include "arx/arx_example.h"
#include "../libraries/mongoose/mongoose.h"

#define DBG_TAG "web"
#define DBG_LVL DBG_INFO
#include <rtdbg.h>

#define WEB_THREAD_STACK_SIZE 4096
#define WEB_THREAD_PRIORITY   25
#define WEB_THREAD_TIMESLICE  10
#define WEB_START_DELAY_MS    5000

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
    "<p><a href=\"/slot1\">Slot 1 float monitor</a></p>"
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

static const char s_slot1_graph_html[] =
    "<!doctype html><meta charset=utf-8><meta name=viewport "
    "content=\"width=device-width,initial-scale=1\"><title>Slot 1 history</title>"
    "<style>body{font:16px system-ui;margin:2rem}canvas{width:100%;height:360px;"
    "border:1px solid #bbb}</style><h1>Slot 1: one point per minute</h1>"
    "<canvas id=c width=1000 height=360></canvas><p id=s></p><script>"
    "async function draw(){let d=await(await fetch('/api/slot1/history')).json(),"
    "a=d.points.reverse(),c=document.querySelector('#c'),g=c.getContext('2d');"
    "g.clearRect(0,0,c.width,c.height);document.querySelector('#s').textContent="
    "a.length+' points (ring archive: 1000)';if(a.length<2)return;"
    "let v=a.map(x=>x[1]),n=Math.min(...v),m=Math.max(...v),r=m-n||1;"
    "g.beginPath();g.strokeStyle='#1769aa';g.lineWidth=2;a.forEach((p,i)=>{"
    "let x=5+i*990/(a.length-1),y=355-(p[1]-n)*350/r;"
    "i?g.lineTo(x,y):g.moveTo(x,y)});g.stroke()}draw();setInterval(draw,60000)"
    "</script>";

static rt_bool_t web_send_qspi_slot(struct mg_connection *connection,
                                    const struct mg_str *uri)
{
    const u8 *data;
    u32 size;
    u16 content_type;

    if (web_slot_find(uri->buf, (u32)uri->len,
                      &data, &size, &content_type) != WEB_SLOT_OK)
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

        if (mg_match(request->uri, mg_str("/api/status"), RT_NULL))
        {
            mg_http_reply(connection, 200,
                          "Content-Type: application/json\r\n"
                          "Cache-Control: no-store\r\n",
                          "{\"ok\":true,\"uptime_ms\":%llu}\n",
                          mg_millis());
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
                mg_printf(connection, "%s[%lu,%.7g]", i ? "," : "",
                          (unsigned long)samples[i].timestamp,
                          (double)samples[i].value);
            mg_printf(connection, "]}\n");
            connection->is_draining = 1;
        }
        else if (mg_match(request->uri, mg_str("/health"), RT_NULL))
        {
            mg_http_reply(connection, 200, "Content-Type: text/plain\r\n",
                          "ok\n");
        }
        else if (web_send_qspi_slot(connection, &request->uri))
        {
            /* Uploaded QSPI page was sent. */
        }
        else if (mg_match(request->uri, mg_str("/slot1/graph"), RT_NULL))
        {
            mg_http_reply(connection, 200,
                          "Content-Type: text/html; charset=utf-8\r\n"
                          "Cache-Control: no-cache\r\n",
                          "%s", s_slot1_graph_html);
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
