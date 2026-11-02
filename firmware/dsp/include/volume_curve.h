#ifndef VOLUME_CURVE_H
#define VOLUME_CURVE_H

#include <stdint.h>

typedef struct {
    int   n;               /* number of inner breakpoints */
    float bp[5];           /* top, inner breakpoints, bottom clamp */
    float slope[4];        /* n + 1 cumulative slopes, dB per percent */
    float floor_db;
} volume_curve_t;

extern const volume_curve_t VC_STOCK_POD;   /* object 0x10010564 */

float volume_curve_db(const volume_curve_t *c, float pct);
float volume_pct_from_uac(int16_t v);       /* 'V' value -> stock percentage */
float pod_filter(float ratio, float *smooth, float prev);   /* stock knob filter -> pod volume 0..99 % */
int   knob_step(float p, int knob, int *dir);              /* reported knob: direction-aware hysteresis */
int   ivol_plausible(uint32_t count, float cal, uint32_t *last, int *score);   /* 0 = floating IVOL (no pod) */

#endif
