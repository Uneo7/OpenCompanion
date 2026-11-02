/* Host check of a switch through the whole post-stage timing (0.2.5): engine EQ chain -> audio_out.c (output ramp
 * at the input of the post-stage, look-ahead limiter delay of protect_limit.c, full-scale guard, PCM).
 *   host_switch
 * 50 Hz sine at 0.12 FS (up to ~0.45 FS after the EQ, below the full-scale guard) on the 6 outputs through a
 * bass EQ chain.
 * 1. App-like structural batch (Simple -> Avance / profile written by OPW): 5 writes 40 ms apart, each one a
 *    structural change (engine_profile_swap semantics: ramp closes, waits 50 ms after the last write or OPX,
 *    installs at rest, reopens). Measured: the corner metric of test/eq_glide_check.py (largest second
 *    difference / A w^2) over the whole sequence, at the DAC. Bound: 10.
 * 2. Unmute (ramp reopening on a running signal): same metric.
 * 3. A NaN injected into the post-stage input: the DAC never gets a non-finite or full-scale sample, and the
 *    output is back to the signal level within 0.2 s.
 * The other protection stages and the meters are stubbed (they are per-sample laws checked elsewhere). */
#include <stdio.h>
#include <math.h>
#include "oc.h"
#include "eq.h"
#include "kern.h"
#include "audio_out.h"
#include "protect.h"
#include "inputs.h"
#include "mathx.h"

audio_state_t g_audio;
supply_t g_supply;
float mx_exp2(float x) { return exp2f(x); }
float mx_log2(float x) { return log2f(x); }
void protect_sats(float (*x)[96]) { (void)x; }
void protect_basses(float *a, float *b) { (void)a; (void)b; }
void gpio_in_out_gain(float (*o)[96], int n, int m, float c) { (void)o; (void)n; (void)m; (void)c; }
void supply_detect(float (*o)[96], int n) { (void)o; (void)n; }
void meters_set(int i, float p) { (void)i; (void)p; }
static int recovered;
void engine_recover(void) { recovered++; }
void protect_init(void) { protect_limit_init(); }
float protect_comp_gain(void) { return 1.0f; }

#define PI 3.14159265358979
static void bell(double f0, double db, double q, biquad_coef_t *s, float *g)
{
    const double a = pow(10.0, db / 40.0), w = 2 * PI * f0 / 48000.0, al = sin(w) / (2 * q);
    const double b0 = 1 + al * a, b1 = -2 * cos(w), b2 = 1 - al * a, a0 = 1 + al / a, a1 = -2 * cos(w), a2 = 1 - al / a;
    *g = (float)(b0 / a0); s->b1 = (float)(b1 / b0); s->b2 = (float)(b2 / b0); s->a1 = (float)(-a1 / a0); s->a2 = (float)(-a2 / a0);
}

static float post[OUT_COUNT][FRAME_LEN];
static tx_buf_t tx;

int main(void)
{
    const double w = 2 * PI * 50 / 48000;
    eq_chain_t c;
    biquad_coef_t s[3];
    float g, prev2 = 0, prev1 = 0, corner = 0, amax = 0, worst_nan = 0, after = 0;
    long n = 0;
    int cf = 0, f, i, k, open = 1, pending = 0, quiet = 0, writes = 0;
    protect_limit_init();
    audio_out_init();
    g = 1.0f;
    bell(45, 12, 2, &s[0], &g);
    chain_init(&c, g, s, 1);
    for (f = 0; f < 1400; f++) {
        float *x[1] = { post[0] };
        /* 1. five structural writes 40 ms apart from frame 300; 2. unmute at frame 800 after a mute at 700 */
        if (f >= 300 && f < 400 && (f - 300) % 20 == 0) {
            g = 1.0f;
            bell(45 + 5 * writes, writes & 1 ? -10 : 8, 1.5, &s[0], &g);
            bell(120, 4, 0.7, &s[1], &g);
            chain_set(&c, g, s, 1 + (writes & 1));
            eq_hold = 1; pending = 1; quiet = 0; writes++;
        }
        open = !(f >= 700 && f < 800);
        if (pending) {
            open = 0;
            if (audio_out_silent() && ++quiet >= 25) { chain_settle(&c, 1); eq_hold = 0; pending = 0; }
        }
        for (i = 0; i < FRAME_LEN; i++, n++) post[0][i] = 0.12f * (float)sin(w * n);
        eq_chain_run(&c, x, 1);
        for (k = 1; k < OUT_COUNT; k++) for (i = 0; i < FRAME_LEN; i++) post[k][i] = post[0][i];
        if (f == 1100) post[2][10] = NAN;                  /* 3. a non-finite sample */
        audio_out(post, &tx, open, 1);
        for (i = 0; i < FRAME_LEN; i++) {
            const float y = (float)tx[0][i] / 2147483648.0f, d2 = fabsf(y - 2 * prev1 + prev2);
            if (f >= 250 && f < 1000 && d2 > corner) { corner = d2; cf = f; }
            if (f >= 600 && f < 700 && fabsf(y) > amax) amax = fabsf(y);
            if (f >= 1100 && fabsf((float)tx[2][i] / 2147483648.0f) > worst_nan) worst_nan = fabsf((float)tx[2][i] / 2147483648.0f);
            if (f >= 1200 && fabsf(y) > after) after = fabsf(y);
            prev2 = prev1; prev1 = y;
        }
    }
    printf("structural batch + mute / unmute: corner %.1f at frame %d (bound 10)\n", corner / (amax * w * w), cf);
    printf("NaN injected: recoveries %d, largest DAC sample after it %.3f FS, signal 0.2 s later %.3f FS (steady %.3f)\n",
           recovered, worst_nan, after, amax);
    return 0;
}
