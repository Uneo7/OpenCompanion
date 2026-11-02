/*
 * OpenCompanion firmware for the TAS1020B USB controller of the Bose Companion 5.
 * Entry point: oc_main(), jumped to by sys/crt0.asm (not main(), so that SDCC does not generate its own, larger
 * vector table). Only the Timer 1 interrupt is used; everything else is polled. Blocking work calls main_idle(),
 * which keeps USB serviced; USB callbacks never block.
 */
#include <8051.h>

#include "dsp_link.h"
#include "ee_area.h"
#include "i2c.h"
#include "pod.h"
#include "pod_sync.h"
#include "power.h"
#include "protocol.h"
#include "rom.h"
#include "stream.h"
#include "tick.h"
#include "uac_notify.h"
#include "usb_core.h"

void main_idle(void)
{
    usb_poll();
    pod_poll();
    uac_notify_poll();
}

/* Off the bus 200 ms so that the host notices; the main loop reconnects (power_poll_usb). */
static void usb_restart(void)
{
    usb_disconnect();
    stream_set_alt(0);
    usb_init();
    delay_ms(200);
}

static void run_action(void)
{
    delay_ms(50);                               /* the host collects the reply */
    if (protocol_action == ACTION_MODE) {
        protocol_action = ACTION_NONE;
        usb_restart();                          /* no reboot: DSP link kept */
        return;
    }
    if (protocol_action == ACTION_DFU) {
        rom_enter_dfu();
    }
    rom_reboot();
}

void oc_main(void)
{
    power_early_init();
    tick_init();
    i2c_init();
    stream_init();
    ee_area_check();

    usb_restart();                              /* the ROM DFU left the device connected with its own identity */
    power_poll_usb();                           /* connects only if USB 5 V is present (P1.2) */

    power_audio_on();
    dsp_link_start();

    for (;;) {
        main_idle();
        power_poll_usb();
        protocol_poll();
        dsp_link_poll();
        pod_sync_poll();
        if (protocol_action != ACTION_NONE) {
            run_action();
        }
    }
}
