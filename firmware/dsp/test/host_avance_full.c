/* Host replay of Avance edits through the whole firmware signal path (0.2.7): engine.c (gate, batch / OPX
 * commit, inputs, stock Bose graph with the loudness hooks, global and per-output EQ banks, trims) ->
 * loudness.c + level_detect.c -> audio_out.c (ramp, protect_band.c, protect_limit.c, guards, PCM).
 * DSP math as on the target: src/util/mathx.c (with a host c6x.h: bit casts, 8-bit RSQRSP estimate), and the
 * x86 FPU in flush-to-zero / denormals-are-zero mode like the C67x.
 *   host_avance_full [seed [trace.f32 [watch_frame [drag_what]]]]
 * Profile writes are the app's (oc-device write_profile): OPW ranges of codec.rs write_ranges (header bytes,
 * then band pairs, at most 55 bytes), OPX when flags or a band type changed. One OPW per frame.
 * Sequence (Bose base, loudness on, OS volume -22 dB, music-like stereo at about -10 dBFS):
 *   1. Simple: 3 global bands. 2. Simple -> Avance (app to_advanced: global bands in front of each output list).
 *   3. 3 minutes of app editing: bands added / removed / type changed on every output (+ OPX), drags of
 *      gain (-15..+15 dB), frequency (20 Hz..20 kHz) and Q (0.1..20) at 60 writes / s, bursts of +12 dB.
 * Checks, every frame: the post-stage input (engine output) finite and below 1e6 (+120 dBFS; the most extreme
 * legal EQ, every band +15 dB with high Q, reaches ~+72 dBFS); the compressor gain (PRT CG) above -100 dB (the
 * reset threshold of audio_out.c); no non-finite DAC sample. The first offending frame is reported with the
 * edit that preceded it. Optional: a per-frame peak trace, a frame to log around, a high-Q drag test. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <xmmintrin.h>
#include "oc.h"
#include "eq.h"
#include "kern.h"
#include "audio_out.h"
#include "protect.h"
#include "inputs.h"
#include "mathx.h"
#include "engine.h"
#include "bose_graph.h"
#include "rate.h"

/* --- stubs: hardware, volume, storage --- */
gpio_in_t g_gpio;
int g_dev_opts;
supply_t g_supply;
uint8_t g_layout = LAYOUT_51;
int bose_tables_ok(void) { return 1; }
void gpio_in_out_gain(float (*o)[96], int n, int m, float c) { (void)o; (void)n; (void)m; (void)c; }
void supply_detect(float (*o)[96], int n) { (void)o; (void)n; }
void meters_set(int i, float p) { (void)i; (void)p; }
void sub_level_apply(float *x, int n) { (void)x; (void)n; }
audio_state_t g_audio;
void audio_update_volume(void) { }

static rx_buf_t rx;
static tx_buf_t tx;
static long n;
static unsigned rng = 1u;
static unsigned rnd(void) { rng = rng * 1103515245u + 12345u; return rng >> 8; }
static float urand(void) { return (float)(rnd() & 0xFFFF) / 65535.0f; }

static char last_edit[160];
static long frame_no, bad_frame = -1;
static float worst_cg = 1.0f, worst_in = 0.0f;
static int bad_dac;
static FILE *trace;                                 /* optional: per-frame peak of the post-stage input */
static long watch = -1;                             /* optional: log the frames around this one */

static void music(void)
{
    static const double f[9] = { 41, 87, 163, 331, 677, 1301, 2711, 5303, 10601 };
    int i, k;
    for (i = 0; i < FRAME_LEN; i++, n++) {
        double l = 0, r = 0;
        for (k = 0; k < 9; k++) {
            l += 0.035 * sin(2 * 3.14159265358979 * f[k] * n / 48000.0 + k);
            r += 0.035 * sin(2 * 3.14159265358979 * f[k] * 1.01 * n / 48000.0 + 2 * k);
        }
        l += 0.02 * (urand() - 0.5);
        r += 0.02 * (urand() - 0.5);
        for (k = 0; k < TDM_SLOTS; k++) rx[0][k][i] = rx[1][k][i] = 0;
        rx[0][0][i] = (int32_t)(l * 2147483647.0);
        rx[0][1][i] = (int32_t)(r * 2147483647.0);
    }
}

