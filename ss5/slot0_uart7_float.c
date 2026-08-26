#include "../module_service_api.h"

#define MODULE_SLOT               0U
#define MODULE_BODY_OFFSET        12U
#define MODULE_TYPE_UART7_FLOAT   1U
#define MODULE_VERSION            1U

#define TIT_MODULE_ENABLE         2500U
#define TIT_POLL_PERIOD_MS        2501U
#define TIT_SCALE_X1000           2502U
#define TIT_ARCHIVE_ENABLE        2504U
#define TIT_FLOAT_HIGH            2510U
#define TIT_FLOAT_LOW             2511U
#define TIT_COMM_STATUS           2520U
#define TIT_SUCCESS_COUNT         2521U
#define HOLDING_FLOAT_ORDER       \
    (MODULE_HOLDING_FLOAT_ORDER_BASE + MODULE_SLOT)

#define RTU_SLAVE_ADDRESS         1U
#define RTU_FLOAT_REGISTER        27U
#define RTU_FLOAT_WORD_COUNT      2U
#define RTU_TIMEOUT_MS            500U

#pragma pack(push, 1)
typedef struct
{
    u16 crc;
    u16 addr;
    u16 size;
    u16 type;
    u16 num;
} example_module_header_t;
#pragma pack(pop)

typedef union
{
    u32 bits;
    float value;
} float_bits_t;

#pragma location=".module0_header"
__root const example_module_header_t module_header =
{
    0U,
    MODULE_BODY_OFFSET,
    0U,
    MODULE_TYPE_UART7_FLOAT,
    MODULE_VERSION
};

/*
 * Читает TIT через стабильную сервисную таблицу основной прошивки.
 * При ошибке возвращает заданное значение по умолчанию.
 */
#pragma location=".module0_code"
static u16 module_tit_read_default(const module_service_api_t *api,
                                   u16 address, u16 default_value)
{
    u16 value;

    if (api->tit_read(address, &value) != 0)
        return default_value;
    return value;
}

/*
 * Собирает IEEE-754 float из двух Modbus-регистров.
 * word_order=0: первый регистр содержит старшее слово;
 * word_order=1: слова меняются местами.
 */
#pragma location=".module0_code"
static float module_words_to_float(const u16 *registers, u16 word_order)
{
    float_bits_t converted;
    u16 first = registers[0];
    u16 second = registers[1];

    switch (word_order)
    {
    case MODULE_FLOAT_ORDER_CDAB:
        converted.bits = ((u32)second << 16) | first;
        break;
    case MODULE_FLOAT_ORDER_BADC:
        converted.bits =
            ((u32)(first & 0x00ffU) << 24) |
            ((u32)(first & 0xff00U) << 8) |
            ((u32)(second & 0x00ffU) << 8) |
            ((u32)(second & 0xff00U) >> 8);
        break;
    case MODULE_FLOAT_ORDER_DCBA:
        converted.bits =
            ((u32)(second & 0x00ffU) << 24) |
            ((u32)(second & 0xff00U) << 8) |
            ((u32)(first & 0x00ffU) << 8) |
            ((u32)(first & 0xff00U) >> 8);
        break;
    default:
        converted.bits = ((u32)first << 16) | second;
        break;
    }
    return converted.value;
}

/*
 * Публикует float как два слова TIT для чтения верхним уровнем
 * через ELAM function 04 на служебном UART8.
 */
#pragma location=".module0_code"
static void module_publish_float(const module_service_api_t *api,
                                 float value)
{
    float_bits_t converted;

    converted.value = value;
    api->tit_write(TIT_FLOAT_HIGH, (u16)(converted.bits >> 16));
    api->tit_write(TIT_FLOAT_LOW, (u16)converted.bits);
}

/*
 * Передает исходные слова и рассчитанный float в демонстрационный архив.
 */
#pragma location=".module0_code"
static void module_archive_value(const module_service_api_t *api,
                                 const u16 *registers, float value)
{
    u16 archive_values[4];
    float_bits_t converted;

    converted.value = value;
    archive_values[0] = registers[0];
    archive_values[1] = registers[1];
    archive_values[2] = (u16)(converted.bits >> 16);
    archive_values[3] = (u16)converted.bits;
    api->archive_tii_append(api->timestamp_get(),
                            archive_values, 4U);
}

/*
 * Точка входа модуля слота 0.
 *
 * Опрос: UART7, Modbus RTU master, 19200 8N1, slave 1,
 * function 03, начальный регистр 27, два слова IEEE-754 float.
 */
#pragma location=".module0_entry"
__root void module_entry(void)
{
    const module_service_api_t *api =
        (const module_service_api_t *)MODULE_SERVICE_API_ADDRESS;
    u16 registers[RTU_FLOAT_WORD_COUNT];
    u16 period_ms;
    u16 scale_x1000;
    u16 word_order;
    u16 success_count = 0U;
    float value;
    s32 result;

    if (api->magic != MODULE_SERVICE_API_MAGIC ||
        api->version != MODULE_SERVICE_API_VERSION ||
        api->size < sizeof(module_service_api_t))
        return;

    for (;;)
    {
        if (module_tit_read_default(api, TIT_MODULE_ENABLE, 0U) == 0U)
        {
            api->delay_ms(100U);
            continue;
        }

        period_ms = module_tit_read_default(api,
                                            TIT_POLL_PERIOD_MS, 1000U);
        if (period_ms < 100U)
            period_ms = 100U;
        scale_x1000 = module_tit_read_default(api,
                                              TIT_SCALE_X1000, 1000U);
        if (scale_x1000 == 0U)
            scale_x1000 = 1000U;
        result = api->rtu7_read_holding(RTU_SLAVE_ADDRESS,
                                        RTU_FLOAT_REGISTER,
                                        RTU_FLOAT_WORD_COUNT,
                                        registers,
                                        RTU_TIMEOUT_MS);
        api->tit_write(TIT_COMM_STATUS, (u16)result);
        if (result == 0)
        {
            word_order = MODULE_FLOAT_ORDER_ABCD;
            if (api->holding_read(HOLDING_FLOAT_ORDER, &word_order, 1U) != 0 ||
                word_order > MODULE_FLOAT_ORDER_DCBA)
                word_order = MODULE_FLOAT_ORDER_ABCD;
            value = module_words_to_float(registers, word_order);
            value = value * (float)scale_x1000 / 1000.0f;
            module_publish_float(api, value);
            success_count++;
            api->tit_write(TIT_SUCCESS_COUNT, success_count);

            if (module_tit_read_default(api,
                                        TIT_ARCHIVE_ENABLE, 0U) != 0U)
                module_archive_value(api, registers, value);
        }
        api->delay_ms(period_ms);
    }
}
