#include <rtthread.h>
#include <fal.h>
#include <board.h>
#include "stm32h7xx_ll_bus.h"
#include "stm32h7xx_ll_gpio.h"
#include "stm32h7xx_ll_spi.h"

#define SPI4_FLASH_SIZE             (16U * 1024U * 1024U)
#define SPI4_FLASH_SECTOR_SIZE      4096U
#define SPI4_FLASH_PAGE_SIZE        256U
#define SPI4_FLASH_JEDEC_ID         0xEF4018UL

#define SPI4_FLASH_CS_PORT          GPIOE
#define SPI4_FLASH_CS_PIN           GPIO_PIN_11
#define SPI4_FLASH_SCK_PIN          GPIO_PIN_12
#define SPI4_FLASH_MISO_PIN         GPIO_PIN_13
#define SPI4_FLASH_MOSI_PIN         GPIO_PIN_14

#define CMD_WRITE_ENABLE            0x06U
#define CMD_READ_STATUS1            0x05U
#define CMD_READ_DATA               0x03U
#define CMD_PAGE_PROGRAM            0x02U
#define CMD_SECTOR_ERASE_4K         0x20U
#define CMD_READ_JEDEC_ID           0x9FU
#define CMD_ENABLE_RESET            0x66U
#define CMD_RESET_DEVICE            0x99U
#define STATUS1_BUSY                0x01U

#define SPI_TRANSFER_TIMEOUT_MS     100U
#define FLASH_READY_TIMEOUT_MS      5000U
#define READ_CHUNK_SIZE             4096U

volatile rt_uint32_t spi4_w25q_jedec_id;
volatile rt_uint8_t spi4_w25q_status1;
volatile rt_int32_t spi4_w25q_last_error;

static struct rt_mutex spi4_flash_lock;
static rt_bool_t spi4_flash_lock_ready;
static rt_bool_t spi4_flash_ready;

static void spi4_cs_high(void)
{
    SPI4_FLASH_CS_PORT->BSRR = SPI4_FLASH_CS_PIN;
}

static void spi4_cs_low(void)
{
    SPI4_FLASH_CS_PORT->BSRR = (rt_uint32_t)SPI4_FLASH_CS_PIN << 16U;
}

static rt_bool_t timeout_expired(rt_tick_t start, rt_uint32_t timeout_ms)
{
    rt_tick_t timeout = rt_tick_from_millisecond((rt_int32_t)timeout_ms);
    return (rt_tick_t)(rt_tick_get() - start) >= timeout;
}

/*
 * Configures U4 on PE11..PE14:
 * PE11 = software CS, PE12 = SPI4_SCK, PE13 = SPI4_MISO,
 * PE14 = SPI4_MOSI.  Clock is kept deliberately low for reliable bring-up.
 */
static void spi4_hardware_init(void)
{
    LL_AHB4_GRP1_EnableClock(LL_AHB4_GRP1_PERIPH_GPIOE);
    spi4_cs_high();

    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_11, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_11, LL_GPIO_SPEED_FREQ_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_11, LL_GPIO_PULL_UP);
    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_11, LL_GPIO_MODE_OUTPUT);

    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_12, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_12, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_12, LL_GPIO_PULL_NO);
    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_13, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_13, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_13, LL_GPIO_PULL_NO);
    LL_GPIO_SetPinOutputType(GPIOE, LL_GPIO_PIN_14, LL_GPIO_OUTPUT_PUSHPULL);
    LL_GPIO_SetPinSpeed(GPIOE, LL_GPIO_PIN_14, LL_GPIO_SPEED_FREQ_VERY_HIGH);
    LL_GPIO_SetPinPull(GPIOE, LL_GPIO_PIN_14, LL_GPIO_PULL_NO);
    LL_GPIO_SetAFPin_8_15(GPIOE, LL_GPIO_PIN_12, LL_GPIO_AF_5);
    LL_GPIO_SetAFPin_8_15(GPIOE, LL_GPIO_PIN_13, LL_GPIO_AF_5);
    LL_GPIO_SetAFPin_8_15(GPIOE, LL_GPIO_PIN_14, LL_GPIO_AF_5);
    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_12, LL_GPIO_MODE_ALTERNATE);
    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_13, LL_GPIO_MODE_ALTERNATE);
    LL_GPIO_SetPinMode(GPIOE, LL_GPIO_PIN_14, LL_GPIO_MODE_ALTERNATE);

    __HAL_RCC_SPI4_CLK_ENABLE();
    __HAL_RCC_SPI4_FORCE_RESET();
    __HAL_RCC_SPI4_RELEASE_RESET();

    /*
     * With software NSS the internal SS input must stay high.  Otherwise
     * enabling SPI raises MODF, clears MASTER and no SCK is generated.
     */
    SPI4->CR1 = SPI_CR1_SSI;
    SPI4->CR2 = 0U;
    SPI4->CFG1 = LL_SPI_DATAWIDTH_8BIT |
                 LL_SPI_FIFO_TH_01DATA |
                 LL_SPI_BAUDRATEPRESCALER_DIV8;
    SPI4->CFG2 = LL_SPI_MODE_MASTER |
                 LL_SPI_FULL_DUPLEX |
                 LL_SPI_NSS_SOFT |
                 LL_SPI_PROTOCOL_MOTOROLA |
                 LL_SPI_PHASE_1EDGE |
                 LL_SPI_POLARITY_LOW |
                 LL_SPI_MSB_FIRST |
                 SPI_CFG2_AFCNTR;
    SPI4->IER = 0U;
}

