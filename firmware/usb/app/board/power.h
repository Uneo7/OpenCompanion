#ifndef POWER_H
#define POWER_H

#include <stdint.h>

extern __bit usb_present;                  /* P1.2: USB 5 V seen through R6011/R6013 */
extern __bit usb_suspended;                /* bus suspend (VECINT 0x16), cleared by resume / reset */
extern __bit usb_active;                   /* USB host active: present, configured, not suspended, DSP link not
                                            * initialising (stock bit 0x07, without "configured"); = DSPINT low */

void power_early_init(void);               /* first thing in main: clocks and pin states */
void power_audio_on(void);                 /* VREN, then DSP out of reset, then pod SENSOR on */
void power_poll_usb(void);                 /* USB presence -> pull-up, USB state -> DSPINT (P1.6) */

#endif /* POWER_H */
