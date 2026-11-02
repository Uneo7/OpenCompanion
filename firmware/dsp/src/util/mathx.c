/* Compact float math, see include/mathx.h.
 *
 * mx_exp2: x = i + f, f in [0, 1); 2^f by its Taylor series in f ln 2 up to degree 7 (truncation error
 *   (ln 2)^8 / 8! < 1.4e-6 relative at f = 1, Horner in float); 2^i is built in the exponent field.
 * mx_log2: x = m 2^e with m in [sqrt(1/2), sqrt(2)); log2(m) = (2 / ln 2) atanh(t), t = (m - 1) / (m + 1),
 *   |t| <= 0.1716, odd series up to t^9 (truncation error < 4e-10).
 * mx_sincos: half angle h = w / 2 in [0, pi/2], sin h and cos h by Taylor series up to h^13 / h^14 (error
 *   < 6e-8 at h = pi/2), then sin w = 2 sin h cos h, cos w = 1 - 2 sin^2 h. The last form keeps 1 - cos w
 *   accurate for small w, which matters for low-frequency biquads.
 * mx_sqrt: hardware reciprocal square root estimate (RSQRSP, 8 bits) plus three Newton steps. */
#include <stdint.h>
#include <c6x.h>
#include "mathx.h"

float mx_exp2(float x)
{
    float f, p;
    int i;
    if (x < -126.0f) x = -126.0f;
    if (x > 127.0f) x = 127.0f;
    i = (int)x;
    if ((float)i > x) i--;                       /* floor */
    f = (x - (float)i) * 0.693147181f;           /* f ln 2 */
    p = 1.0f + f * (1.0f + f * (0.5f + f * (0.166666667f + f * (0.0416666667f + f * (0.00833333333f +
        f * (0.00138888889f + f * 0.000198412698f))))));
    return p * _itof((uint32_t)(i + 127) << 23);
}

float mx_log2(float x)
{
    uint32_t b;
    int e;
    float m, t, t2;
    if (!(x > 0.0f)) return -126.0f;
    b = _ftoi(x);
    e = (int)((b >> 23) & 0xFFu) - 127;
    m = _itof((b & 0x007FFFFFu) | 0x3F800000u);  /* [1, 2) */
    if (m > 1.41421356f) { m *= 0.5f; e++; }
    t = (m - 1.0f) / (m + 1.0f);
    t2 = t * t;
    return (float)e + 2.88539008f * t * (1.0f + t2 * (0.333333333f + t2 * (0.2f + t2 * (0.142857143f +
           t2 * 0.111111111f))));
}

float mx_sqrt(float x)
{
    float r;
    int k;
    if (!(x > 0.0f)) return 0.0f;
    r = _rsqrsp(x);
    for (k = 0; k < 3; k++) r = r * (1.5f - 0.5f * x * r * r);
    return x * r;
}

void mx_sincos(float w, float *s, float *c)
{
    const float h = 0.5f * w, h2 = h * h;
    float sh, ch;
    /* nested Taylor factors: 1/(2k(2k+1)) for sin, 1/((2k-1)2k) for cos (no division at run time) */
    sh = h * (1.0f - h2 * 0.166666667f * (1.0f - h2 * 0.05f * (1.0f - h2 * 0.0238095238f *
         (1.0f - h2 * 0.0138888889f * (1.0f - h2 * 0.00909090909f * (1.0f - h2 * 0.00641025641f))))));
    ch = 1.0f - h2 * 0.5f * (1.0f - h2 * 0.0833333333f * (1.0f - h2 * 0.0333333333f * (1.0f - h2 *
         0.0178571429f * (1.0f - h2 * 0.0111111111f * (1.0f - h2 * 0.00757575758f * (1.0f - h2 *
         0.00549450549f))))));
    *s = 2.0f * sh * ch;
    *c = 1.0f - 2.0f * sh * sh;
}

float mx_pow(float b, float e) { return mx_exp2(e * mx_log2(b)); }
