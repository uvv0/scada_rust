#include "elam_modbus.h"
#include "thread_profiler.h"
#include "arx/arx_flashdb.h"
#include "arx/arx_modbus.h"
#include "arx/holding_flashdb.h"
#include "modules/module_modbus.h"
#include "modules/module_service_api.h"
#include "tag_config_modbus.h"
#include "tag_registry.h"

#include <rtdevice.h>
#include <string.h>

#define ELAM_PACKET_MAX      1200U
#define ELAM_REQUEST_MAX     17U
#define ELAM_STANDARD_READ_MAX 125U
#define ELAM_EXTENDED_READ_MAX 591U
#define TAG_VALUE_MB_BASE      14000U
#define TAG_VALUE_MB_WORDS     (TAG_PORT_COUNT * TAG_DEVICE_COUNT * TAG_DEVICE_TAG_COUNT * 4U)
#define TAG_VALUE_MB_END       (TAG_VALUE_MB_BASE + TAG_VALUE_MB_WORDS)
#define LUA_STATUS_MB_END       (MODULE_LUA_STATUS_MB_BASE + MODULE_LUA_STATUS_MB_WORDS)

static rt_err_t lua_status_modbus_read(rt_uint16_t address,
                                       rt_uint16_t *values,
                                       rt_uint16_t count)
{
    if (address < MODULE_LUA_STATUS_MB_BASE ||
        (rt_uint32_t)address + count > LUA_STATUS_MB_END)
        return -RT_EINVAL;
    return module_lua_status_read(
        (rt_uint16_t)(address - MODULE_LUA_STATUS_MB_BASE), values, count) == 0 ?
        RT_EOK : -RT_ERROR;
}

static rt_err_t tag_value_modbus_read(rt_uint16_t address,
                                      rt_uint16_t *values,
                                      rt_uint16_t count)
{
    rt_uint16_t index;
    if (address < TAG_VALUE_MB_BASE ||
        (rt_uint32_t)address + count > TAG_VALUE_MB_END)
        return -RT_EINVAL;
    for (index = 0U; index < count; index++)
    {
        rt_uint32_t relative = (rt_uint32_t)address + index - TAG_VALUE_MB_BASE;
        rt_uint32_t slot = relative / 4U;
        rt_uint16_t field = (rt_uint16_t)(relative & 3U);
        rt_uint8_t port = (rt_uint8_t)(slot / (TAG_DEVICE_COUNT * TAG_DEVICE_TAG_COUNT));
        rt_uint16_t within = (rt_uint16_t)(slot % (TAG_DEVICE_COUNT * TAG_DEVICE_TAG_COUNT));
        rt_uint8_t device = (rt_uint8_t)(within / TAG_DEVICE_TAG_COUNT + 1U);
        rt_uint8_t sensor = (rt_uint8_t)(within % TAG_DEVICE_TAG_COUNT + 1U);
        tag_value_t tag;
        memset(&tag, 0, sizeof(tag));
        tag.id = TAG_ID(port, device, sensor);
        (void)tag_read(tag.id, &tag);
        if (field == 0U) values[index] = tag.id;
        else if (field == 1U) values[index] = (rt_uint16_t)(((rt_uint16_t)tag.type << 8) | tag.flags);
        else if (field == 2U) values[index] = (rt_uint16_t)(tag.value_bits >> 16);
        else values[index] = (rt_uint16_t)tag.value_bits;
    }
    return RT_EOK;
}
#define ELAM_THREAD_STACK    1536U
#define ELAM_THREAD_PRIORITY 15U
#define ELAM_GAP_TICKS       3U
#define MODULE_TIT_CONFIG_BASE 2500U
#define MODULE_TIT_CONFIG_END  2505U

struct elam_port
{
    const char *device_name;
    const char *thread_name;
    rt_device_t device;
    struct rt_semaphore rx_sem;
    rt_uint8_t rx[ELAM_PACKET_MAX];
    rt_uint8_t tx[ELAM_PACKET_MAX];
};

