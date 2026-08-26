/*
 * Copyright (c) 2006-2020, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date           Author       Notes
 * 2020-11-21     RT-Thread    first version
 */

#include <rtthread.h>
#include "elam_modbus.h"
#include "thread_profiler.h"
#include <fal.h>
#include "arx/arx_example.h"
#include "arx/holding_flashdb.h"
#include "modules/qspi_modules.h"
#include "modules/module_modbus.h"
#include "modules/module_service_api.h"
#include "web_server.h"

#define DBG_TAG "main"
#define DBG_LVL DBG_LOG
#include <rtdbg.h>

/*
 * Инициализирует медленные подсистемы W25Q128/FAL/FlashDB в фоне.
 * UART8 ELAM и прикладной UART7 уже работают и не ждут FlashDB.
 */
static void storage_init_thread(void *parameter)
{
    (void)parameter;
    if (fal_init() > 0)
    {
        if (holding_flashdb_start() != RT_EOK)
            LOG_E("FlashDB holding start failed");
        if (arx_example_init() != RT_EOK)
            LOG_E("FlashDB archive start failed");
        if (module_start_slot(0U) != MODULE_OK)
            LOG_E("QSPI module slot 0 start failed");
        if (module_start_slot(1U) != MODULE_OK)
            LOG_E("QSPI module slot 1 start failed");
    }
    else
    {
        LOG_E("W25Q128/FAL initialization failed");
    }
}

/*
 * Инициализирует диагностику, загрузчик модулей, Modbus и фоновые
 * FlashDB-хранилища, после чего оставляет управление потокам RT-Thread.
 */
int main(void)
{
    rt_thread_t storage_thread;

    /*
     * RAM-часть загрузчика создается до Modbus. Физическая запись W25Q128
     * станет доступна после завершения fal_init() в фоновом потоке.
     */
    if (module_manager_init() != MODULE_OK)
        LOG_E("QSPI module manager start failed");
    if (module_modbus_init() != RT_EOK)
        LOG_E("QSPI module Modbus window start failed");
    if (module_service_init() != RT_EOK)
        LOG_E("Module UART7/UART2 service start failed");

    /* Обмен запускается первым и не зависит от состояния внешней Flash. */
    if (elam_modbus_start() != RT_EOK)
    {
        LOG_E("ELAM Modbus UART8 start failed");
    }

    if (web_server_start() != RT_EOK)
    {
        LOG_E("Mongoose web server start failed");
    }

    storage_thread = rt_thread_create("storage_init", storage_init_thread,
                                      RT_NULL, 4096, 20, 10);
    if (storage_thread)
        rt_thread_startup(storage_thread);
    else
        LOG_E("Storage initialization thread create failed");

    while (1)
    {
        rt_thread_mdelay(1000);
    }
}
