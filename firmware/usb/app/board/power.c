/*
 * Board pins (stock firmware and service manual):
 *   GLOBCTL.VREN  (pin 42) VRAWON: Vraw pre-regulator and +10 V (amplifiers)
 *   GLOBCTL.RESET (pin 41) DSPRESET\, active low
 *   P1.2 (pin 25) USB 5 V presence; the pull-up must be removed when it is absent
 *   P1.3 (pin 26) pod MUTE, 1 = muted
 *   P1.5 (pin 29) SENSOR switch, 1 = off. [I] Inferred from the stock 80 ms pulse before standby (0x125F).
 *   P1.6 (pin 30) DSPINT / USBINT\ handshake with the DSP
 *   P1.7 (pin 31) BADDATA to the DSP
 *   P3.4 (pin 18) driven low by the stock main (0x0B42); function unknown, reproduced as is.
 */
#include <8051.h>

#include "ee_area.h"
#include "power.h"
#include "tas1020b.h"
#include "tick.h"
#include "usb_core.h"

extern __bit dsp_link_busy;

__bit usb_present;
__bit usb_suspended;
__bit usb_active;

void power_early_init(void)
{
    EA = 0;
    IE = 0;
    P1_3 = 1;                                   /* quasi-bidirectional port: 1 = input */
    P1_2 = 1;
    P1_6 = 1;
    P1_7 = 0;
    P1_5 = 1;                                   /* SENSOR off until power_audio_on() (>= 240 ms): after a DFU
                                                 * reload without power cycle, pod_detect() needs the sensor to power
                                                 * up again, which forces the line high ("muted"). */
    GLOBCTL = GLOBCTL_MCUCLK | GLOBCTL_LPWR;    /* stock 0x0B32: 0x84 (VREN = RESET = 0: DSP in reset, power off) */
    P3_4 = 0;
}

void power_audio_on(void)
{
    GLOBCTL |= GLOBCTL_VREN;                    /* stock 0x0B81 */
    delay_ms(40);
    GLOBCTL |= GLOBCTL_RESET;                   /* stock 0x0B8F: DSPRESET\ released, the DSP boots (~2.8 s) */
    P1_5 = 0;                                   /* stock 0x0B96: SENSOR on */
}

/* DSPINT (P1.6) low = USB host active: the DSP switches to HOST mode (OS volume is master), LOCAL (pod knob) when
 * high. Unlike stock, "active" also requires SET_CONFIGURATION, so a 5 V supply without a host keeps the knob
 * local. */
void power_poll_usb(void)
{
    usb_present = P1_2;
    if (usb_present && ee_area_ok) {            /* no valid data area = no descriptors: stay off the bus */
        USBCTL |= USBCTL_CONT | USBCTL_FEN;     /* FEN: also after a connect from the "absent" state */
    } else {
        USBCTL &= (uint8_t)~USBCTL_CONT;
    }
    usb_active = !dsp_link_busy && usb_present && !usb_suspended && usb_configuration;
    P1_6 = !usb_active;                         /* low = USB active (USBINT\), released (input) otherwise */
}