static struct elam_port ports[] =
{
    {"uart8", "elam_u8", RT_NULL}
};

rt_uint16_t TIT[ELAM_MODBUS_REGISTER_MAX];
static struct rt_mutex register_lock;
static struct rt_mutex read_lock;
static rt_uint16_t read_values[ELAM_EXTENDED_READ_MAX];

/*
 * Вычисляет контрольную сумму Modbus CRC16.
 *
 * data   - указатель на первый байт данных;
 * length - количество байтов, участвующих в расчёте.
 *
 * Возвращает CRC16 в формате процессора. При передаче сначала отправляется
 * младший байт CRC, затем старший.
 */
static rt_uint16_t modbus_crc16(const rt_uint8_t *data, rt_size_t length)
{
    rt_uint16_t crc = 0xffff;
    rt_uint8_t bit;

    while (length--)
    {
        crc ^= *data++;
        for (bit = 0; bit < 8; bit++)
            crc = (crc & 1U) ? (rt_uint16_t)((crc >> 1) ^ 0xa001U) : (rt_uint16_t)(crc >> 1);
    }
    return crc;
}

/*
 * Определяет тип принятого кадра и положение кода функции.
 *
 * Для обычного Modbus адрес станции находится в первом байте.
 * Для ELAM используется расширенный адрес:
 *   station = ((request[0] & 7) << 8) + request[1] + 248.
 *
 * request         - принятый кадр;
 * length          - длина принятого кадра;
 * elam            - сюда записывается признак формата ELAM;
 * station         - сюда записывается вычисленный адрес станции;
 * function_offset - сюда записывается индекс байта кода функции.
 *
 * Возвращает RT_TRUE, если длины достаточно для разбора заголовка.
 */
static rt_bool_t frame_layout(const rt_uint8_t *request, rt_size_t length,
                              rt_bool_t *elam, rt_uint16_t *station,
                              rt_size_t *function_offset)
{
    *elam = ((request[0] & 0xf8U) == 0xf8U);
    *function_offset = *elam ? 2U : 1U;
    if (length < (*elam ? 9U : 8U))
        return RT_FALSE;

    if (*elam)
        *station = (rt_uint16_t)((((rt_uint16_t)request[0] & 7U) << 8) + request[1] + 248U);
    else
        *station = request[0];
    return RT_TRUE;
}

/*
 * Проверяет CRC принятого Modbus/ELAM-кадра.
 *
 * frame  - принятый кадр вместе с двумя байтами CRC;
 * length - полная длина кадра.
 *
 * Возвращает RT_TRUE при правильной CRC, иначе RT_FALSE.
 */
static rt_bool_t frame_crc_ok(const rt_uint8_t *frame, rt_size_t length)
{
    rt_uint16_t crc;
    if (length < 4U)
        return RT_FALSE;
    crc = modbus_crc16(frame, length - 2U);
    return frame[length - 2U] == (rt_uint8_t)crc &&
           frame[length - 1U] == (rt_uint8_t)(crc >> 8);
}

/*
 * Вычисляет CRC и дописывает её в конец формируемого ответа.
 *
 * frame              - буфер ответа;
 * length_without_crc - длина ответа без двух байтов CRC.
 */
static void append_crc(rt_uint8_t *frame, rt_size_t length_without_crc)
{
    rt_uint16_t crc = modbus_crc16(frame, length_without_crc);
    frame[length_without_crc] = (rt_uint8_t)crc;
    frame[length_without_crc + 1U] = (rt_uint8_t)(crc >> 8);
}

/*
 * Формирует ответ Modbus Exception при ошибке запроса.
 *
 * request   - исходный запрос, из которого копируется адрес;
 * elam      - признак расширенного формата ELAM;
 * exception - код ошибки Modbus;
 * response  - буфер формируемого ответа.
 *
 * Возвращает полную длину ответа вместе с CRC.
 */