static void frame(void)
{
    float *x[BG_IN_COUNT], pow6[LD_CHANNELS], usb[FRAME_LEN], s, pf, pk = 0.0f;
    static float post[OUT_COUNT][FRAME_LEN];
    int i, k;
    pf = engine_frame_gate();
    music();
    s = engine_fill(&rx, x);
    level_detect(x, s, 1.0f, pow6);
    loudness_frame(-22.0f, 0, 1.0f, (g_profile.flags & PF_LOUDNESS) != 0, pow6);
    loudness_gains(FRAME_LEN);
    for (i = 0; i < FRAME_LEN; i++) usb[i] = 1.0f;
    engine_run(usb, post);
    for (k = 0; k < OUT_COUNT; k++)
        for (i = 0; i < FRAME_LEN; i++) {
            const float a = fabsf(post[k][i]);
            if (!(a < 1e30f)) pk = INFINITY; else if (a > pk) pk = a;
        }
    if (pk > worst_in) worst_in = pk;
    if (trace) fwrite(&pk, 4, 1, trace);
    if (watch >= 0 && frame_no > watch - 60 && frame_no < watch + 20)
        printf("  %ld peak %g [%.3g %.3g %.3g %.3g %.3g] | %s\n", frame_no, pk, k_peaksq(post[0], FRAME_LEN, &s),
               k_peaksq(post[1], FRAME_LEN, &s), k_peaksq(post[2], FRAME_LEN, &s), k_peaksq(post[3], FRAME_LEN, &s),
               k_peaksq(post[4], FRAME_LEN, &s), last_edit);
    audio_out(post, &tx, pf >= 1.0f, engine_bass_delay());
    if (protect_comp_gain() < worst_cg) worst_cg = protect_comp_gain();
    for (k = 0; k < OUT_COUNT; k++)
        for (i = 0; i < FRAME_LEN; i++) if (tx[k][i] == (int32_t)0x80000000) bad_dac++;
    if (bad_frame < 0 && (!(pk < 1.0e6f) || !(protect_comp_gain() > 1.0e-5f))) {
        bad_frame = frame_no;
        printf("frame %ld: post-stage input peak %g, compressor %.1f dB, after: %s\n", frame_no, pk,
               20 * log10f(protect_comp_gain() + 1e-30f), last_edit);
    }
    frame_no++;
}

/* --- the app's profile writes --- */
static int structural(const profile_t *a, const profile_t *b)
{
    int t, k;
    if (a->flags != b->flags) return 1;
    for (t = 0; t < EQ_TARGETS; t++)
        for (k = 0; k < EQ_MAX_BANDS; k++) if (a->band[t][k].type != b->band[t][k].type) return 1;
    return 0;
}

static void opw(uint32_t off, const uint8_t *b, uint32_t len) { params_write(off, b + off, len); frame(); }

static void write_profile(const profile_t *to)
{
    enum { MAX = 55, PAIR = 16 };
    uint8_t a[PROFILE_SIZE], b[PROFILE_SIZE];
    profile_t from = g_profile;
    int lo = -1, hi = -1, i, p, rs = -1, re = 0;
    memcpy(a, &from, PROFILE_SIZE);
    memcpy(b, to, PROFILE_SIZE);
    for (i = 0; i < PROFILE_BANDS_OFF; i++) if (a[i] != b[i]) { if (lo < 0) lo = i; hi = i; }
    for (i = lo; lo >= 0 && i <= hi; i += MAX) opw((uint32_t)i, b, (uint32_t)((i + MAX > hi + 1 ? hi + 1 : i + MAX) - i));
    for (p = PROFILE_BANDS_OFF; p < PROFILE_SIZE; p += PAIR) {
        const int d = memcmp(a + p, b + p, PAIR) != 0;
        if (d && rs >= 0 && (re - rs) / PAIR < MAX / PAIR) re = p + PAIR;
        else if (d && rs >= 0) { opw((uint32_t)rs, b, (uint32_t)(re - rs)); rs = p; re = p + PAIR; }
        else if (d) { rs = p; re = p + PAIR; }
        else if (rs >= 0) { opw((uint32_t)rs, b, (uint32_t)(re - rs)); rs = -1; }
    }
    if (rs >= 0) opw((uint32_t)rs, b, (uint32_t)(re - rs));
    if (structural(&from, to)) engine_commit();
}

static void set_band(profile_t *p, int t, int k, int type, float hz, float db, float q)
{
    eq_band_t *d = &p->band[t][k];
    d->type = (uint8_t)type;
    d->freq_hz = (uint16_t)(hz + 0.5f);
    d->gain_cdb = (int16_t)lrintf(db * 100.0f);
    d->q_milli = (uint16_t)lrintf(q * 1000.0f);
}

static int count(const profile_t *p, int t)
{
    int k = 0;
    while (k < EQ_MAX_BANDS && p->band[t][k].type != BAND_OFF) k++;
    return k;
}

static void run(int frames) { while (frames--) frame(); }