static void spi4_abort_transfer(void)
{
    spi4_cs_high();
    LL_SPI_Disable(SPI4);
    SPI4->IFCR = SPI_IFCR_EOTC | SPI_IFCR_TXTFC |
                 SPI_IFCR_OVRC | SPI_IFCR_UDRC |
                 SPI_IFCR_TIFREC | SPI_IFCR_MODFC |
                 SPI_IFCR_SUSPC;
}

static int spi4_begin_transfer(rt_uint32_t size)
{
    if (size == 0U || size > 0xFFFFU)
        return -1;

    LL_SPI_Disable(SPI4);
    SPI4->IFCR = SPI_IFCR_EOTC | SPI_IFCR_TXTFC |
                 SPI_IFCR_OVRC | SPI_IFCR_UDRC |
                 SPI_IFCR_TIFREC | SPI_IFCR_MODFC |
                 SPI_IFCR_SUSPC;
    LL_SPI_SetTransferSize(SPI4, size);
    spi4_cs_low();
    LL_SPI_Enable(SPI4);
    LL_SPI_StartMasterTransfer(SPI4);
    return 0;
}

static int spi4_exchange_byte(rt_uint8_t tx, rt_uint8_t *rx)
{
    rt_tick_t start = rt_tick_get();

    while (!LL_SPI_IsActiveFlag_TXP(SPI4))
    {
        if (timeout_expired(start, SPI_TRANSFER_TIMEOUT_MS))
            return -1;
    }
    LL_SPI_TransmitData8(SPI4, tx);

    start = rt_tick_get();
    while (!LL_SPI_IsActiveFlag_RXP(SPI4))
    {
        if (timeout_expired(start, SPI_TRANSFER_TIMEOUT_MS))
            return -1;
    }
    tx = LL_SPI_ReceiveData8(SPI4);
    if (rx != RT_NULL)
        *rx = tx;
    return 0;
}

static int spi4_end_transfer(void)
{
    rt_tick_t start = rt_tick_get();

    while (!LL_SPI_IsActiveFlag_EOT(SPI4))
    {
        if (timeout_expired(start, SPI_TRANSFER_TIMEOUT_MS))
        {
            spi4_abort_transfer();
            return -1;
        }
    }
    spi4_cs_high();
    SPI4->IFCR = SPI_IFCR_EOTC | SPI_IFCR_TXTFC;
    LL_SPI_Disable(SPI4);
    return 0;
}

static int spi4_command(const rt_uint8_t *tx, rt_uint32_t size)
{
    rt_uint32_t index;

    if (spi4_begin_transfer(size) != 0)
        return -1;
    for (index = 0U; index < size; index++)
    {
        if (spi4_exchange_byte(tx[index], RT_NULL) != 0)
        {
            spi4_abort_transfer();
            return -1;
        }
    }
    return spi4_end_transfer();
}

static int flash_read_status1(rt_uint8_t *status)
{
    rt_uint8_t value;

    if (spi4_begin_transfer(2U) != 0)
        return -1;
    if (spi4_exchange_byte(CMD_READ_STATUS1, RT_NULL) != 0 ||
        spi4_exchange_byte(0xFFU, &value) != 0 ||
        spi4_end_transfer() != 0)
    {
        spi4_abort_transfer();
        return -1;
    }
    *status = value;
    spi4_w25q_status1 = value;
    return 0;
}