static rt_size_t exception_response(const rt_uint8_t *request, rt_bool_t elam,
                                    rt_uint8_t exception, rt_uint8_t *response)
{
    rt_size_t function_offset = elam ? 2U : 1U;
    memcpy(response, request, function_offset);
    response[function_offset] = request[function_offset] | 0x80U;
    response[function_offset + 1U] = exception;
    append_crc(response, function_offset + 2U);
    return function_offset + 4U;
}

/*
 * Разбирает один запрос и формирует ответ ELAM Modbus.
 *
 * Поддерживаются функции:
 *   03 - чтение holding-регистров;
 *   04 - чтение input-регистров из той же таблицы;
 *   06 - запись одного регистра;
 *   16 - запись нескольких регистров.
 *
 * Обрабатываются только кадры с корректной CRC и адресом станции
 * ELAM_MODBUS_ADDRESS. Доступ к общей таблице регистров защищён mutex.
 *
 * request  - принятый запрос;
 * length   - полная длина запроса;
 * response - буфер для ответа.
 *
 * Возвращает длину готового ответа. Ноль означает, что кадр нужно пропустить.
 */
static rt_size_t process_request(const rt_uint8_t *request, rt_size_t length,
                                 rt_uint8_t *response)
{
    rt_bool_t elam;
    rt_uint16_t station, address, quantity, value;
    rt_err_t read_result;
    rt_size_t function_offset, i;
    rt_uint8_t function;

    if (!frame_layout(request, length, &elam, &station, &function_offset) ||
        station != ELAM_MODBUS_ADDRESS || !frame_crc_ok(request, length))
        return 0;

    function = request[function_offset];
    address = (rt_uint16_t)((request[function_offset + 1U] << 8) |
                            request[function_offset + 2U]);
    quantity = (rt_uint16_t)((request[function_offset + 3U] << 8) |
                             request[function_offset + 4U]);

    if ((function == 3U) || (function == 4U))
    {
        if (!quantity ||
            quantity > (station < 278U ? ELAM_STANDARD_READ_MAX :
                                         ELAM_EXTENDED_READ_MAX))
            return exception_response(request, elam, 2U, response);
        if (function == 3U &&
            !((address < HOLDING_FLASHDB_REG_COUNT &&
               quantity <= HOLDING_FLASHDB_REG_COUNT - address) ||
              (address >= THREAD_PROFILER_MB_BASE &&
               address < THREAD_PROFILER_MB_END &&
               quantity <= THREAD_PROFILER_MB_END - address) ||
              (address >= MODULE_MB_DATA_BASE &&
               address < MODULE_MB_END &&
               quantity <= MODULE_MB_END - address) ||
              (address >= TAGCFG_MB_DATA_BASE &&
               address < TAGCFG_MB_END &&
               quantity <= TAGCFG_MB_END - address) ||
              (address >= TAG_VALUE_MB_BASE &&
               address < TAG_VALUE_MB_END &&
               quantity <= TAG_VALUE_MB_END - address) ||
              (address >= MODULE_LUA_STATUS_MB_BASE &&
               address < LUA_STATUS_MB_END &&
               quantity <= LUA_STATUS_MB_END - address)))
            return exception_response(request, elam, 2U, response);
        if (function == 4U &&
            ((address < ELAM_MODBUS_REGISTER_MAX &&
              quantity > ELAM_MODBUS_REGISTER_MAX - address) ||
             (rt_uint32_t)address + quantity > 0x10000UL))
            return exception_response(request, elam, 2U, response);

        memcpy(response, request, function_offset + 1U);
        if (elam)
        {
            response[function_offset + 1U] = (rt_uint8_t)(quantity >> 7);
            response[function_offset + 2U] = (rt_uint8_t)(quantity << 1);
            i = function_offset + 3U;
        }
        else
        {
            response[function_offset + 1U] = (rt_uint8_t)(quantity << 1);
            i = function_offset + 2U;
        }

        /* Функция 03 читает 7000 holding-регистров непосредственно из KVDB. */
        if (function == 3U)
        {
            rt_mutex_take(&read_lock, RT_WAITING_FOREVER);
            if (address >= MODULE_LUA_STATUS_MB_BASE)
            {
                read_result = lua_status_modbus_read(address, read_values,
                                                     quantity);
            }
            else if (address >= TAG_VALUE_MB_BASE)
            {
                read_result = tag_value_modbus_read(address, read_values,
                                                    quantity);
            }
            else if (address >= TAGCFG_MB_DATA_BASE)
            {
                read_result = tag_config_modbus_read(address, read_values,
                                                     quantity);
            }
            else if (address >= MODULE_MB_DATA_BASE)
            {
                read_result = module_modbus_read(address, read_values,
                                                 quantity);
            }
            else if (address >= THREAD_PROFILER_MB_BASE)
            {
                read_result = thread_profiler_modbus_read(address, read_values,
                                                          quantity);
            }
            else
                read_result = holding_flashdb_read(address, read_values,
                                                   quantity);
            if (read_result != RT_EOK)
            {
                rt_mutex_release(&read_lock);
                return exception_response(request, elam, 4U, response);
            }
            for (value = 0; value < quantity; value++)
            {
                response[i++] = (rt_uint8_t)(read_values[value] >> 8);
                response[i++] = (rt_uint8_t)read_values[value];
            }
            rt_mutex_release(&read_lock);
            append_crc(response, i);
            return i + 2U;
        }

        /* Функция 04 от адреса 7000 передаётся диспетчеру архивов. */
        if (address >= ELAM_MODBUS_REGISTER_MAX)
        {
            if (!arx_modbus_read(address, quantity, response + i))
                return exception_response(request, elam, 2U, response);
            i += (rt_size_t)quantity * 2U;
            append_crc(response, i);
            return i + 2U;
        }

        rt_mutex_take(&register_lock, RT_WAITING_FOREVER);
        while (quantity--)
        {
            /*
             * Функция 04 в диапазоне 0..6999 читает внутреннюю TIT.
             */
            value = TIT[address];
            address++;
            response[i++] = (rt_uint8_t)(value >> 8);
            response[i++] = (rt_uint8_t)value;
        }
        rt_mutex_release(&register_lock);
        append_crc(response, i);
        return i + 2U;
    }

    if (function == 6U)
    {
        /*
         * Функция 06 пишет одно слово в обычный HOLDING.
         * Поле quantity для этой функции содержит записываемое значение.
         */
        value = quantity;
        if (address == THREAD_PROFILER_MB_CONTROL)
        {
            rt_err_t profiler_result;
            if (value == 1U)
                profiler_result = thread_profiler_init();
            else if (value == 0U)
                profiler_result = thread_profiler_stop();
            else
                return exception_response(request, elam, 3U, response);
            if (profiler_result != RT_EOK)
                return exception_response(request, elam, 4U, response);
        }
        else if (address >= MODULE_TIT_CONFIG_BASE &&
            address < MODULE_TIT_CONFIG_END)
        {
            /*
             * Только TIT[2500..2504] разрешены для внешней настройки модуля.
             * Остальная TIT по-прежнему изменяется только внутренними задачами.
             */
            if (elam_modbus_write_register(address, value) != RT_EOK)
                return exception_response(request, elam, 4U, response);
        }
        else if (address >= TAGCFG_MB_DATA_BASE &&
                 address < TAGCFG_MB_END)
        {
            if (tag_config_modbus_write(address, &value, 1U) != RT_EOK)
                return exception_response(request, elam, 4U, response);
        }
        else if (address >= MODULE_MB_DATA_BASE && address < MODULE_MB_END)
        {
            if (module_modbus_write(address, &value, 1U) != RT_EOK)
                return exception_response(request, elam, 4U, response);
        }
        else if (!arx_modbus_write(address, &value, 1U))
            return exception_response(request, elam, 2U, response);
        memcpy(response, request, length);
        return length;
    }

    if (function == 16U)
    {
        rt_uint8_t byte_count;
        rt_uint16_t values[123];

        if (length < function_offset + 8U)
            return 0;
        byte_count = request[function_offset + 5U];
        if (!quantity || quantity > 123U ||
            byte_count != quantity * 2U ||
            length != function_offset + 8U + byte_count)
            return exception_response(request, elam, 3U, response);

        for (i = 0; i < quantity; i++)
            values[i] =
                (rt_uint16_t)((request[function_offset + 6U + i * 2U] << 8) |
                              request[function_offset + 7U + i * 2U]);

        if (address < MODULE_TIT_CONFIG_END &&
            (rt_uint32_t)address + quantity > MODULE_TIT_CONFIG_BASE)
        {
            if (address < MODULE_TIT_CONFIG_BASE ||
                (rt_uint32_t)address + quantity > MODULE_TIT_CONFIG_END)
                return exception_response(request, elam, 2U, response);

            rt_mutex_take(&register_lock, RT_WAITING_FOREVER);
            for (i = 0; i < quantity; i++)
                TIT[address + i] = values[i];
            rt_mutex_release(&register_lock);
        }
        else if (address >= TAGCFG_MB_DATA_BASE &&
                 address < TAGCFG_MB_END)
        {
            if (tag_config_modbus_write(address, values,
                                        quantity) != RT_EOK)
                return exception_response(request, elam, 4U, response);
        }
        else if (address >= MODULE_MB_DATA_BASE && address < MODULE_MB_END)
        {
            if (module_modbus_write(address, values, quantity) != RT_EOK)
                return exception_response(request, elam, 4U, response);
        }
        else if (!arx_modbus_write(address, values, quantity))
        {
            return exception_response(request, elam, 2U, response);
        }

        /*
         * Ответ функции 16 содержит адрес и количество записанных слов,
         * но не содержит byte count и сами данные.
         */
        memcpy(response, request, function_offset + 5U);
        append_crc(response, function_offset + 5U);
        return function_offset + 7U;
    }

    return exception_response(request, elam, 1U, response);
}

