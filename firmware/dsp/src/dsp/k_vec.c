#include "kern.h"

#ifdef __TI_COMPILER_VERSION__
#define ITER4 _Pragma("MUST_ITERATE(4, , 4)")
#define ABSF(x) _fabsf(x)
#else
#define ITER4
#define ABSF(x) ((x) < 0.0f ? -(x) : (x))
#endif

/* Any n >= 1 (no trip-count hint: also the copies of the short delay lines, k_copy). */
void k_scale(float *restrict y, const float *restrict x, float t, int n)
{
    int i;
    for (i = 0; i < n; i++) y[i] = x[i] * t;
}

void k_mac(float *restrict y, const float *restrict x, float t, int n)
{
    int i;
    ITER4
        for (i = 0; i < n; i++) y[i] += x[i] * t;
}

void k_scalev(float *restrict y, const float *restrict x, const float *restrict g, int n)
{
    int i;
        for (i = 0; i < n; i++) y[i] = x[i] * g[i];
}

/* Meters, limiter, powers: 2 partial sums (rounding differs from a plain loop in the last bits). n even. */
float k_peaksq(const float *restrict x, int n, float *sumsq)
{
    float p0 = 0.0f, p1 = 0.0f, s0 = 0.0f, s1 = 0.0f;
    int i;
    ITER4
    for (i = 0; i < n; i += 2) {
        const float a = x[i], b = x[i + 1], ma = ABSF(a), mb = ABSF(b);
        p0 = ma > p0 ? ma : p0;
        p1 = mb > p1 ? mb : p1;
        s0 += a * a;
        s1 += b * b;
    }
    *sumsq = s0 + s1;
    return p0 > p1 ? p0 : p1;
}

float k_power(const float *x, int n)
{
    float sq;
    k_peaksq(x, n, &sq);
    return sq;
}

void k_smooth(float *restrict g, float *cur, float *prev, float t, float b, float p, int n)
{
    float v = *cur, q = *prev;
    int i;
    ITER4
    for (i = 0; i < n; i++) {
        v = b * (t + q) + p * v;
        q = t;
        g[i] = v;
    }
    *cur = v;
    *prev = q;
}

void k_i2f(float *restrict y, const int32_t *restrict x, float s, int n)
{
    int i;
        for (i = 0; i < n; i++) y[i] = (float)x[i] * s;
}

/* y = x h (kept for the output meters), then 24-bit PCM: clamp to +-0.999, x 2^31, low byte cleared. */
void k_pcm(int32_t *restrict pcm, float *restrict x, const float *restrict h, int n)
{
    int i;
        for (i = 0; i < n; i++) {
        float v = x[i] * h[i];
        x[i] = v;
        v = (v < 0.999f && v > -0.999f) ? v : (v >= 0.999f ? 0.999f : (v <= -0.999f ? -0.999f : 0.0f));   /* NaN -> 0 */
        pcm[i] = ((int32_t)(v * 2147483648.0f)) & (int32_t)0xFFFFFF00;
    }
}

void k_macv(float *restrict y, const float *restrict x, const float *restrict g, int n)
{
    int i;
        for (i = 0; i < n; i++) y[i] += x[i] * g[i];
}

/* Circular delay in at most two straight segments per pass (no per-sample wrap test). */
static void seg(float *restrict line, const float *restrict x, float *restrict y, int m)
{
    int i;
    for (i = 0; i < m; i++) {
        const float v = line[i];
        line[i] = x[i];
        y[i] = v;
    }
}

void k_delay(float *line, int len, int *pos, const float *x, float *y, int n)
{
    int p = *pos;
    while (n > 0) {
        int m = len - p;
        if (m > n) m = n;
        seg(line + p, x, y, m);
        x += m;
        y += m;
        n -= m;
        p += m;
        if (p == len) p = 0;
    }
    *pos = p;
}

