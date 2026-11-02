/* Host check of Avance (per-output EQ) edits through the real output path (0.2.6): eq.c / eq_design.c /
 * eq_chain.c output banks -> audio_out.c (output ramp, protect_band.c clippers + compressors, protect_limit.c
 * look-ahead limiter, full-scale guard, PCM).
 *   host_avance
 * Signal: 50 + 300 + 1500 + 6000 Hz, 0.15 FS each, on the 4 satellites and the bass.
 * Edits, as the app writes them (structural ones behind the output ramp, engine.c semantics: closes, waits for
 * the end of the batch, installs at rest, protect_comp_reset(), reopens):
 *   1. Simple -> Avance: 4 bands at +12 dB on every satellite output, one write per output, 40 ms apart.
 *   2. Drags for 6 s, one write every 8 frames (60 Hz): gain -15..+15 dB, frequency 20 Hz..20 kHz, Q 0.1..20,
 *      over every band of every satellite output; shelves and a high-Q low bell at +15 dB in the mix.
 *   3. Back to the step-1 set; the level must be the level of a fresh run of that set.
 *   4. One frame of absurd finite samples (1e30) at the post-stage input (an upstream filter blowing up): the
 *      compressors must not latch (0.2.5: gain driven to 0, satellites silent for good, nothing at the output
 *      to see); 0.2.6 resets the path (audio_out.c, compressor gain check) and the level comes back.
 * Checks: no NaN / Inf at the DAC; compressor gain (PRT CG) never below -60 dB; satellite level after 3. and
 * after 4. within 1 dB of the reference. */
#include <stdio.h>
#include <string.h>
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
profile_t g_profile;
float mx_exp2(float x) { return exp2f(x); }
float mx_log2(float x) { return x > 0.0f ? log2f(x) : -126.0f; }
float mx_sqrt(float x) { return sqrtf(x); }
void mx_sincos(float w, float *s, float *c) { *s = sinf(w); *c = cosf(w); }
void gpio_in_out_gain(float (*o)[96], int n, int m, float c) { (void)o; (void)n; (void)m; (void)c; }
void supply_detect(float (*o)[96], int n) { (void)o; (void)n; }
void meters_set(int i, float p) { (void)i; (void)p; }
int bose_tables_ok(void) { return 1; }
int audio_graph_active(void) { return 0; }

static int pending, quiet, recovered;
void engine_profile_swap(void) { pending = 1; quiet = 0; eq_hold = 1; }
void engine_recover(void) { recovered++; eq_settle(1); }

static float post[OUT_COUNT][FRAME_LEN];
static tx_buf_t tx;
static long n;
static float worst_cg = 1.0f;
static int bad_dac;

static void set_band(int t, int b, int type, int hz, int cdb, int qm)
{
    eq_band_t *d = &g_profile.band[t][b];
    d->type = (uint8_t)type; d->freq_hz = (uint16_t)hz; d->gain_cdb = (int16_t)cdb; d->q_milli = (uint16_t)qm;
}

/* params_write: structural (a type changed) -> behind the ramp; then eq_rebuild (a section count change also
 * asks for the swap, eq.c). */
static void write_target(int t, int structural)
{
    if (structural || pending) engine_profile_swap();
    eq_rebuild(&g_profile, DIRTY_TARGET(t));
}

static void step1(void)
{
    static const int hz[4] = { 60, 150, 400, 2500 };
    int t, b;
    for (t = EQ_INNER_L; t <= EQ_OUTER_R; t++)
        for (b = 0; b < 4; b++) set_band(t, b, BAND_PEAK, hz[b], 1200, 1000);
}

/* One frame; returns the largest satellite sample at the DAC. */
static float frame(int open)
{
    static const double f[4] = { 50, 300, 1500, 6000 };
    float *x[EQ_BANK_LANES];
    float pk = 0.0f;
    int i, k;
    if (pending) {
        open = 0;
        if (audio_out_silent() && ++quiet >= 25) { eq_settle(1); protect_comp_reset(); eq_hold = 0; pending = 0; }
    }
    for (i = 0; i < FRAME_LEN; i++, n++) {
        double v = 0;
        for (k = 0; k < 4; k++) v += 0.15 * sin(2 * 3.14159265358979 * f[k] * n / 48000.0);
        for (k = 0; k < OUT_COUNT; k++) post[k][i] = (float)v;
    }
    for (k = 0; k < 5; k++) x[k] = post[k];
    eq_bank_run(EQ_BANK_OUT, x);
    memcpy(post[OUT_BASS_B], post[OUT_BASS_A], sizeof(post[0]));
    audio_out(post, &tx, open, 1);
    if (protect_comp_gain() < worst_cg) worst_cg = protect_comp_gain();
    for (k = 0; k < 4; k++)
        for (i = 0; i < FRAME_LEN; i++) {
            const float y = (float)tx[k][i] / 2147483648.0f;
            if (!isfinite(y)) bad_dac++;
            if (fabsf(y) > pk) pk = fabsf(y);
        }
    return pk;
}

