/* Software rescue request: our firmware writes RESCUE_MAGIC and its complement at RAM_FLAG, then reboots
 * (RTI watchdog reset). The flag is consumed here, so it only acts once. If the RAM is not retained across
 * the reset (to be verified on hardware), the request is simply lost and the image boots normally. */
#include "boot.h"
#include "layout.h"
#include "hw.h"

#define RESCUE_MAGIC 0x5352434Fu    /* "OCRS" */

int request_soft(void)
{
    uint32_t a = REG(RAM_FLAG), b = REG(RAM_FLAG + 4u);
    REG(RAM_FLAG) = 0;
    REG(RAM_FLAG + 4u) = 0;
    return a == RESCUE_MAGIC && b == ~RESCUE_MAGIC;
}
