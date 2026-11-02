/* Stock satellite look-ahead peak limiter (class 0x1002B020, instance 0x10011AC8, container 0x10011A88),
 * decoded from re/dsp/out/img.dis and checked against a cycle-level emulation (residual < 5e-8).
 * Parameters: P0 = 32 (sub-block), P1 = 19 (window, sub-blocks), P3 = raised-cosine window below,
 * P4 = 0.95 (ceiling), P5 = 0 (Fritsch-Butland slopes). Channels are independent (no stereo link).
 * Per channel, for each 32-sample sub-block j:
 *   pk[j] = max |x| over the sub-block                                   (0x1002DB40)
 *   env   = max_k pk[j - k] w[k], k = 0..18                               (0x1001CA40)
 *   gt    = min(1, 0.95 / env)
 *   m1    = s1 strictly between s0 and gt ? 2 (s1 - s0)(gt - s1) / (gt - s0) : 0  (|gt - s0| >= 1e-4)
 *   gain over the sub-block: cubic Hermite from s0 to s1 with slopes m0, m1, t = (i + 1) / 32
 *   s0 = s1, s1 = gt, m0 = m1
 * Output = x delayed by 320 samples (10 sub-blocks) x gain: the delayed peak of sub-block p falls where
 * both Hermite end points weight pk[p] by w = 1, so it is held at 0.95.
 * Start-up: s0 = s1 = 0, so the gain is 0 for one sub-block, then ramps up (stock soft start).
 * OpenCompanion: the bass gets the same 320-sample delay (stock: no delay on the bass, 6.7 ms ahead of the
 * satellites), so the crossover alignment of the profiles is unchanged. The sub-block length stays 32
 * samples (48 kHz only). */
#include <string.h>
#include "oc.h"
#include "protect.h"
#include "kern.h"

#define SUB     32
#define WIN     19
#define NSAT    4
#define CEIL    0.95f

static const float WINDOW[WIN] = {
    0.119930089f, 0.360098541f, 0.57808423f, 0.760458767f, 0.895987511f, 0.976321638f,
    0.999636352f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.999636352f, 0.976321638f, 0.895987511f, 0.760458767f,
    0.57808423f, 0.360098541f, 0.119930089f };

typedef struct {
    float pk[WIN];
    int   idx;
    float s0, s1, m0;
    float line[LIM_DELAY];
    int   pos;                /* k_delay position */
} lim_ch_t;

/* .dsp_ram (not zeroed at start): every state is cleared by protect_limit_init() */
DSP_SCRATCH(ch)
DSP_SCRATCH(bass_line)
DSP_SCRATCH(gain)
static lim_ch_t ch[NSAT];
static float bass_line[2][LIM_DELAY];
static int bass_pos[2];
DSP_SCRATCH(tmp)
static float tmp[FRAME_LEN];
static float gain[FRAME_LEN];
static float lim_min;

void protect_limit_init(void)
{
    memset(ch, 0, sizeof(ch));
    memset(bass_line, 0, sizeof(bass_line));
    bass_pos[0] = bass_pos[1] = 0;
    lim_min = 1.0f;
}

static float gain_curve(lim_ch_t *c, const float *x, int n)
{
    float mn = 1.0f;
    int j, i, k;
    for (j = 0; j + SUB <= n; j += SUB) {
        float env = 0.0f, gt, m1 = 0.0f, d, c2, c3, g;
        c->pk[c->idx] = k_peaksq(x + j, SUB, &d);
        for (k = 0, i = c->idx; k < WIN; k++) {      /* newest first: pk[j - k] * w[k] */
            float v = c->pk[i] * WINDOW[k];
            if (v > env) env = v;
            if (--i < 0) i = WIN - 1;
        }
        if (++c->idx == WIN) c->idx = 0;
        gt = CEIL / (env + 1e-30f);
        if (gt > 1.0f) gt = 1.0f;
        if ((c->s1 < gt || c->s1 < c->s0) && (c->s1 > gt || c->s1 > c->s0)) {
            float den = gt - c->s0;
            if (den < 1e-4f && den > -1e-4f) den = gt < c->s0 ? -1e-4f : 1e-4f;
            m1 = 2.0f * (c->s1 - c->s0) * (gt - c->s1) / den;
        }
        d = c->s1 - c->s0;
        c3 = c->m0 + m1 - 2.0f * d;
        c2 = d - c->m0 - c3;
        g = k_hermite(gain + j, c->s0, c->m0, c2, c3, 1.0f / SUB, SUB);
        if (g < mn) mn = g;
        c->s0 = c->s1;
        c->s1 = gt;
        c->m0 = m1;
    }
    return mn;
}

void protect_limit(float (*sat)[96], int n)
{
    int k;
    float mn = 1.0f, g;
    for (k = 0; k < NSAT; k++) {
        lim_ch_t *c = &ch[k];
        g = gain_curve(c, sat[k], n);
        if (g < mn) mn = g;
        k_copy(tmp, sat[k], n);
        k_delay(c->line, LIM_DELAY, &c->pos, tmp, sat[k], n);
        k_scalev(sat[k], sat[k], gain, n);
    }
    lim_min = mn;
}

void protect_delay_bass(int b, float *x, int n)
{
    k_copy(tmp, x, n);
    k_delay(bass_line[b], LIM_DELAY, &bass_pos[b], tmp, x, n);
}

float protect_lim_gain(void) { return lim_min; }
