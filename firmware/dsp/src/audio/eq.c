/* On the neutral preset each output lane also carries the stock crossover (satellites: high-pass 0x10009D6C
 * twice, ~170 Hz; bass: low-pass 0x10009C58), always on, so it stays with bypass; crossover_hz is reserved. */
#include <string.h>
#include "eq.h"
#include "bose_tables.h"
#include "oc.h"
#include "rate.h"
#include "mathx.h"
#include "bose_graph_size.h"
#include "kern.h"
#include "engine.h"

eq_gains_t g_eq_gains;

static eq_chain_t global_lr[2];
static eq_chain_t out_eq[EQ_OUTPUTS];
static int xover_on, route21;               /* eq_preset() */
static uint32_t n_sections;

/* Stock Bose crossover (re/dsp/out/bose_params.json): satellites 4th-order high-pass (filter 0x10009D6C
 * applied twice, ~170 Hz), bass 4th-order low-pass (filter 0x10009C58). */
static const biquad_coef_t BOSE_HPF_SAT[2] = {
    { -2.0f, 1.0f, 1.962980031967163f, -0.9636529684066772f },
    { -2.0f, 1.0f, 1.962980031967163f, -0.9636529684066772f },
};
static const float BOSE_HPF_SAT_G = 0.9816582798957825f * 0.9816582798957825f;
static const biquad_coef_t BOSE_LPF_SUB[2] = {
    { 2.0f, 1.0f, 1.928221583366394f, -0.9287218451499939f },
    { 2.0f, 1.0f, 1.9595320224761963f, -0.9603347182273865f },
};
static const float BOSE_LPF_SUB_G = 2.816600463972918e-08f;

void eq_bank_run(int bank, float *const x[])
{
    if (bank == EQ_BANK_GLOBAL) {
        eq_chain_run(global_lr, x, 2);
        return;
    }
    eq_chain_run(out_eq, x, 4);
    eq_chain_run(&out_eq[EQ_SUB - 1], &x[4], 1);
}

static float cdb_to_lin(int cdb) { return mx_db_to_lin((float)cdb * 0.01f); }

void eq_init(void)
{
    int k;
    chain_init(&global_lr[0], 1.0f, 0, 0);
    chain_init(&global_lr[1], 1.0f, 0, 0);
    for (k = 0; k < EQ_OUTPUTS; k++) chain_init(&out_eq[k], 1.0f, 0, 0);
    g_eq_gains.aux = 1.0f;
    g_eq_gains.sub = 1.0f;
    for (k = 0; k < EQ_OUTPUTS; k++) g_eq_gains.trim[k] = 1.0f;
    xover_on = route21 = 0;
    n_sections = 0;
}

static uint32_t count_bands(const eq_band_t *bands)
{
    uint32_t b, n = 0;
    for (b = 0; b < EQ_MAX_BANDS; b++)
        n += (uint8_t)(bands[b].type - 1) < BAND_TYPE_COUNT - 1;   /* PEAK..NOTCH, not disabled */
    return n;
}

/* Bands that feed output target t (link L/R, routage 2.1). */
static int band_src(const profile_t *p, int t)
{
    if (route21 && (t == EQ_OUTER_L || t == EQ_OUTER_R)) t -= 2;          /* outer plays the inner signal */
    if ((p->flags & PF_LINK_LR) && (t == EQ_INNER_R || t == EQ_OUTER_R)) t--;
    return t;
}

void eq_settle(int rest)
{
    int k;
    for (k = 0; k < 2; k++) chain_settle(&global_lr[k], rest);
    for (k = 0; k < EQ_OUTPUTS; k++) chain_settle(&out_eq[k], rest);
}

void eq_preset(int xover, int r21)
{
    if (xover == xover_on && r21 == route21) return;
    xover_on = xover;
    route21 = r21;
    eq_rebuild(&g_profile, DIRTY_TARGET(EQ_INNER_L) | DIRTY_TARGET(EQ_INNER_R) | DIRTY_TARGET(EQ_OUTER_L) |
                           DIRTY_TARGET(EQ_OUTER_R) | DIRTY_TARGET(EQ_SUB));
}

void eq_rebuild(const profile_t *p, uint32_t dirty)
{
    static const eq_band_t none[EQ_MAX_BANDS];
    biquad_coef_t sec[BIQUAD_MAX_SECTIONS];
    int repack = 0;                      /* a section count changed: switch behind the output ramp */
    float g;
    int n, t, k;
    const int bypass = (p->flags & PF_BYPASS) != 0;

    if (dirty & DIRTY_TARGET(EQ_GLOBAL)) {
        n = eq_compile(bypass ? none : p->band[EQ_GLOBAL], sec, &g);
        repack |= chain_set(&global_lr[0], g, sec, n);
        repack |= chain_set(&global_lr[1], g, sec, n);
    }
    for (t = EQ_INNER_L; t <= EQ_SUB; t++) {
        if (!(dirty & DIRTY_TARGET(t))) continue;
        n = eq_compile(bypass ? none : p->band[band_src(p, t)], sec, &g);
        if (xover_on) {                                  /* stock crossover after the user bands */
            const int bass = t == EQ_SUB;
            float gx = bass ? BOSE_LPF_SUB_G : BOSE_HPF_SAT_G;
            sec[n] = (bass ? BOSE_LPF_SUB : BOSE_HPF_SAT)[0];
            sec[n + 1] = (bass ? BOSE_LPF_SUB : BOSE_HPF_SAT)[1];
            n += 2;
            g *= gx;
        }
        repack |= chain_set(&out_eq[t - 1], g, sec, n);
    }
    if (repack) engine_profile_swap();
    if (dirty & DIRTY_GAINS) {
        g_eq_gains.aux = cdb_to_lin(p->aux_gain_cdb);
        g_eq_gains.sub = cdb_to_lin(p->sub_level_cdb);
        for (k = 0; k < EQ_OUTPUTS; k++) g_eq_gains.trim[k] = cdb_to_lin(p->trim_cdb[k]);
    }

    n_sections = bypass ? 0 : 2 * count_bands(p->band[EQ_GLOBAL]);
    for (t = EQ_INNER_L; t <= EQ_SUB && !bypass; t++) n_sections += count_bands(p->band[band_src(p, t)]);
}

uint32_t eq_active_sections(void)
{
    /* engine: the program's sections (stock graph 142; neutral: BADDATA 6, loudness 3, crossover 10), the user
     * EQ, the satellite band protection 32 */
    return n_sections + 32 + (audio_graph_active() ? BG_SECTIONS : 19);
}