/*
 * Определяет полную длину одного запроса внутри составного GPRS-пакета.
 *
 * Функции 03, 04 и 06 имеют фиксированную длину. Для функции 16 длина
 * вычисляется по полю byte count. Формат ELAM длиннее обычного Modbus
 * на один байт расширенного адреса.
 *
 * Возвращает длину кадра, 0 для неполного кадра или оставшегося хвоста.
 */
static rt_size_t request_length(const rt_uint8_t *request, rt_size_t available)
{
    rt_bool_t elam;
    rt_size_t function_offset;
    rt_uint8_t function;

    if (!available)
        return 0;
    elam = ((request[0] & 0xf8U) == 0xf8U);
    function_offset = elam ? 2U : 1U;
    if (available <= function_offset)
        return 0;
    function = request[function_offset];

    if (function == 16U)
    {
        if (available <= function_offset + 5U)
            return 0;
        return function_offset + 8U + request[function_offset + 5U];
    }

    return function_offset + 7U;
}

/*
 * Обрабатывает составной GPRS-пакет, содержащий до 17 Modbus/ELAM-запросов.
 *
 * Каждый кадр имеет собственную длину и CRC. Ответы добавляются в tx только
 * целиком. Функция прекращает обработку на неполном кадре, после 17 запросов
 * или если следующий полный ответ не помещается в 1200-байтный tx-буфер.
 *
 * Возвращает суммарную длину всех сформированных ответов.
 */
