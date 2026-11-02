/* Biquad cascade using the Bose DSP coefficient format:
 * H(z) = g * prod (1 + b1 z^-1 + b2 z^-2) / (1 - a1 z^-1 - a2 z^-2)   (an all-zero section is identity) */
#ifndef BIQUAD_H
#define BIQUAD_H

#define BIQUAD_MAX_SECTIONS 14      /* 12 user bands + the 2 crossover sections (eq.c) */

typedef struct { float b1, b2, a1, a2; } biquad_coef_t;

typedef struct {
    float g;
    int nsec;
    biquad_coef_t c[BIQUAD_MAX_SECTIONS];
    float z[BIQUAD_MAX_SECTIONS][2];     /* state (transposed direct form II) */
} biquad_t;

void biquad_init(biquad_t *bq, float g, const biquad_coef_t *sections, int nsec);

/* Four cascades in one pass of the 4-lane kernel (null bq[l] = idle lane, out[l] then scratch). */
void biquad_run4(biquad_t *const bq[4], const float *const in[4], float *const out[4], int n);

#endif
