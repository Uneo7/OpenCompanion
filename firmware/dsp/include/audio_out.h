#ifndef AUDIO_OUT_H
#define AUDIO_OUT_H

#include "oc.h"

void audio_out_init(void);
void audio_out_rate(void);
/* post: the 6 outputs after crossover and trims (modified in place); fade: 0/1 target of this frame.
 * bass_delay: 1 = the bass gets the satellites' 320-sample look-ahead delay (neutral preset, crossover alignment);
 * 0 = stock timing (bass 6.7 ms ahead of the satellites), used by the stock graph engine. */
void audio_out(float (*post)[FRAME_LEN], tx_buf_t *out, int open, int bass_delay);
/* 1 once the output ramp (15 ms smoothstep, audio_out.c) has reached silence: rate / preset switches wait for it. */
int  audio_out_silent(void);

#endif