u16 raz2(const u8 *input, u16 input_size, u8 *output, u16 output_max)
{
    u8 *one_response;
    u16 input_offset = 0;
    u16 output_size = 0;
    u16 frame_size;
    u16 response_size;
    u8 request_count = 0;

    one_response = (u8 *)rt_malloc(output_max);
    if (one_response == RT_NULL)
        return 0U;

    while (input_offset < input_size && request_count < ELAM_REQUEST_MAX)
    {
        frame_size = (u16)request_length(input + input_offset,
                                         input_size - input_offset);
        if (!frame_size || frame_size > input_size - input_offset)
            break;

        response_size = (u16)process_request(input + input_offset, frame_size,
                                             one_response);
        if (response_size)
        {
            if (response_size > output_max - output_size)
                break;
            memcpy(output + output_size, one_response, response_size);
            output_size += response_size;
        }
        input_offset += frame_size;
        request_count++;
    }
    rt_free(one_response);
    return output_size;
}

/*
 * Callback serial-драйвера RT-Thread о поступлении данных.
 *
 * Находит структуру служебного UART8 по указателю устройства и освобождает
 * семафор его потока. Чтение и обработка кадра в прерывании
 * не выполняются.
 *
 * device - устройство, на котором появились данные;
 * size   - количество доступных байтов, переданное serial-драйвером.
 *
 * Возвращает RT_EOK при найденном порте или -RT_ERROR.
 */
