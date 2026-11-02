/* Stock spatial control, class 0x10027978 (process 0x10027978..0x100289C4, instance 0x10010AC0), decoded in
 * re/dsp/notes/spatial.md and validated against the stock code run in a C67x+ emulator (re/dsp/spatial_ref.py,
 * re/dsp/c6emu.py). Once per 2 ms block it turns the block powers of the high-passed (~150 Hz, 0x10010A1C) left,
 * right, sum and difference signals into the 12 gains of the 2->6 matrix 0x10010B58:
 *   direct L = dL L, direct R = dR R, array L = sqrt(1 - dL^2) L, array R = sqrt(1 - dR^2) R, third paths 0
 *   (instance word 24 = 0; mode 'N' mutes them anyway with mixer 0x10010C54).
 * Envelopes (6: L, R, S = L+R, D = L-R, max(L, R), max(S, D)), per block:
 *   v = max(rms, 0.367879 v_prev)                  peak hold
 *   coherence r2 = (x1 + max(S,D) - |L,R|) / (x1 + eps + L + R - |L,R|),  x1 = max(max(L,R)/max(S,D) - 0.9, 0)
 *   fast = 0.31606 (v + v_prev) + 0.367879 fast    (1 block)     slow = 0.00530499 (v + v_prev) + 0.98939 slow
 *   env = r2^2 fast + (1 - r2^2) slow
 * Steering, with g = 1 / (max(L,R) + eps2) and c = 2 q - 1, q = (eps + max(S,D) - |L,R|) / (eps + L + R - |L,R|)
 * (1 = mono / anti-phase / one side only, -1 = decorrelated):
 *   width w: cs = 0.999 cs + 0.001 c; w = clamp(0.5 + 2 cs, 0.5, 1.5); w = 1.5 if c > 0.5, 0.5 if c < 0
 *   d = g (S - x c+ - max(L,R) (1 - c+)), x = L (resp. R), c+ = max(c, 0)
 *   if D > S: d -= clamp01(g c+ (1 - 2 x))
 *   d = clamp01(0.95 clamp01(d)); d = clamp01(1 + w (d - 1))
 * Mono gives d = 0.9245 (array -8.4 dB), decorrelated stereo d = 0.697 (w 0.5; array -2.9 dB), a single side
 * d = 0 on that side (all of it into the array), anti-phase and silence d = 0. Rates: per-block constants kept as stock (2.18 ms blocks at
 * 48 kHz only). */
#include "bose_graph.h"
#include "mathx.h"

#define EPS      3.16228e-4f    /* W20 */
#define EPS2     3.16228e-5f    /* W21 */
#define HOLD     0.367879f      /* W15 */
#define FAST_A   0.367879f      /* -W16 */
#define FAST_B   0.31606f       /* W18 */
#define SLOW_A   0.98939f       /* -W17 */
#define SLOW_B   0.00530499f    /* W19 */
#define DIRECT   0.95f          /* W23 (20 log10 < 0: direct paths scaled) */
#define CS_POLE  0.999f         /* W37 */
#define W_LO     0.5f           /* W31 (width at cs <= 0) */
#define W_HI     1.5f           /* W32 */
#define W_X1     0.5f           /* W30 (cs giving W_HI); W29 = 0 */

bose_spatial_t g_spatial;

void bose_spatial_init(void)
{
    int k;
    for (k = 0; k < 6; k++)
        g_spatial.hold[k] = g_spatial.fast[k] = g_spatial.slow[k] = 0.0f;
    g_spatial.cs = 0.0f;
    g_spatial.width = W_HI;
}

static float clamp01(float x) { return x < 0.0f ? 0.0f : (x > 1.0f ? 1.0f : x); }
static float maxf(float a, float b) { return a > b ? a : b; }
static float sqrt0(float x) { return x > 0.0f ? mx_sqrt(x) : 0.0f; }

static float direct(float x, const float *e, float g, float cp, float w)
{
    float t = clamp01(g * cp * (1.0f - 2.0f * x));
    float d = g * ((e[2] - x * cp) - e[4] * (1.0f - cp));
    if (e[3] > e[2]) d -= t;
    d = clamp01(DIRECT * clamp01(d));
    return clamp01(1.0f + w * (d - 1.0f));
}

void bose_spatial_block(const float pow4[4], float g12[12])
{
    bose_spatial_t *s = &g_spatial;
    float v[6], e[6], r1, x1, nlr, r2, k2, g, q, c, w, dl, dr;
    int k;
    for (k = 0; k < 4; k++) {
        float h = s->hold[k] * HOLD;
        v[k] = maxf(sqrt0(pow4[k]), h);
    }
    v[4] = maxf(v[0], v[1]);
    v[5] = maxf(v[2], v[3]);
    r1 = (EPS + v[4]) / (EPS + v[5]);
    x1 = maxf(r1 - 0.9f, 0.0f);
    nlr = sqrt0(v[0] * v[0] + v[1] * v[1]);
    r2 = (x1 + (v[5] - nlr)) / (x1 + (EPS + (v[1] + v[0] - nlr)));
    k2 = r2 * r2;
    for (k = 0; k < 6; k++) {
        float in = v[k] + s->hold[k];
        s->hold[k] = v[k];
        s->fast[k] = FAST_B * in + FAST_A * s->fast[k];
        s->slow[k] = SLOW_B * in + SLOW_A * s->slow[k];
        e[k] = s->slow[k] * (1.0f - k2) + s->fast[k] * k2;
    }
    g = 1.0f / (EPS2 + e[4]);
    nlr = sqrt0(e[0] * e[0] + e[1] * e[1]);
    q = (EPS + (e[5] - nlr)) / (EPS + (e[1] + e[0] - nlr));
    c = 1.0f - 2.0f * (1.0f - q);
    s->cs = CS_POLE * s->cs + c * (1.0f - CS_POLE);
    w = s->cs * ((W_HI - W_LO) / (EPS + W_X1)) + W_LO;
    if (!(w > W_LO)) w = W_LO;
    if (w > W_HI) w = W_HI;
    if (c > W_X1) w = W_HI;
    else if (c < 0.0f) w = W_LO;
    s->width = w;
    c = maxf(c, 0.0f);
    dl = direct(e[0], e, g, c, w);
    dr = direct(e[1], e, g, c, w);
    for (k = 0; k < 12; k++) g12[k] = 0.0f;
    g12[0] = dl;
    g12[3] = dr;
    g12[4] = sqrt0(1.0f - dl * dl);
    g12[7] = sqrt0(1.0f - dr * dr);
}
