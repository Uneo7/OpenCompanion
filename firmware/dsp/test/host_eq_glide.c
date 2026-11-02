/* Host check of the click-free EQ coefficient updates (src/audio/eq_chain.c).
 *   host_eq_glide OUT_GLIDE OUT_ABRUPT OUT_OLD
 * 50 Hz sine (amplitude 0.25) at 48 kHz through one chain: flat for 100 frames, then a +12 dB bell at 50 Hz
 * (Q 1, RBJ peaking in the Bose convention) at frame 100, moved to 63 Hz at frame 200 (a running section with
 * large states), set with chain_set(); OUT_GLIDE = the firmware behaviour (glide over EQ_GLIDE_FRAMES frames),
 * OUT_ABRUPT = the same changes applied in one step (the coefficients jump, the states run on), OUT_OLD = the
 * former firmware (new filter started from the old filter's states, one-frame linear cross-fade), for comparison.
 * Each file: 400 frames x 96 float samples. Analysed by test/eq_glide_check.py. */
#include <stdio.h>
#include <math.h>
#include "oc.h"
#include "eq.h"
#include "mathx.h"

float mx_exp2(float x) { return exp2f(x); }
float mx_log2(float x) { return log2f(x); }

static void bell(double f0, double db, double q, biquad_coef_t *s, float *g)
{
    const double a = pow(10.0, db / 40.0), w = 2.0 * 3.14159265358979 * f0 / 48000.0, al = sin(w) / (2.0 * q);
    const double b0 = 1.0 + al * a, b1 = -2.0 * cos(w), b2 = 1.0 - al * a;
    const double a0 = 1.0 + al / a, a1 = -2.0 * cos(w), a2 = 1.0 - al / a;
    *g = (float)(b0 / a0);
    s->b1 = (float)(b1 / b0);
    s->b2 = (float)(b2 / b0);
    s->a1 = (float)(-a1 / a0);
    s->a2 = (float)(-a2 / a0);
}

static void run(const char *path, int abrupt)
{
    static float buf[FRAME_LEN];
    float *x[1] = { buf };
    eq_chain_t c;
    biquad_coef_t s;
    float g;
    long n = 0;
    int f, i;
    FILE *fo = fopen(path, "wb");
    chain_init(&c, 1.0f, 0, 0);
    for (f = 0; f < 400; f++) {
        if (f == 100 || f == 200) {
            bell(f == 100 ? 50.0 : 63.0, 12.0, 1.0, &s, &g);
            chain_set(&c, g, &s, 1);
            if (abrupt) c.steps = 1;            /* one-step change: the coefficients jump at this frame */
        }
        for (i = 0; i < FRAME_LEN; i++, n++) buf[i] = 0.25f * (float)sin(2.0 * 3.14159265358979 * 50.0 * n / 48000.0);
        eq_chain_run(&c, x, 1);
        fwrite(buf, 4, FRAME_LEN, fo);
    }
    fclose(fo);
}

/* The former update (eq.c before the glide): biquad_init of the new set, states copied, 1-frame cross-fade. */
static void run_old(const char *path)
{
    static float buf[FRAME_LEN], nb[FRAME_LEN];
    biquad_t f[2], *bq[4] = { 0, 0, 0, 0 };
    const float *in[4] = { buf, 0, 0, 0 };
    float *out[4];
    static float idle[3][FRAME_LEN];
    biquad_coef_t s;
    float g;
    long n = 0;
    int f0, i, act = 0, fading = 0;
    FILE *fo = fopen(path, "wb");
    out[1] = idle[0]; out[2] = idle[1]; out[3] = idle[2];
    biquad_init(&f[0], 1.0f, 0, 0);
    biquad_init(&f[1], 1.0f, 0, 0);
    for (f0 = 0; f0 < 400; f0++) {
        if (f0 == 100 || f0 == 200) {
            bell(f0 == 100 ? 50.0 : 63.0, 12.0, 1.0, &s, &g);
            biquad_init(&f[act ^ 1], g, &s, 1);
            if (f[act].nsec) { f[act ^ 1].z[0][0] = f[act].z[0][0]; f[act ^ 1].z[0][1] = f[act].z[0][1]; }
            fading = 1;
        }
        for (i = 0; i < FRAME_LEN; i++, n++) buf[i] = 0.25f * (float)sin(2.0 * 3.14159265358979 * 50.0 * n / 48000.0);
        if (fading) {
            bq[0] = &f[act ^ 1]; out[0] = nb;
            biquad_run4(bq, in, out, FRAME_LEN);
        }
        bq[0] = &f[act]; out[0] = buf;
        biquad_run4(bq, in, out, FRAME_LEN);
        if (fading) {
            for (i = 0; i < FRAME_LEN; i++) buf[i] += (nb[i] - buf[i]) * ((float)i / FRAME_LEN);
            act ^= 1;
            fading = 0;
        }
        fwrite(buf, 4, FRAME_LEN, fo);
    }
    fclose(fo);
}

int main(int argc, char **argv)
{
    if (argc < 4) return 2;
    run(argv[1], 0);
    run(argv[2], 1);
    run_old(argv[3]);
    return 0;
}
