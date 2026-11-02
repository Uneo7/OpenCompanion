/* Stock speaker protection, ported from the stock graph (re/dsp/out/graph.txt lines 92-138):
 *  - protect_band.c:  satellite band soft clippers (~250 Hz band, containers 0x10011450 / 0x100115C4 /
 *                     0x10011738 / 0x100118AC), satellite and bass compressors (class 0x1002CB4C), bass soft
 *                     clipper (0x10011C24);
 *  - protect_limit.c: satellite look-ahead peak limiter (class 0x1002B020 + 320-sample delays 0x1001AD94 +
 *                     multipliers 0x1001CB2C), ceiling 0.95;
 *  - protect_supply.c: VRAW-dependent final gain (0x1001BB6C) and the supply / clip fault latch
 *                     (0x1001B820 + 0x10015910).
 * Order on each output, as in stock: [crossover, trims] -> band clipper -> compressor -> look-ahead limiter
 * (satellites) / bass clipper -> final gain (supply ceiling x thermal gain) -> OpenCompanion -6 dBFS limiter.
 * Levels: float +-1.0 = DAC full scale, as assumed for the stock graph (level_detect.c); the thresholds are
 * therefore DAC-referred and do not depend on the profile or the volume law. */
#ifndef PROTECT_H
#define PROTECT_H

#include <stdint.h>

#define LIM_DELAY 320                     /* samples (stock delays 0x10011B04..0x10011BDC) */

typedef struct {
    float    vs;                          /* smoothed VRAW, V */
    float    ceiling;                     /* final gain from VRAW (0.89145 above 10.5 V) */
    float    boost_db;                    /* pre-gain compensation, dB (>= 0) */
    float    hold;                        /* clip / heat flag peak hold */
    uint32_t c1, c2;                      /* fault counters, blocks */
    uint8_t  fault;                       /* latched (mute + standby request + 'S' fault byte) */
} supply_t;
extern supply_t g_supply;

void supply_init(void);
void supply_rate(void);
void supply_frame(void);                          /* before the gains of the frame are computed */
void supply_detect(float (*out)[96], int n);      /* after the final gain */

void  protect_init(void);
void  protect_rate(void);
void  protect_sats(float (*x)[96]);               /* inner L, inner R, outer L, outer R; in place */
void  protect_basses(float *b0, float *b1);        /* bass A, bass B; in place */
void  protect_limit(float (*sat)[96], int n);     /* 4 satellites, in place (delayed by LIM_DELAY) */
void  protect_limit_init(void);
void  protect_delay_bass(int b, float *x, int n);        /* same delay on the bass, keeps the crossover alignment */
void  protect_comp_reset(void);                   /* compressor gains = 1 (switch behind the ramp) */
float protect_comp_gain(void);                    /* lowest compressor gain (end of the last frame) */
float protect_lim_gain(void);                     /* lowest look-ahead limiter gain of the last frame */

#endif
