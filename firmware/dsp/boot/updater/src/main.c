/* Updater: RAM-only program that writes the U7001 EEPROM for the host (update, recovery, migration).
 * It is never stored in the EEPROM: the host tool refuses its tag in the boot1 area. Because it is not
 * immutable, a bug here is fixed by shipping a new updater, unlike boot1. */
#include "boot.h"
#include "ee.h"
#include "rescue.h"

const char boot_tag[8] = "OCUPDT1";
const uint8_t g_mode = MODE_UPDATER;

uint8_t g_boot_reason;
int g_in_window;
int g_rescue_requested;
extern int g_ee_unlock_boot;

void updater_main(void)
{
    g_boot_reason = R_UPDATER;
    g_in_window = 0;
    g_rescue_requested = 0;
    g_ee_unlock_boot = 0;
    ee_init();          /* PLL already running (stock firmware or boot1) */
    rescue_run();
}
