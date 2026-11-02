/* McASP0 ramp converter: pod volume knob (IVOL), bass knob (BEQ+/BEQ-) and supply sense (VRAWSEN).
 * Port of the stock code, see src/ramp.c. */
#ifndef RAMP_H
#define RAMP_H

#include <stdint.h>

/* Converted values (all updated once per McASP0 frame, 2 ms, by ramp_poll()). */
typedef struct {
    float    cal;          /* full-scale count (ramp bits to 3.3 V), 80..120, else 100 (stock obj+0xE4) */
    float    vraw_ratio;   /* VRAWSEN count / cal, not clamped (obj+0x100) */
    float    vraw;         /* supply voltage, V: 5.1 + 3.3 * ratio / 0.43 (stock 0x10010148, init 12) */
    float    ivol_ratio;   /* IVOL count / cal, clamped to 1 (obj+0xF4) */
    float    ivol_smooth;  /* one-pole smoothed IVOL ratio (obj+0xF8) */
    float    pod_pct;      /* knob position after hysteresis, 0..99 % (obj+0xFC) */
    float    beq_p;        /* BEQ+ ratio, serializer 0 (obj+0xE8) */
    float    beq_n;        /* BEQ- ratio, serializer 1 (obj+0xEC) */
    float    bass_ratio;   /* (BEQ- - BEQ+) / (BEQ- + BEQ+) (obj+0xF0) */
    float    bass;         /* bass knob value, -5/3..+5/3 (stock 0x10010104, init 0) */
    uint32_t count[4];     /* last raw counts of serializers 0..3 (diagnostics, not in stock) */
    uint32_t frames;       /* processed McASP0 frames */
    uint32_t cal_frames;   /* calibration frames seen */
    uint8_t  ivol_ok;      /* IVOL reading plausible (volume_curve.c ivol_plausible): 0 = floating, no pod */
} ramp_state_t;

extern ramp_state_t g_ramp;

void     ramp_init(void);           /* McASP0 receive + dMAX (LoMAX event 5) + INT9, stock 0x10019418 */
void     ramp_on_frame_start(void); /* INT9 handler body, stock 0x1001A360 */
void     ramp_on_dma_irq(void);     /* from the INT8 handler: TCC 8 = one McASP0 frame received */
void     ramp_poll(void);           /* main loop: converts the last received frame (stock task 0x100167E4) */

#endif
