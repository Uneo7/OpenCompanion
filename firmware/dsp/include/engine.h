#ifndef ENGINE_H
#define ENGINE_H

#include "oc.h"

void  engine_init(void);
/* Once per frame, before engine_run(): preset changes. Returns the output fade target (0 while switching). */
float engine_frame_gate(void);
/* Inputs of the frame into the program's input wires (x[0..7]: USB L, R, C, LFE, Ls, Rs, AUX L, AUX R; USB
 * at the program's input level, AUX at full scale = 1). Returns 1 / the USB input level. */
float engine_fill(const rx_buf_t *in, float *x[8]);
/* Program -> post[OUT_*] (before the protection). usb_gain: per-sample USB mute ramp. */
void  engine_run(const float *usb_gain, float (*post)[FRAME_LEN]);
int   engine_bass_delay(void);          /* the bass gets the look-ahead delay (neutral preset) */
void  engine_profile_swap(void);        /* structural change: switch behind the output ramp, after the batch */
void  engine_commit(void);              /* the batch is complete (OPX): no settle wait */
void  engine_recover(void);             /* states back to rest after a non-finite sample */
int   engine_swap_pending(void);

#endif