static float level(int frames)
{
    float pk = 0.0f, p;
    while (frames--) if ((p = frame(1)) > pk) pk = p;
    return pk;
}

static void reset_all(void)
{
    memset(&g_profile, 0, sizeof(g_profile));
    g_profile.flags = PF_ADVANCED | PF_STOCK_GRAPH | PF_LOUDNESS;
    eq_init();
    protect_init();
    audio_out_init();
    pending = quiet = 0;
    eq_hold = 0;
}

int main(void)
{
    float ref, after_drag, after_burst, wcg_drag;
    int f, t, rec_drag;
    unsigned r = 12345u;

    /* reference: the step-1 set from scratch */
    reset_all();
    step1();
    eq_rebuild(&g_profile, DIRTY_ALL);
    eq_settle(1);
    level(500);
    ref = level(100);

    reset_all();
    level(200);
    step1();                                            /* 1. Simple -> Avance, one write per output */
    for (t = EQ_INNER_L; t <= EQ_OUTER_R; t++) { write_target(t, 1); level(20); }
    level(200);
    for (f = 0; f < 6 * 500 / 8; f++) {                 /* 2. drags */
        int b, k;
        r = r * 1103515245u + 12345u;
        t = EQ_INNER_L + (int)(r >> 16) % 4;
        b = (int)(r >> 20) % 4;
        k = (int)(r >> 24) % 3;
        {
            eq_band_t *d = &g_profile.band[t][b];
            const float u = (float)((r >> 8) & 0xFFFF) / 65535.0f;
            if (k == 0) d->gain_cdb = (int16_t)(-1500 + 3000 * u);
            else if (k == 1) d->freq_hz = (uint16_t)(20.0f * powf(1000.0f, u));
            else d->q_milli = (uint16_t)(100.0f * powf(200.0f, u));
        }
        if (f % 97 == 0) set_band(t, 4, BAND_LOW_SHELF, 20, 1500, 20000), write_target(t, 1);
        else if (f % 89 == 0) set_band(t, 5, BAND_HIGH_SHELF, 16000, 1500, 20000), write_target(t, 1);
        else if (f % 83 == 0) set_band(t, 6, BAND_PEAK, 20, 1500, 20000), write_target(t, 1);
        else write_target(t, 0);
        level(8);
    }
    wcg_drag = worst_cg;
    rec_drag = recovered;
    memset(g_profile.band, 0, sizeof(g_profile.band));  /* 3. back to the step-1 set */
    step1();
    for (t = EQ_INNER_L; t <= EQ_OUTER_R; t++) write_target(t, 1);
    level(500);
    after_drag = level(100);

    level(10);                                          /* 4. one absurd frame at the post-stage input */
    {
        static const double fq[4] = { 50, 300, 1500, 6000 };
        float *x[EQ_BANK_LANES];
        int i, k;
        for (i = 0; i < FRAME_LEN; i++, n++) {
            double v = 0;
            for (k = 0; k < 4; k++) v += 0.15 * sin(2 * 3.14159265358979 * fq[k] * n / 48000.0);
            for (k = 0; k < OUT_COUNT; k++) post[k][i] = (float)v;
        }
        for (k = 0; k < 5; k++) x[k] = post[k];
        eq_bank_run(EQ_BANK_OUT, x);
        for (k = 0; k < 4; k++) for (i = 0; i < FRAME_LEN; i++) post[k][i] *= 1e30f;
        audio_out(post, &tx, 1, 1);
    }
    level(500);
    after_burst = level(100);

    printf("drags: compressor gain min %.1f dB (floor -60), recoveries %d, non-finite DAC samples %d\n",
           20 * log10f(wcg_drag), rec_drag, bad_dac);
    printf("satellite peak: reference %.3f FS, after the drags %.3f FS, after an absurd frame %.3f FS "
           "(recoveries %d, compressor %.1f dB)\n", ref, after_drag, after_burst, recovered,
           20 * log10f(protect_comp_gain()));
    return !(bad_dac == 0 && wcg_drag >= 0.000999f && fabsf(20 * log10f(after_drag / ref)) < 1.0f &&
             fabsf(20 * log10f(after_burst / ref)) < 1.0f);
}
