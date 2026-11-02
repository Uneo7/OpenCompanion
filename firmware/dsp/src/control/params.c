#include <string.h>
#include "params.h"
#include "engine.h"
#include "eq.h"
#include "oc.h"

profile_t g_profile;

static int16_t clamp16(int v, int lo, int hi) { return (int16_t)(v < lo ? lo : (v > hi ? hi : v)); }

void params_defaults(profile_t *p)
{
    /* Default band frequencies: log-spaced 32 Hz .. 16 kHz, all bands off */
    static const uint16_t freqs[EQ_MAX_BANDS] = { 32, 55, 95, 160, 280, 480, 820, 1400, 2400, 4100, 7000, 12000 };
    int t, b;
    memset(p, 0, sizeof(*p));
    p->version = PROFILE_VERSION;
    strcpy(p->name, "Flat");
    for (t = 0; t < EQ_TARGETS; t++)
        for (b = 0; b < EQ_MAX_BANDS; b++) {
            p->band[t][b].type = BAND_OFF;
            p->band[t][b].freq_hz = freqs[b];
            p->band[t][b].gain_cdb = 0;
            p->band[t][b].q_milli = 707;
        }
}


void params_sanitize(profile_t *p)
{
    int t, b, i;
    p->version = PROFILE_VERSION;
    p->flags &= PF_BYPASS | PF_LINK_LR | PF_LOUDNESS | PF_VOL_STOCK | PF_STOCK_GRAPH | PF_ADVANCED;
    p->reserved0 = 0;
    p->sub_level_cdb = clamp16(p->sub_level_cdb, -1200, 1200);
    p->crossover_hz = 0;                                   /* reserved: stock crossover only */
    p->aux_gain_cdb = clamp16(p->aux_gain_cdb, -2400, 1200);
    for (i = 0; i < EQ_OUTPUTS; i++)
        p->trim_cdb[i] = clamp16(p->trim_cdb[i], -1200, 600);
    p->reserved36 = 0;
    memset(p->reserved1, 0, sizeof(p->reserved1));
    for (t = 0; t < EQ_TARGETS; t++) {
        for (b = 0; b < EQ_MAX_BANDS; b++) {
            eq_band_t *bd = &p->band[t][b];
            if (bd->type >= BAND_TYPE_COUNT &&                  /* only PEAK..NOTCH may carry BAND_DISABLED */
                (uint8_t)(bd->type - (BAND_DISABLED | BAND_PEAK)) > BAND_NOTCH - BAND_PEAK) bd->type = BAND_OFF;
            bd->reserved = 0;
            bd->freq_hz = (uint16_t)clamp16(bd->freq_hz, 20, 20000);
            bd->gain_cdb = clamp16(bd->gain_cdb, -1500, 1500);
            bd->q_milli = (uint16_t)(bd->q_milli < 100 ? 100 : (bd->q_milli > 20000 ? 20000 : bd->q_milli));
        }
    }
}

static uint32_t dirty_for_range(uint32_t off, uint32_t n)
{
    uint32_t dirty = 0, end = off + n, t;
    if (off < 2 && end > 1) dirty |= DIRTY_ALL;                     /* flags: bypass / link / volume law */
    if (off < 24 && end > 20) dirty |= DIRTY_GAINS;                  /* sub level */
    if (off < 36 && end > 24) dirty |= DIRTY_GAINS;                  /* AUX gain, trims */
    for (t = 0; t < EQ_TARGETS; t++) {
        uint32_t lo = PROFILE_BANDS_OFF + t * EQ_MAX_BANDS * sizeof(eq_band_t);
        uint32_t hi = lo + EQ_MAX_BANDS * sizeof(eq_band_t);
        if (off < hi && end > lo) {
            dirty |= DIRTY_TARGET(t);
            if (g_profile.flags & PF_LINK_LR) {                      /* left bands also drive the right */
                if (t == EQ_INNER_L) dirty |= DIRTY_TARGET(EQ_INNER_R);
                if (t == EQ_OUTER_L) dirty |= DIRTY_TARGET(EQ_OUTER_R);
            }
        }
    }
    return dirty;
}

int params_read(uint32_t off, uint8_t *dst, uint32_t n)
{
    if (off > PROFILE_SIZE || n > PROFILE_SIZE - off) return -1;
    memcpy(dst, (const uint8_t *)&g_profile + off, n);
    return (int)n;
}

/* Structural change: the flags (base, bypass, link L/R, loudness, EQ mode) or the type of any band slot (incl.
 * its enabled bit). Gain / frequency / Q edits of existing bands are not: they glide (eq_chain.c). */
static int structural(const profile_t *a, const profile_t *b)
{
    int t, k;
    if (a->flags != b->flags) return 1;
    for (t = 0; t < EQ_TARGETS; t++)
        for (k = 0; k < EQ_MAX_BANDS; k++)
            if (a->band[t][k].type != b->band[t][k].type) return 1;
    return 0;
}

/* A write that changes the structure, or any write while such a change is pending, goes behind the output ramp:
 * the engine closes the ramp, waits until the writes stop (or OPX, engine_commit()), installs everything at
 * rest and reopens (engine.c). A multi-write batch (Simple -> Avance, a profile written by OPW) is thus heard
 * as one 15 ms fade out / in, never as its partial states. */
int params_write(uint32_t off, const uint8_t *src, uint32_t n)
{
    static profile_t before;
    if (n == 0 || off > PROFILE_SIZE || n > PROFILE_SIZE - off) return -1;
    before = g_profile;
    memcpy((uint8_t *)&g_profile + off, src, n);
    params_sanitize(&g_profile);
    params_apply(dirty_for_range(off, n));
    if (engine_swap_pending() || structural(&before, &g_profile)) engine_profile_swap();
    return 0;
}

void params_set_active(const profile_t *p)
{
    g_profile = *p;
    params_sanitize(&g_profile);
    params_apply(DIRTY_ALL);
    engine_profile_swap();                 /* behind the 15 ms output ramp (engine.c), complete: no wait */
    engine_commit();
}

void params_apply(uint32_t dirty)
{
    eq_rebuild(&g_profile, dirty);
    audio_update_volume();                 /* the volume law may have changed */
}