static rt_err_t rx_indicate(rt_device_t device, rt_size_t size)
{
    rt_size_t i;
    (void)size;
    for (i = 0; i < sizeof(ports) / sizeof(ports[0]); i++)
        if (ports[i].device == device)
            return rt_sem_release(&ports[i].rx_sem);
    return -RT_ERROR;
}

/*
 * Рабочая функция отдельного потока ELAM Modbus.
 *
 * Поток ожидает семафор приёма, читает байты из кольцевого буфера UART и
 * ожидает межкадровую паузу ELAM_GAP_TICKS. После паузы считает кадр
 * завершённым, вызывает process_request() и передаёт сформированный ответ
 * через тот же UART.
 *
 * parameter - указатель на struct elam_port конкретного UART.
 *
 * Функция является бесконечным циклом потока и не возвращает значение.
 */
static void elam_thread(void *parameter)
{
    struct elam_port *port = (struct elam_port *)parameter;
    rt_size_t used = 0, count, response_length;

    for (;;)
    {
        if (rt_sem_take(&port->rx_sem, RT_WAITING_FOREVER) != RT_EOK)
            continue;

        do
        {
            count = rt_device_read(port->device, 0, port->rx + used,
                                   sizeof(port->rx) - used);
            used += count;
        } while (count && used < sizeof(port->rx));

        while (rt_sem_take(&port->rx_sem, ELAM_GAP_TICKS) == RT_EOK)
        {
            do
            {
                count = rt_device_read(port->device, 0, port->rx + used,
                                       sizeof(port->rx) - used);
                used += count;
            } while (count && used < sizeof(port->rx));
        }

        response_length = raz2(port->rx, (u16)used, port->tx,
                               (u16)sizeof(port->tx));
        if (response_length)
            rt_device_write(port->device, 0, port->tx, response_length);
        used = 0;
    }
}

/*
 * Инициализирует служебный ELAM Modbus только на UART8.
 *
 * Последовательность создания:
 *   1. Задаются общие параметры UART: 19200 бод, 8 бит, без чётности,
 *      один стоп-бит (8N1), приёмный буфер 256 байт.
 *   2. Создаётся mutex общей таблицы из ELAM_MODBUS_REGISTER_MAX регистров.
 *   3. Находится устройство RT-Thread с именем "uart8".
 *   4. Создаётся отдельный семафор приёма.
 *   5. UART конфигурируется и открывается в режиме приёма по прерыванию.
 *   6. Регистрируется callback rx_indicate().
 *   7. Создаётся поток elam_u8 со стеком
 *      ELAM_THREAD_STACK и приоритетом ELAM_THREAD_PRIORITY.
 *   8. Созданный поток запускается функцией rt_thread_startup().
 *
 * Возвращает RT_EOK после успешного запуска потока UART8 либо код ошибки,
 * если устройство не найдено, UART не открылся или поток не создался.
 */
