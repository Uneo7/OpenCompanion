/* Dynamics kernels (include/kern.h): stock soft clipper and compressor, peak follower, gain smoother, limiter
 * gain curve. Same arithmetic as the per-sample code they replace (protect_band.c laws, checked against the
 * stock code), written without branches so that the loops are software pipelined. */
#include "kern.h"

#ifdef __TI_COMPILER_VERSION__
#define ITER4 _Pragma("MUST_ITERATE(4, , 4)")
#define ABSF(x) _fabsf(x)
#else
#define ITER4
#define ABSF(x) ((x) < 0.0f ? -(x) : (x))
#endif

void k_softclip(float *restrict x, float lo, float hi, float c, int n)
{
    int i;
    ITER4
    for (i = 0; i < n; i++) {
        const float v = x[i];
        float a = ABSF(v), e;
        a = a > hi ? hi : a;
        e = a - lo;
        e = e > 0.0f ? e : 0.0f;
        a -= c * e * e;                      /* e = 0: a - 0 = a, as the stock branch */
        x[i] = v < 0.0f ? -a : a;
    }
}

/* One compressor lane: g' = P1 g (gu < P0), P1 g (1 + P0 - gu) (gu < 1 + P0 - P4), else P4 g; min(g', 1).
 * The gain is multiplicative (release = P1 g), so g = 0 is a trap: a detector input near the float range (an
 * upstream filter blowing up), or a NaN / Inf for ~80 samples (every compare false -> P4 g, -10 dB per
 * sample), drives g below 1e-38, flushed to 0 by the C67x; the channel then stays silent for good, its output
 * a finite 0 that no output check sees. audio_out.c resets the path when a gain is below -100 dB (0.2.7; the
 * stock law needs an input 100 dB over full scale for that: the most extreme legal user EQ, every band at
 * +15 dB with high Q, measured -59 dB in test/host_avance_full.c, so 0.2.6's -59 dB was too close). */
#define COMP_LANE(x, g) do { \
        const float u_ = ABSF(p2 * (x)), gu_ = (g) * u_, a_ = p1 * (g), b_ = a_ * (top - gu_), c_ = p4 * (g); \
        float n_ = gu_ < p0 ? a_ : (gu_ < knee ? b_ : c_); \
        n_ = n_ > 1.0f ? 1.0f : n_; \
        (g) = n_; \
        (x) *= n_; \
    } while (0)

void k_comp4(float *const x[4], float g[4], const float p[4], int n)
{
    float *restrict x0 = x[0], *restrict x1 = x[1], *restrict x2 = x[2], *restrict x3 = x[3];
    const float p0 = p[0], p1 = p[1], p2 = p[2], p4 = p[3];
    const float knee = 1.0f + p0 - p4, top = 1.0f + p0;
    float g0 = g[0], g1 = g[1], g2 = g[2], g3 = g[3];
    int i;
    ITER4
    for (i = 0; i < n; i++) {
        COMP_LANE(x0[i], g0);
        COMP_LANE(x1[i], g1);
        COMP_LANE(x2[i], g2);
        COMP_LANE(x3[i], g3);
    }
    g[0] = g0; g[1] = g1; g[2] = g2; g[3] = g3;
}

