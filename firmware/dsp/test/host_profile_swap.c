/* Host check of a profile switch (engine.c engine_profile_swap + audio_out.c output ramp + eq_chain.c).
 *   host_profile_swap OUT_SWAP OUT_GLIDE FREQ_HZ
 * A sine at FREQ_HZ (amplitude 0.25), 48 kHz, through one bass EQ chain. Profile A: +12 dB bell at 45 Hz (Q 2)
 * and +6 dB low shelf at 80 Hz; profile B: -10 dB bell at 60 Hz (Q 1.5), high-pass at 30 Hz, -4 dB high shelf
 * at 120 Hz. At frame 100 the profile changes.
 *   OUT_SWAP:  the firmware path: output ramp closes (15 ms smoothstep, k_fade), every glide is settled and the
 *              new set starts at rest (chain_settle(c, 1)), the ramp opens again.
 *   OUT_GLIDE: the bld_e path for comparison: the new set glides from the old one, no ramp.
 * Each file: 400 frames x 96 floats. Checked by test/profile_swap_check.py. */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include "oc.h"
#include "eq.h"
#include "kern.h"
#include "mathx.h"

float mx_exp2(float x) { return exp2f(x); }
float mx_log2(float x) { return log2f(x); }

#define PI 3.14159265358979

/* RBJ cookbook in the Bose convention: section (b1, b2, a1, a2) normalised by b0, gain b0 / a0 into *g. */
static void rbj(int type, double f0, double db, double q, biquad_coef_t *s, float *g)
{
    const double A = pow(10.0, db / 40.0), w = 2.0 * PI * f0 / 48000.0, cw = cos(w), al = sin(w) / (2.0 * q);
    double b0, b1, b2, a0, a1, a2;
    if (type == 0) {                                         /* bell */
        b0 = 1 + al * A; b1 = -2 * cw; b2 = 1 - al * A; a0 = 1 + al / A; a1 = -2 * cw; a2 = 1 - al / A;
    } else if (type == 1) {                                  /* high-pass */
        b0 = (1 + cw) / 2; b1 = -(1 + cw); b2 = (1 + cw) / 2; a0 = 1 + al; a1 = -2 * cw; a2 = 1 - al;
    } else {                                                 /* shelf: 2 low, 3 high */
        const double sA = 2 * sqrt(A) * al, sg = type == 2 ? -1.0 : 1.0;
        b0 = A * ((A + 1) + sg * (A - 1) * cw + sA);
        b1 = -2 * sg * A * ((A - 1) + sg * (A + 1) * cw);
        b2 = A * ((A + 1) + sg * (A - 1) * cw - sA);
        a0 = (A + 1) - sg * (A - 1) * cw + sA;
        a1 = 2 * sg * ((A - 1) - sg * (A + 1) * cw);
        a2 = (A + 1) - sg * (A - 1) * cw - sA;
    }
    *g *= (float)(b0 / a0);
    s->b1 = (float)(b1 / b0);
    s->b2 = (float)(b2 / b0);
    s->a1 = (float)(-a1 / a0);
    s->a2 = (float)(-a2 / a0);
}

static void run(const char *path, int swap, double hz)
{
    static float buf[FRAME_LEN], h[FRAME_LEN];
    float *x[1] = { buf };
    biquad_coef_t a[2], b[3];
    float ga = 1.0f, gb = 1.0f, ph = 1.0f, step = 1000.0f / (15.0f * 48000.0f) * FRAME_LEN;
    eq_chain_t c;
    long n = 0;
    int f, i, pending = 0;
    FILE *fo = fopen(path, "wb");
    rbj(0, 45, 12, 2, &a[0], &ga);
    rbj(2, 80, 6, 0.707, &a[1], &ga);
    rbj(0, 60, -10, 1.5, &b[0], &gb);
    rbj(1, 30, 0, 0.707, &b[1], &gb);
    rbj(3, 120, -4, 0.707, &b[2], &gb);
    chain_init(&c, ga, a, 2);
    for (f = 0; f < 400; f++) {
        float p0 = ph;
        if (f == 100) {
            if (swap) pending = 1;
            else chain_set(&c, gb, b, 3);
        }
        if (pending && ph <= 0.0f) {                         /* silent: install B at rest */
            chain_set(&c, gb, b, 3);
            chain_settle(&c, 1);
            pending = 0;
        }
        ph += pending ? -step : step;
        ph = ph < 0.0f ? 0.0f : (ph > 1.0f ? 1.0f : ph);
        k_fade(h, 1.0f, 1.0f, p0, ph, FRAME_LEN);
        for (i = 0; i < FRAME_LEN; i++, n++) buf[i] = 0.25f * (float)sin(2.0 * PI * hz * n / 48000.0);
        eq_chain_run(&c, x, 1);
        for (i = 0; i < FRAME_LEN; i++) buf[i] *= h[i];
        fwrite(buf, 4, FRAME_LEN, fo);
    }
    fclose(fo);
}

int main(int argc, char **argv)
{
    if (argc < 4) return 2;
    run(argv[1], 1, atof(argv[3]));
    run(argv[2], 0, atof(argv[3]));
    return 0;
}
