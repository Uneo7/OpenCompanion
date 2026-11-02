#ifndef SERVO_H
#define SERVO_H

#include <stdint.h>

extern __data uint8_t servo_resyncs;       /* DMA re-alignments since boot (stock P1.7 / BADDATA events) */
extern __data int16_t servo_off;           /* ACG1 word = 0x61A80F (24.576 MHz) + 8 x servo_off */
extern __data int16_t servo_acc;           /* accumulated phase error, MCLK cycles */

void servo_init(void);                     /* ACG1 = 24.576 MHz (48 kHz only since 0.5.1), servo reset */
void servo_reset(void);
void servo_sof(void);                      /* call on every SOF while configured */

#endif /* SERVO_H */
