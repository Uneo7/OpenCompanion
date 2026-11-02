/*
 * TAS1020B test firmware: vendor HID (1209:0001), read-only memory access (boot ROM dump) and the guarded
 * RESTORE_HEADER. Loaded into RAM by the ROM DFU.
 */
#include "board.h"
#include "commands.h"
#include "tas1020b.h"
#include "test_usb.h"
#include "usb_core.h"

void main(void)
{
    board_init();

    /* The ROM DFU left the device connected: drop off the bus long enough for the host to notice. */
    usb_disconnect();
    delay_ms(200);
    usb_init();
    usb_connect();

    for (;;) {
        usb_poll();
        if (reboot_requested && (EP_DCNTX(IEP_BASE(1)) & EPDCNT_NACK)) {
            delay_ms(20);
            reboot_to_rom();
        }
    }
}
