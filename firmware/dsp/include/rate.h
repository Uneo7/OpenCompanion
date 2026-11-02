/* Sampling rate: 48 kHz only (since 0.2.4; the TAS 0.5.1 dropped 44.1 kHz). The 'R' frame [rate, 24-bit
 * big-endian Hz][layout] is still accepted; its rate is ignored (a 44.1 kHz 'R' changes nothing: the DSP stays
 * at 48 kHz). Its layout byte (TAS >= 0.4.0: bit0 stream open, bit1 2.1 mode) selects the input layout; a layout
 * change closes the output ramp, switches while silent, holds the mute for RATE_HOLD_FRAMES and reopens.
 * The former 44.1 kHz support (bilinear re-mapping of the stored 48 kHz sections, re-timed smoothers and
 * scaled block counts, FS4 / FS8) is gone; the modules' *_rate() functions only set their 48 kHz constants. */
#ifndef RATE_H
#define RATE_H

#include <stdint.h>
#include "biquad.h"

#define RATE_BASE        48000.0f
#define RATE_HOLD_FRAMES 50u              /* ~100 ms muted around a layout switch */
#define g_fs             RATE_BASE        /* the sampling rate, Hz */

/* LAYOUT_51: 6 x 16-bit (L, R, C, LFE, Ls, Rs). LAYOUT_21: TAS "2.1 mode", 24-bit stereo (L, R): the sub is
 * (L + R) / 2 and each satellite plays one signal on both drivers ("routage 2.1", neutral preset). */
enum { LAYOUT_NONE, LAYOUT_51, LAYOUT_21 };
extern uint8_t g_layout;                  /* current input layout (LAYOUT_*) */

void  rate_init(void);
#define rate_request(hz) ((void)(hz))     /* 'R' frame rate: ignored, 48 kHz only */
void  layout_request(uint8_t layout);     /* 'R' frame byte 4 (bit1 = 2.1 mode) */
float rate_frame(void);                   /* once per frame: runs a layout switch; returns the output fade target 0/1 */
#endif
