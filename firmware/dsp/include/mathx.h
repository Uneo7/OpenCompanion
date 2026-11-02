/* Compact float math (src/mathx.c), replaces powf / log10f / logf / sinf / cosf / sqrtf of the RTS library
 * (~5 KB of code with their frexpf / ldexpf / modff helpers) in an image that must fit the EEPROM body.
 * Accuracy (checked on the host against libm, see the comments in mathx.c): relative error below 2e-6 for
 * mx_exp2 / mx_db_to_lin, absolute error below 1e-6 for mx_log2 and below 2e-7 for mx_sincos. Used for
 * gains, filter design and control laws, never for per-sample DSP. */
#ifndef MATHX_H
#define MATHX_H

float mx_exp2(float x);                      /* 2^x, x in [-126, 127] (clamped) */
float mx_log2(float x);                      /* log2(x), x > 0; returns -126 for x <= 0 */
float mx_sqrt(float x);                      /* x >= 0 */
void  mx_sincos(float w, float *s, float *c);/* w in [0, pi] */
float mx_pow(float b, float e);              /* b > 0 */

#define MX_LOG2_10_20  0.166096404744f       /* log2(10) / 20 */
#define MX_LOG2_10_10  0.332192809489f       /* log2(10) / 10 */
#define MX_DB_PER_LOG2 6.02059991328f        /* 20 log10(2) */

static inline float mx_db_to_lin(float db) { return mx_exp2(db * MX_LOG2_10_20); }   /* 10^(dB/20) */
static inline float mx_lin_to_db(float x) { return mx_log2(x) * MX_DB_PER_LOG2; }   /* 20 log10(x) */
static inline float mx_pow_db(float p) { return mx_log2(p) * (0.5f * MX_DB_PER_LOG2); } /* 10 log10(p) */

#endif
