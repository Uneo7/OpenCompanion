/* Biquad section on 4 channels at once (include/kern.h). One channel alone is bound by its recurrence (ii = 16
 * cycles per sample, compiler feedback); 4 independent channels share those cycles (ii = 12 for the 4). The
 * recurrence is written (b1 x + z1) + a1 y so that only one multiply and one add are on the loop-carried path.
 * Transposed direct form II, Bose coefficient convention: H = (1 + b1 z^-1 + b2 z^-2) / (1 - a1 z^-1 - a2 z^-2)
 * (the section gain is applied separately); an all-zero section is the identity. */
#include "kern.h"

void k_sec4(float *const y[4], const float *const c[4], float *const z[4], int n)
{
    float *restrict y0 = y[0], *restrict y1 = y[1], *restrict y2 = y[2], *restrict y3 = y[3];
    const float b10 = c[0][0], b20 = c[0][1], a10 = c[0][2], a20 = c[0][3];
    const float b11 = c[1][0], b21 = c[1][1], a11 = c[1][2], a21 = c[1][3];
    const float b12 = c[2][0], b22 = c[2][1], a12 = c[2][2], a22 = c[2][3];
    const float b13 = c[3][0], b23 = c[3][1], a13 = c[3][2], a23 = c[3][3];
    float p0 = z[0][0], q0 = z[0][1], p1 = z[1][0], q1 = z[1][1];
    float p2 = z[2][0], q2 = z[2][1], p3 = z[3][0], q3 = z[3][1];
    int i;
#ifdef __TI_COMPILER_VERSION__
#pragma MUST_ITERATE(4, , 4)
#endif
    for (i = 0; i < n; i++) {
        const float x0 = y0[i], x1 = y1[i], x2 = y2[i], x3 = y3[i];
        const float v0 = x0 + p0, v1 = x1 + p1, v2 = x2 + p2, v3 = x3 + p3;
        p0 = (b10 * x0 + q0) + a10 * v0;
        p1 = (b11 * x1 + q1) + a11 * v1;
        p2 = (b12 * x2 + q2) + a12 * v2;
        p3 = (b13 * x3 + q3) + a13 * v3;
        q0 = b20 * x0 + a20 * v0;
        q1 = b21 * x1 + a21 * v1;
        q2 = b22 * x2 + a22 * v2;
        q3 = b23 * x3 + a23 * v3;
        y0[i] = v0;
        y1[i] = v1;
        y2[i] = v2;
        y3[i] = v3;
    }
    z[0][0] = p0; z[0][1] = q0; z[1][0] = p1; z[1][1] = q1;
    z[2][0] = p2; z[2][1] = q2; z[3][0] = p3; z[3][1] = q3;
}