int main(int argc, char **argv)
{
    profile_t p;
    int t, k, step;
    _mm_setcsr(_mm_getcsr() | 0x8040);                 /* FTZ + DAZ, as the C67x */
    if (argc > 1) rng = (unsigned)atoi(argv[1]);
    if (argc > 2) trace = fopen(argv[2], "wb");
    if (argc > 3) watch = atol(argv[3]);
    eq_init();
    loudness_init();
    protect_init();
    audio_out_init();
    params_defaults(&p);
    p.flags = PF_STOCK_GRAPH | PF_LOUDNESS;
    params_set_active(&p);
    engine_init();
    run(500);

    p = g_profile;                                     /* 1. Simple, 3 global bands */
    set_band(&p, EQ_GLOBAL, 0, BAND_PEAK, 100, 3, 1);
    set_band(&p, EQ_GLOBAL, 1, BAND_LOW_SHELF, 60, 4, 0.7f);
    set_band(&p, EQ_GLOBAL, 2, BAND_HIGH_SHELF, 8000, 2, 0.7f);
    strcpy(last_edit, "Simple bands");
    write_profile(&p);
    run(250);

    p = g_profile;                                     /* 2. Simple -> Avance */
    for (t = EQ_INNER_L; t <= EQ_SUB; t++) {
        memmove(&p.band[t][3], &p.band[t][0], sizeof(eq_band_t) * (EQ_MAX_BANDS - 3));
        memcpy(&p.band[t][0], &p.band[EQ_GLOBAL][0], sizeof(eq_band_t) * 3);
    }
    memset(p.band[EQ_GLOBAL], 0, sizeof(p.band[EQ_GLOBAL]));
    p.flags |= PF_ADVANCED;
    strcpy(last_edit, "Simple -> Avance");
    write_profile(&p);
    run(250);

    if (argc > 4) {                                    /* drag test: argv[4] = what (0 freq, 1 gain, 2 Q) */
        const int what = atoi(argv[4]);
        p = g_profile;
        for (t = EQ_INNER_L; t <= EQ_SUB; t++) set_band(&p, t, 3, BAND_PEAK, 30, 15, 20);
        strcpy(last_edit, "add Q 20 bells");
        write_profile(&p);
        run(250);
        for (step = 0; step < 20 * 60; step++) {
            const float ph = (float)(step % 120) / 60.0f, tri = ph < 1.0f ? ph : 2.0f - ph;   /* 2 s sweep */
            float pk = 0.0f;
            p = g_profile;
            for (t = EQ_INNER_L; t <= EQ_SUB; t++) {
                eq_band_t *d = &p.band[t][3];
                if (what == 0) d->freq_hz = (uint16_t)(20.0f * powf(4.0f, tri));
                else if (what == 1) d->gain_cdb = (int16_t)(-1500 + 3000 * tri);
                else d->q_milli = (uint16_t)(1000.0f * powf(20.0f, tri));
            }
            sprintf(last_edit, "drag %d step %d", what, step);
            write_profile(&p);
            run(7);
            if (worst_in > pk) pk = worst_in;
            if (step % 60 == 59) { printf("  t %2d s: worst post-stage input so far %g\n", step / 60 + 1, worst_in); }
        }
        return 0;
    }

    for (step = 0; step < 180 * 60; step++) {          /* 3. editing, 60 writes / s */
        const unsigned r = rnd();
        p = g_profile;
        t = EQ_INNER_L + (int)(r % 5);
        k = count(&p, t);
        if (r % 211 == 0 && k < EQ_MAX_BANDS) {           /* add a band */
            static const int types[6] = { BAND_PEAK, BAND_LOW_SHELF, BAND_HIGH_SHELF, BAND_HPF, BAND_LPF, BAND_NOTCH };
            const int ty = types[rnd() % 6];
            set_band(&p, t, k, ty, 20.0f * powf(1000.0f, urand()), 0, 0.707f);
            sprintf(last_edit, "add type %d on output %d slot %d", ty, t, k);
        } else if (r % 389 == 0 && k > 0) {               /* remove the last band */
            p.band[t][k - 1].type = BAND_OFF;
            sprintf(last_edit, "remove output %d slot %d", t, k - 1);
        } else if (r % 401 == 0 && k > 0) {               /* change a type */
            const int s = (int)(rnd() % (unsigned)k);
            p.band[t][s].type = (uint8_t)(BAND_PEAK + rnd() % 6);
            sprintf(last_edit, "type of output %d slot %d -> %d", t, s, p.band[t][s].type);
        } else if (k > 0) {                               /* drag */
            const int s = (int)(rnd() % (unsigned)k), what = (int)(rnd() % 4);
            eq_band_t *d = &p.band[t][s];
            const float u = urand();
            if (what == 0) d->gain_cdb = (int16_t)(-1500 + 3000 * u);
            else if (what == 1) d->freq_hz = (uint16_t)(20.0f * powf(1000.0f, u));
            else if (what == 2) d->q_milli = (uint16_t)(100.0f * powf(200.0f, u));
            else d->gain_cdb = 1200;
            sprintf(last_edit, "drag output %d slot %d (type %d) %s -> f %u Hz, %d cdB, Q %u m", t, s, d->type,
                    what == 0 ? "gain" : what == 1 ? "freq" : what == 2 ? "Q" : "+12", d->freq_hz, d->gain_cdb,
                    d->q_milli);
        } else continue;
        write_profile(&p);
        run(7);
    }
    printf("frames %ld: worst post-stage input %g, worst compressor gain %.1f dB, bad DAC samples %d, first bad "
           "frame %ld\n", frame_no, worst_in, 20 * log10f(worst_cg + 1e-30f), bad_dac, bad_frame);
    return bad_frame >= 0;
}