static int flash_wait_ready(void)
{
    rt_tick_t start = rt_tick_get();
    rt_uint8_t status;

    do
    {
        if (flash_read_status1(&status) != 0)
            return -1;
        if ((status & STATUS1_BUSY) == 0U)
            return 0;
        rt_thread_mdelay(1U);
    } while (!timeout_expired(start, FLASH_READY_TIMEOUT_MS));

    return -1;
}

static int flash_write_enable(void)
{
    const rt_uint8_t command = CMD_WRITE_ENABLE;
    return spi4_command(&command, 1U);
}

static rt_uint32_t flash_read_jedec_id(void)
{
    rt_uint8_t id[3];
    rt_uint32_t index;

    if (spi4_begin_transfer(4U) != 0)
        return 0U;
    if (spi4_exchange_byte(CMD_READ_JEDEC_ID, RT_NULL) != 0)
    {
        spi4_abort_transfer();
        return 0U;
    }
    for (index = 0U; index < 3U; index++)
    {
        if (spi4_exchange_byte(0xFFU, &id[index]) != 0)
        {
            spi4_abort_transfer();
            return 0U;
        }
    }
    if (spi4_end_transfer() != 0)
        return 0U;
    return ((rt_uint32_t)id[0] << 16U) |
           ((rt_uint32_t)id[1] << 8U) |
           id[2];
}

static int spi4_flash_init(void)
{
    const rt_uint8_t enable_reset = CMD_ENABLE_RESET;
    const rt_uint8_t reset_device = CMD_RESET_DEVICE;
    rt_uint8_t status;

    spi4_w25q_last_error = 0;
    spi4_flash_ready = RT_FALSE;

    if (!spi4_flash_lock_ready)
    {
        if (rt_mutex_init(&spi4_flash_lock, "spi4f", RT_IPC_FLAG_PRIO) != RT_EOK)
        {
            spi4_w25q_last_error = 1;
            return -1;
        }
        spi4_flash_lock_ready = RT_TRUE;
    }

    spi4_hardware_init();
    rt_thread_mdelay(2U);
    if (spi4_command(&enable_reset, 1U) != 0 ||
        spi4_command(&reset_device, 1U) != 0)
    {
        spi4_w25q_last_error = 2;
        return -1;
    }
    rt_thread_mdelay(2U);

    spi4_w25q_jedec_id = flash_read_jedec_id();
    if (spi4_w25q_jedec_id != SPI4_FLASH_JEDEC_ID)
    {
        spi4_w25q_last_error = 3;
        return -1;
    }
    if (flash_read_status1(&status) != 0 || flash_wait_ready() != 0)
    {
        spi4_w25q_last_error = 4;
        return -1;
    }

    spi4_flash_ready = RT_TRUE;
    return 0;
}

static int spi4_flash_read(long offset, rt_uint8_t *buffer, size_t size)
{
    size_t done = 0U;

    if (!spi4_flash_ready || offset < 0 || buffer == RT_NULL ||
        (rt_uint32_t)offset + size > SPI4_FLASH_SIZE)
        return -1;
    if (size == 0U)
        return 0;

    rt_mutex_take(&spi4_flash_lock, RT_WAITING_FOREVER);
    while (done < size)
    {
        rt_uint32_t index;
        rt_uint32_t address = (rt_uint32_t)offset + (rt_uint32_t)done;
        rt_uint32_t chunk = (rt_uint32_t)(size - done);
        rt_uint8_t command[4];

        if (chunk > READ_CHUNK_SIZE)
            chunk = READ_CHUNK_SIZE;
        command[0] = CMD_READ_DATA;
        command[1] = (rt_uint8_t)(address >> 16U);
        command[2] = (rt_uint8_t)(address >> 8U);
        command[3] = (rt_uint8_t)address;

        if (spi4_begin_transfer(4U + chunk) != 0)
            goto read_error;
        for (index = 0U; index < 4U; index++)
        {
            if (spi4_exchange_byte(command[index], RT_NULL) != 0)
                goto read_error;
        }
        for (index = 0U; index < chunk; index++)
        {
            if (spi4_exchange_byte(0xFFU, &buffer[done + index]) != 0)
                goto read_error;
        }
        if (spi4_end_transfer() != 0)
            goto read_error;
        done += chunk;
    }
    rt_mutex_release(&spi4_flash_lock);
    return (int)size;

read_error:
    spi4_abort_transfer();
    spi4_w25q_last_error = 10;
    rt_mutex_release(&spi4_flash_lock);
    return -1;
}

