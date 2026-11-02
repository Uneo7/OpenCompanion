/* Single-precision division helper of the C6000 EABI (__c6xabi_divf), replacing the RTS member (672 bytes):
 * same algorithm as the RTS for C67x (lib/src/divf_i.h): RCPSP estimate (8 bits), one single-precision and two
 * double-precision Newton steps, product in double, rounded once to float. The RTS normalises the divisor
 * exponent first; here the special cases are folded into one test on the estimate instead:
 *   b = +-0 (or flushed subnormal) -> estimate inf  -> a * inf = +-inf, NaN for 0 / 0;
 *   |b| >= 2^126 or b = inf       -> estimate 0    -> a * 0 = 0 (NaN for inf / inf);
 *   b = NaN                        -> NaN.
 * Linked before the RTS (Makefile), so the library member is not pulled in. */
#include <c6x.h>

float __c6xabi_divf(float a, float b);

float __c6xabi_divf(float a, float b)
{
    float r = _rcpsp(b);
    const unsigned e = (_ftoi(r) >> 23) & 0xFFu;     /* exponent of the estimate: 0 = zero, 0xFF = inf / NaN */
    double d;
    if (e == 0u || e == 0xFFu)
        return a * r;
    r = r * (2.0f - b * r);
    d = (double)r * (2.0 - (double)b * (double)r);
    d = d * (2.0 - (double)b * d);
    return (float)((double)a * d);
}
