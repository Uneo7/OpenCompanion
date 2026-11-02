/* Stock band soft clippers and compressors (see include/protect.h). Decoded from re/dsp/out/img.dis; the
 * per-sample laws were checked against a cycle-level emulation of the stock code (residual < 5e-7).
 *
 * Satellite chain (4 identical containers 0x10011450 outer L, 0x100115C4 inner L, 0x10011738 outer R,
 * 0x100118AC inner R; gain 1.0, mixer targets (1, 1)):
 *     y = B(clip(A(x))) + C(x)
 *   A = B: band-pass, g = 1.08058084e-4, zeros (1 + z^-1)^2 (1 - z^-1)^2, poles at ~304 Hz and ~191 Hz
 *          (biquads 0x100114E8 / 0x10011524 ...; sections 3-4 are identity);
 *   C:     complementary band-stop, g = 0.970815122, 4 zeros on the unit circle at 247.4 Hz, same poles
 *          (0x10011560 ...). Small signal: A B + C = 1 (all-pass, checked in bose_profile.md).
 *   clip (class 0x1002D0A8, words 7, 8, 9 = lo, hi, c = 1 / (2 (hi - lo))):
 *     a = min(|x|, hi);  y = sign(x) (a - c max(a - lo, 0)^2)
 *     satellites 0.18 / 0.22 / 12.5 (output <= 0.20); bass 0x10011C24: 0.398107 / 1.19055 / 0.630961.
 *   The knee is at 0.18 of the band signal before the final gain (0.89145): ~-16 dBFS band peak at the DAC.
 * Compressor (class 0x1002CB4C, init gain 1; detector = the channel itself), per sample:
 *     u = |P2 x|;  gu = g u
 *     gu < P0          -> g = P1 g                         (release, P1 > 1)
 *     gu < 1 + P0 - P4 -> g = P1 g (1 + P0 - gu)           (reduction proportional to the excess)
 *     else             -> g = P4 g                         (at most -10 dB per sample)
 *     g = min(g, P3);  y = g x                             (the gain of the current sample is used)
 *   satellites 0x10011A20: P0 = P2 = 0.0236999001, P1 = 1.00023985 (100 dB/s release), P3 = 1, P4 = 0.316227764
 *   bass 0x10011A54:       P0 = 0.00716975238, P1 = 1.00009596 (40 dB/s), P2 = 0.00804459397, P3 = 1, P4 = same
 *   Threshold P0 / P2 = 1.0 (satellites), 0.89125 (bass). */
#include "oc.h"
#include "protect.h"
#include "biquad.h"
#include "rate.h"
#include "kern.h"
#include "oc.h"

typedef struct { float p0, p1, p2, p4; } comp_par_t;     /* k_comp4 order */

static const biquad_coef_t SAT_AB[2] = {
    { 2.0f, 1.0f, 1.98031998f, -0.981974006f },
    { -2.0f, 1.0f, 1.98798132f, -0.988636255f },
};
#define SAT_AB_G 0.000108058084f
static const biquad_coef_t SAT_C[4] = {
    { -1.99895144f, 1.0f, 1.98031998f, -0.981974006f },
    { -1.99895144f, 1.0f, 1.98031998f, -0.981974006f },
    { -1.99895144f, 1.0f, 1.98798132f, -0.988636255f },
    { -1.99895144f, 1.0f, 1.98798132f, -0.988636255f },
};
#define SAT_C_G 0.970815122f

static const comp_par_t COMP_SAT48 = { 0.0236999001f, 1.00023985f, 0.0236999001f, 0.316227764f };
static const comp_par_t COMP_BASS48 = { 0.00716975238f, 1.00009596f, 0.00804459397f, 0.316227764f };

static biquad_t fa[4], fb[4], fc[4];
static comp_par_t comp_sat, comp_bass;
static float g_sat[4], g_bass[4];        /* g_bass[2..3]: idle lanes of k_comp4 */


static float band[4][FRAME_LEN], rest[4][FRAME_LEN];

void protect_rate(void)
{
    int k;
    for (k = 0; k < 4; k++) {
        biquad_init(&fa[k], SAT_AB_G, SAT_AB, 2);
        biquad_init(&fb[k], SAT_AB_G, SAT_AB, 2);
        biquad_init(&fc[k], SAT_C_G, SAT_C, 4);
    }
    comp_sat = COMP_SAT48;
    comp_bass = COMP_BASS48;
}

void protect_init(void)
{
    int k;
    for (k = 0; k < 4; k++) g_sat[k] = 1.0f;
    for (k = 0; k < 4; k++) g_bass[k] = 1.0f;
    protect_rate();
    protect_limit_init();
}

/* The 4 satellites at once (4-lane biquads, 4-lane compressor): y = B(clip(A(x))) + C(x), then compressed. */
void protect_sats(float (*x)[FRAME_LEN])
{
    biquad_t *pa[4], *pb[4], *pc[4];
    const float *in[4];
    float *pband[4], *prest[4], *px[4];
    int k;
    for (k = 0; k < 4; k++) {
        pa[k] = &fa[k]; pb[k] = &fb[k]; pc[k] = &fc[k];
        in[k] = x[k]; pband[k] = band[k]; prest[k] = rest[k]; px[k] = x[k];
    }
    biquad_run4(pa, in, pband, FRAME_LEN);
    for (k = 0; k < 4; k++) k_softclip(band[k], 0.18f, 0.22f, 12.5f, FRAME_LEN);
    biquad_run4(pb, (const float *const *)pband, pband, FRAME_LEN);
    biquad_run4(pc, in, prest, FRAME_LEN);
    for (k = 0; k < 4; k++) {
        k_scale(x[k], band[k], 1.0f, FRAME_LEN);
        k_mac(x[k], rest[k], 1.0f, FRAME_LEN);
    }
    k_comp4(px, g_sat, &comp_sat.p0, FRAME_LEN);
}

/* Both bass outputs (lanes 2, 3 of the compressor idle on a scratch buffer). */
void protect_basses(float *b0, float *b1)
{
    float *px[4];
    px[0] = b0; px[1] = b1; px[2] = band[0]; px[3] = band[1];
    k_comp4(px, g_bass, &comp_bass.p0, FRAME_LEN);
    k_softclip(b0, 0.398107171f, 1.19054925f, 0.630960882f, FRAME_LEN);
    k_softclip(b1, 0.398107171f, 1.19054925f, 0.630960882f, FRAME_LEN);
}

/* Compressor gains back to the stock initial value (switch behind the output ramp: the new set starts at
 * rest, the protection too). */
void protect_comp_reset(void)
{
    int k;
    for (k = 0; k < 4; k++) g_sat[k] = g_bass[k] = 1.0f;
}

float protect_comp_gain(void)
{
    float g = g_bass[0] < g_bass[1] ? g_bass[0] : g_bass[1];
    int k;
    for (k = 0; k < 4; k++) if (g_sat[k] < g) g = g_sat[k];
    return g;
}
