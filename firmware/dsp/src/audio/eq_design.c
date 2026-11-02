#include "eq.h"
#include "oc.h"
#include "rate.h"
#include "mathx.h"

#define PI_F        3.14159265358979f

int eq_design_band(const eq_band_t *b, float fs, biquad_coef_t *sec, float *gain)
{
    float w0, cw, sw, alpha, A, sqA, b0, b1, b2, a0, a1, a2, q;
    if (b->type == BAND_OFF || b->type >= BAND_TYPE_COUNT) return 0;   /* off, disabled */
    w0 = 2.0f * PI_F * (float)b->freq_hz / fs;
    if (w0 > 0.99f * PI_F) return 0;                       /* at or above Nyquist */
    mx_sincos(w0, &sw, &cw);
    q = (float)b->q_milli / 1000.0f;
    alpha = sw / (2.0f * q);
    A = mx_exp2((float)b->gain_cdb * (0.01f * 0.5f * MX_LOG2_10_20));   /* 10^(dB/40) */
    sqA = mx_sqrt(A);
    switch (b->type) {
    case BAND_PEAK:
        b0 = 1.0f + alpha * A; b1 = -2.0f * cw; b2 = 1.0f - alpha * A;
        a0 = 1.0f + alpha / A; a1 = -2.0f * cw; a2 = 1.0f - alpha / A;
        break;
    case BAND_LOW_SHELF:
        b0 = A * ((A + 1.0f) - (A - 1.0f) * cw + 2.0f * sqA * alpha);
        b1 = 2.0f * A * ((A - 1.0f) - (A + 1.0f) * cw);
        b2 = A * ((A + 1.0f) - (A - 1.0f) * cw - 2.0f * sqA * alpha);
        a0 = (A + 1.0f) + (A - 1.0f) * cw + 2.0f * sqA * alpha;
        a1 = -2.0f * ((A - 1.0f) + (A + 1.0f) * cw);
        a2 = (A + 1.0f) + (A - 1.0f) * cw - 2.0f * sqA * alpha;
        break;
    case BAND_HIGH_SHELF:
        b0 = A * ((A + 1.0f) + (A - 1.0f) * cw + 2.0f * sqA * alpha);
        b1 = -2.0f * A * ((A - 1.0f) + (A + 1.0f) * cw);
        b2 = A * ((A + 1.0f) + (A - 1.0f) * cw - 2.0f * sqA * alpha);
        a0 = (A + 1.0f) - (A - 1.0f) * cw + 2.0f * sqA * alpha;
        a1 = 2.0f * ((A - 1.0f) - (A + 1.0f) * cw);
        a2 = (A + 1.0f) - (A - 1.0f) * cw - 2.0f * sqA * alpha;
        break;
    case BAND_HPF:
        b0 = (1.0f + cw) * 0.5f; b1 = -(1.0f + cw); b2 = b0;
        a0 = 1.0f + alpha; a1 = -2.0f * cw; a2 = 1.0f - alpha;
        break;
    case BAND_LPF:
        b0 = (1.0f - cw) * 0.5f; b1 = 1.0f - cw; b2 = b0;
        a0 = 1.0f + alpha; a1 = -2.0f * cw; a2 = 1.0f - alpha;
        break;
    default: /* BAND_NOTCH */
        b0 = 1.0f; b1 = -2.0f * cw; b2 = 1.0f;
        a0 = 1.0f + alpha; a1 = -2.0f * cw; a2 = 1.0f - alpha;
        break;
    }
    if ((b0 < 1e-9f && b0 > -1e-9f) || (a0 < 1e-9f && a0 > -1e-9f)) return 0;
    /* Bose form: g * (1 + b1' z^-1 + b2' z^-2) / (1 - a1' z^-1 - a2' z^-2) */
    sec->b1 = b1 / b0;
    sec->b2 = b2 / b0;
    sec->a1 = -a1 / a0;
    sec->a2 = -a2 / a0;
    *gain *= b0 / a0;
    return 1;
}

int eq_compile(const eq_band_t *bands, biquad_coef_t *sec, float *g)
{
    int b, n = 0;
    *g = 1.0f;
    for (b = 0; b < EQ_MAX_BANDS; b++) n += eq_design_band(&bands[b], g_fs, &sec[n], g);
    return n;
}