static int spi4_flash_write(long offset,
                            const rt_uint8_t *buffer,
                            size_t size)
{
    size_t done = 0U;

    if (!spi4_flash_ready || offset < 0 || buffer == RT_NULL ||
        (rt_uint32_t)offset + size > SPI4_FLASH_SIZE)
        return -1;
    if (size == 0U)
        return 0;

    rt_mutex_take(&spi4_flash_lock, RT_WAITING_FOREVER);
    while (done < size)
    {
        rt_uint32_t index;
        rt_uint32_t address = (rt_uint32_t)offset + (rt_uint32_t)done;
        rt_uint32_t page_left =
            SPI4_FLASH_PAGE_SIZE - (address % SPI4_FLASH_PAGE_SIZE);
        rt_uint32_t chunk = (rt_uint32_t)(size - done);
        rt_uint8_t command[4];

        if (chunk > page_left)
            chunk = page_left;
        if (flash_write_enable() != 0)
            goto write_error;

        command[0] = CMD_PAGE_PROGRAM;
        command[1] = (rt_uint8_t)(address >> 16U);
        command[2] = (rt_uint8_t)(address >> 8U);
        command[3] = (rt_uint8_t)address;
        if (spi4_begin_transfer(4U + chunk) != 0)
            goto write_error;
        for (index = 0U; index < 4U; index++)
        {
            if (spi4_exchange_byte(command[index], RT_NULL) != 0)
                goto write_error;
        }
        for (index = 0U; index < chunk; index++)
        {
            if (spi4_exchange_byte(buffer[done + index], RT_NULL) != 0)
                goto write_error;
        }
        if (spi4_end_transfer() != 0 || flash_wait_ready() != 0)
            goto write_error;
        done += chunk;
    }
    rt_mutex_release(&spi4_flash_lock);
    return (int)size;

write_error:
    spi4_abort_transfer();
    spi4_w25q_last_error = 20;
    rt_mutex_release(&spi4_flash_lock);
    return -1;
}

static int spi4_flash_erase(long offset, size_t size)
{
    rt_uint32_t address;
    rt_uint32_t end;

    if (!spi4_flash_ready || offset < 0 ||
        (rt_uint32_t)offset + size > SPI4_FLASH_SIZE)
        return -1;
    if (size == 0U)
        return 0;

    address = (rt_uint32_t)offset & ~(SPI4_FLASH_SECTOR_SIZE - 1U);
    end = ((rt_uint32_t)offset + (rt_uint32_t)size +
           SPI4_FLASH_SECTOR_SIZE - 1U) &
          ~(SPI4_FLASH_SECTOR_SIZE - 1U);

    rt_mutex_take(&spi4_flash_lock, RT_WAITING_FOREVER);
    while (address < end)
    {
        rt_uint8_t command[4];

        if (flash_write_enable() != 0)
            goto erase_error;
        command[0] = CMD_SECTOR_ERASE_4K;
        command[1] = (rt_uint8_t)(address >> 16U);
        command[2] = (rt_uint8_t)(address >> 8U);
        command[3] = (rt_uint8_t)address;
        if (spi4_command(command, sizeof(command)) != 0 ||
            flash_wait_ready() != 0)
            goto erase_error;
        address += SPI4_FLASH_SECTOR_SIZE;
    }
    rt_mutex_release(&spi4_flash_lock);
    return (int)size;

erase_error:
    spi4_abort_transfer();
    spi4_w25q_last_error = 30;
    rt_mutex_release(&spi4_flash_lock);
    return -1;
}

struct fal_flash_dev db_flash =
{
    .name = "dbflash",
    .addr = 0,
    .len = SPI4_FLASH_SIZE,
    .blk_size = SPI4_FLASH_SECTOR_SIZE,
    .ops = {
        spi4_flash_init,
        spi4_flash_read,
        spi4_flash_write,
        spi4_flash_erase
    },
    .write_gran = 1
};
