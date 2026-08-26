#include "../module_service_api.h"

#define MODULE_BODY_OFFSET        12U
#define MODULE_TYPE_UART2_FLOAT   2U
#define MODULE_VERSION            1U
#define MODULE_SLOT_NUMBER        1U

#define TIT_POLL_PERIOD_MS        2501U
#define TIT_SCALE_X1000           2502U
#define TIT_ARCHIVE_ENABLE        2504U
#define TIT_FLOAT_HIGH            2512U
#define TIT_FLOAT_LOW             2513U
#define TIT_COMM_STATUS           2522U
#define TIT_SUCCESS_COUNT         2523U
#define HOLDING_FLOAT_ORDER       \
    (MODULE_HOLDING_FLOAT_ORDER_BASE + MODULE_SLOT_NUMBER)

#define RTU_SLAVE_ADDRESS         1U
#define RTU_FLOAT_REGISTER        27U
#define RTU_FLOAT_WORD_COUNT      2U
#define RTU_TIMEOUT_MS            500U
#define ARCHIVE_PERIOD_SECONDS    60U
#define SLOT1_ARCHIVE_MARKER      0x5101U

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

#pragma location=".module1_header"
__root const example_module_header_t module_header_slot1 =
{
    0U,
    MODULE_BODY_OFFSET,
    0U,
    MODULE_TYPE_UART2_FLOAT,
    MODULE_VERSION
};

/*
 * Reads one TIT register through the service table.
 * Returns default_value when the base firmware reports an error.
 */
#pragma location=".module1_code"
static u16 module1_tit_read_default(const module_service_api_t *api,
                                    u16 address, u16 default_value)
{
    u16 value;

    if (api->tit_read(address, &value) != 0)
        return default_value;
    return value;
}

/* Converts the four Modbus bytes according to the slot Holding setting. */
#pragma location=".module1_code"
static float module1_words_to_float(const u16 *registers, u16 word_order)
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
 * Publishes the resulting float in TIT[2512] and TIT[2513].
 * TIT[2512] contains the high IEEE-754 word.
 */
#pragma location=".module1_code"
static void module1_publish_float(const module_service_api_t *api,
                                  float value)
{
    float_bits_t converted;

    converted.value = value;
    api->tit_write(TIT_FLOAT_HIGH, (u16)(converted.bits >> 16));
    api->tit_write(TIT_FLOAT_LOW, (u16)converted.bits);
}

/*
 * Passes the source registers and converted float to the example archive.
 */
#pragma location=".module1_code"
static void module1_archive_value(const module_service_api_t *api,
                                  const u16 *registers, float value)
{
    u16 archive_values[5];
    float_bits_t converted;

    converted.value = value;
    archive_values[0] = SLOT1_ARCHIVE_MARKER;
    archive_values[1] = registers[0];
    archive_values[2] = registers[1];
    archive_values[3] = (u16)(converted.bits >> 16);
    archive_values[4] = (u16)converted.bits;
    api->archive_tii_append(api->timestamp_get(),
                            archive_values, 5U);
}

/*
 * Entry point of QSPI slot 1.
 *
 * Polls USART2/RS-485 at 19200 8N1 with Modbus RTU function 03:
 * slave 1, first register 27, two words containing an IEEE-754 float.
 * The base firmware controls the native PD4 USART2_DE output.
 */
#pragma location=".module1_entry"
__root void module_entry_slot1(void)
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
    u32 last_archive_timestamp = 0U;

    if (api->magic != MODULE_SERVICE_API_MAGIC ||
        api->version != MODULE_SERVICE_API_VERSION ||
        api->size < sizeof(module_service_api_t))
        return;

    for (;;)
    {
        period_ms = module1_tit_read_default(api,
                                             TIT_POLL_PERIOD_MS, 1000U);
        if (period_ms < 100U)
            period_ms = 100U;
        scale_x1000 = module1_tit_read_default(api,
                                               TIT_SCALE_X1000, 1000U);
        if (scale_x1000 == 0U)
            scale_x1000 = 1000U;
        result = api->rtu2_read_holding(RTU_SLAVE_ADDRESS,
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
            value = module1_words_to_float(registers, word_order);
            value = value * (float)scale_x1000 / 1000.0f;
            module1_publish_float(api, value);
            success_count++;
            api->tit_write(TIT_SUCCESS_COUNT, success_count);

            if (module1_tit_read_default(api, TIT_ARCHIVE_ENABLE, 0U) != 0U &&
                (last_archive_timestamp == 0U ||
                 api->timestamp_get() - last_archive_timestamp >=
                 ARCHIVE_PERIOD_SECONDS))
            {
                module1_archive_value(api, registers, value);
                last_archive_timestamp = api->timestamp_get();
            }
        }
        api->delay_ms(period_ms);
    }
}
