/* Click-free biquad cascades of the user EQ banks (include/eq.h, eq_chain_t).
 * A new coefficient set is not swapped in: the running coefficients glide to it once per frame, along a
 * smoothstep, over EQ_GLIDE_FRAMES frames, the filter states running on. Every intermediate second-order
 * section is stable: the stability region of (a1, a2) is a triangle, which is convex, so a linear path between
 * two stable sections stays inside. Sections that appear or disappear glide from / to the identity section
 * (all-zero coefficients, Bose convention); a vanished section is dropped at the end of the glide (its states
 * are 0 two samples after it became the identity). A new target during a glide restarts it from the current
 * coefficients: the last write wins, without a jump. Nothing is done while no glide runs.
 * Only for sets of the same structure: when the section count changes (a band switched on / off, a profile
 * load), the sections are re-packed and section k of the old set has nothing to do with section k of the new
 * one: gliding between them passes through bass-boosting filters (host test test/host_profile_swap.c: corner
 * 140-198 against 2-5 behind the ramp). chain_set() reports it and eq.c switches behind the output ramp.
 * The cascade gain g glides in the log domain: a low-pass with a tiny g (stock crossover: 2.8e-8) whose poles
 * approach z = 1 must not see g still near 1 while its denominator is already small (linear interpolation of g
 * gave +100 dB at DC halfway). Section coefficients glide linearly (convex stability region). */
#include <string.h>
#include "eq.h"
#include "kern.h"
#include "oc.h"
#include "mathx.h"

void chain_init(eq_chain_t *c, float g, const biquad_coef_t *sec, int n)
{
    biquad_init(&c->f, g, sec, n);
    memcpy(c->tc, c->f.c, sizeof(c->tc));
    c->tg = g;
    c->tn = (uint8_t)c->f.nsec;
    c->steps = 0;
}

int chain_set(eq_chain_t *c, float g, const biquad_coef_t *sec, int n)
{
    int s, changed;
    if (n > BIQUAD_MAX_SECTIONS) n = BIQUAD_MAX_SECTIONS;
    changed = n != c->tn;
    memset(c->tc, 0, sizeof(c->tc));                 /* identity beyond n */
    for (s = 0; s < n; s++) c->tc[s] = sec[s];
    for (s = c->f.nsec; s < n; s++) {                /* new sections start as the identity, at rest */
        memset(&c->f.c[s], 0, sizeof(c->f.c[s]));
        c->f.z[s][0] = c->f.z[s][1] = 0.0f;
    }
    if (n > c->f.nsec) c->f.nsec = n;
    c->tg = g;
    c->tn = (uint8_t)n;
    c->steps = EQ_GLIDE_FRAMES;
    return changed;
}

static float sstep(float t) { return t * t * (3.0f - 2.0f * t); }

/* One glide step along a smoothstep trajectory (zero slope at both ends: no corner in the coefficient path,
 * which is what a listener hears as a faint click): coefficients += (target - coefficients) * k, k = the share
 * of the remaining path covered by this frame. */
static void glide(eq_chain_t *c)
{
    const float n = (float)EQ_GLIDE_FRAMES, m = n - (float)c->steps;
    const float s0 = sstep(m / n), k = (sstep((m + 1.0f) / n) - s0) / (1.0f - s0);
    float *cur = &c->f.c[0].b1;
    const float *tgt = &c->tc[0].b1;
    int i;
    for (i = 0; i < 4 * c->f.nsec; i++) cur[i] += (tgt[i] - cur[i]) * k;
    if (c->f.g > 0.0f && c->tg > 0.0f)               /* gain in the log domain (see the header) */
        c->f.g *= mx_exp2(k * mx_log2(c->tg / c->f.g));
    else
        c->f.g += (c->tg - c->f.g) * k;
    if (--c->steps == 0) {                           /* exact target, vanished sections dropped */
        memcpy(c->f.c, c->tc, sizeof(c->tc));
        c->f.g = c->tg;
        c->f.nsec = c->tn;
    }
}

/* Ends a glide at once (the outputs are silent: preset, rate or profile switch); rest = 1 also clears the
 * states (profile switch: the new set starts at rest, behind the reopening output ramp). */
void chain_settle(eq_chain_t *c, int rest)
{
    if (c->steps) {
        c->steps = 1;
        glide(c);
    }
    if (rest) memset(c->f.z, 0, sizeof(c->f.z));
}

int eq_hold;                                   /* 1: no glide step (a switch behind the ramp is pending) */

DSP_SCRATCH(idle)
static float idle[4][FRAME_LEN];               /* outputs of the idle lanes */

void eq_chain_run(eq_chain_t *c, float *const x[], int n)
{
    biquad_t *bq[4];
    const float *in[4];
    float *out[4];
    int l;
    for (l = 0; l < 4; l++) {
        const int use = l < n;
        if (use && c[l].steps && !eq_hold) glide(&c[l]);
        bq[l] = use ? &c[l].f : 0;
        in[l] = use ? x[l] : 0;
        out[l] = use ? x[l] : idle[l];
    }
    biquad_run4(bq, in, out, FRAME_LEN);
}