rt_err_t elam_modbus_start(void)
{
    struct serial_configure config = RT_SERIAL_CONFIG_DEFAULT;
    rt_thread_t thread;
    rt_size_t i;

    config.baud_rate = BAUD_RATE_19200;
    config.data_bits = DATA_BITS_8;
    config.stop_bits = STOP_BITS_1;
    config.parity = PARITY_NONE;
    config.bufsz = 256;
    rt_mutex_init(&register_lock, "elam_reg", RT_IPC_FLAG_PRIO);
    rt_mutex_init(&read_lock, "elam_read", RT_IPC_FLAG_PRIO);

    for (i = 0; i < sizeof(ports) / sizeof(ports[0]); i++)
    {
        ports[i].device = rt_device_find(ports[i].device_name);
        if (!ports[i].device)
            return -RT_ENOSYS;
        rt_sem_init(&ports[i].rx_sem, ports[i].thread_name, 0, RT_IPC_FLAG_FIFO);
        if (rt_device_control(ports[i].device, RT_DEVICE_CTRL_CONFIG, &config) != RT_EOK ||
            rt_device_open(ports[i].device, RT_DEVICE_FLAG_INT_RX) != RT_EOK)
            return -RT_ERROR;
        rt_device_set_rx_indicate(ports[i].device, rx_indicate);
        thread = rt_thread_create(ports[i].thread_name, elam_thread, &ports[i],
                                  ELAM_THREAD_STACK, ELAM_THREAD_PRIORITY, 10);
        if (!thread)
            return -RT_ENOMEM;
        rt_thread_startup(thread);
    }
    return RT_EOK;
}

/*
 * Безопасно читает один регистр из общей таблицы ELAM Modbus.
 *
 * address - адрес регистра от 0 до ELAM_MODBUS_REGISTER_MAX - 1;
 * value   - указатель, куда будет записано значение.
 *
 * Доступ защищён register_lock. Возвращает RT_EOK либо -RT_EINVAL при
 * неправильном адресе или нулевом указателе value.
 */
rt_err_t elam_modbus_read_register(rt_uint16_t address, rt_uint16_t *value)
{
    if (!value || address >= ELAM_MODBUS_REGISTER_MAX)
        return -RT_EINVAL;
    rt_mutex_take(&register_lock, RT_WAITING_FOREVER);
    *value = TIT[address];
    rt_mutex_release(&register_lock);
    return RT_EOK;
}

/*
 * Безопасно записывает один регистр общей таблицы ELAM Modbus.
 *
 * address - адрес регистра от 0 до ELAM_MODBUS_REGISTER_MAX - 1;
 * value   - новое 16-битное значение регистра.
 *
 * Доступ защищён register_lock. Возвращает RT_EOK либо -RT_EINVAL при
 * выходе адреса за границы таблицы.
 */
rt_err_t elam_modbus_write_register(rt_uint16_t address, rt_uint16_t value)
{
    if (address >= ELAM_MODBUS_REGISTER_MAX)
        return -RT_EINVAL;
    rt_mutex_take(&register_lock, RT_WAITING_FOREVER);
    TIT[address] = value;
    rt_mutex_release(&register_lock);
    return RT_EOK;
}

/*
 * Копирует массив 16-битных слов в отдельную таблицу HOLDING функции 04.
 *
 * address - первый holding-регистр;
 * data    - копируемые слова;
 * count   - количество слов.
 */
rt_err_t elam_modbus_write_holding(rt_uint16_t address,
                                   const rt_uint16_t *data,
                                   rt_uint16_t count)
{
    return holding_flashdb_write(address, data, count);
}
