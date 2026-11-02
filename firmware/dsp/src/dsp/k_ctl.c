/* Block loops of small weight (a few calls per frame, or only on parameter changes), built for size (not
 * pipelined: about 2x the cycles of a pipelined loop, < 1 % of the frame in total). include/kern.h. */
#include "kern.h"

#define ABSF(x) ((x) < 0.0f ? -(x) : (x))
#define ITER4

void k_zero(float *restrict y, int n)
{
    int i;
    ITER4
    for (i = 0; i < n; i++) y[i] = 0.0f;
}

/* Output ramp of one block: g[i] = (a + (b - a) (i + 1) / n) S(p0 + (p1 - p0) (i + 1) / n), S(t) = 3 t^2 - 2 t^3
 * (smoothstep: zero slope at both ends, like a raised cosine). */
void k_fade(float *g, float a, float b, float p0, float p1, int n)
{
    const float da = (b - a) / (float)n, dp = (p1 - p0) / (float)n;
    int i;
    for (i = 0; i < n; i++) {
        const float t = p0 + dp * (float)(i + 1);
        g[i] = (a + da * (float)(i + 1)) * (t * t * (3.0f - 2.0f * t));
    }
}

float k_hermite(float *restrict g, float s0, float m0, float c2, float c3, float dt, int n)
{
    float mn = 1.0f;
    int i;
    ITER4
    for (i = 0; i < n; i++) {
        const float t = (float)(i + 1) * dt;
        const float v = s0 + t * (m0 + t * (c2 + t * c3));
        g[i] = v;
        mn = v < mn ? v : mn;
    }
    return mn;
}

float k_decay_peak(const float *restrict x, float p, float d, int n)
{
    int i;
    ITER4
    for (i = 0; i < n; i++) {
        const float a = ABSF(x[i]);
        p *= d;
        p = a > p ? a : p;
    }
    return p;
}
