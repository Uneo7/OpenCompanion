/* A preset change fades the outputs out for one frame, resets the engine to its power-up state and fades back
 * in. Bypass keeps the preset (it empties the user EQ and turns the loudness off). */
#include "oc.h"
#include "engine.h"
#include "bose_graph.h"
#include "bose_tables.h"
#include "eq.h"
#include "inputs.h"
#include "rate.h"
#include "kern.h"
#include "sub_level.h"
#include "prof.h"
#include "audio_out.h"
#include "protect.h"
#include "profiles.h"

#define FULL_SCALE   2147483648.0f       /* 2^31: 24-bit sample left-justified in 32 bits */
#define DOWNMIX      0.70710678f         /* 5.1 -> 2.1: centre and surrounds at -3 dB */

static int bose_on, switching, swap;      /* swap: a structural change is pending (engine_profile_swap) */
static int commit;                        /* the batch is complete (OPX, or a whole profile installed) */
static uint32_t quiet;                    /* silent frames since the last structural write */
#define SETTLE_FRAMES 25u                 /* 50 ms without a write closes a batch without OPX */
/* Neutral input matrix (bose_progs.c BG_H_MTX sums): L = L + k C + k Ls, R = R + k C + k Rs, S = kl LFE,
 * L += aux AUX L, R += aux AUX R, bass = S + (L + R) / 2; k, kl, aux are set every frame. */
static float mtx[14] = { 1.0f, 0.0f, 0.0f, 1.0f, 0.0f, 0.0f, 0.0f, 1.0f, 0.0f, 1.0f, 0.0f, 1.0f, 0.5f, 0.5f };

int audio_graph_active(void) { return bose_on; }
/* 2.1 routing follows the TAS mode ('R' bit1, rate.h LAYOUT_21) on the neutral preset. */
int audio_route21_active(void) { return !bose_on && g_layout == LAYOUT_21; }

static void select_preset(int bose)
{
    bose_on = bose;
    bose_graph_select(bose ? &bg_prog_bose : &bg_prog_neutral);
    eq_preset(!bose, audio_route21_active());
    eq_settle(0);                             /* outputs silent: no glide of the crossover */
}

void engine_init(void)
{
    switching = 0;
    select_preset(0);
}

int engine_bass_delay(void) { return (bose_graph_program()->flags & BGP_BASS_DELAY) != 0; }

/* A whole profile replaces the active one (load, pod double press, OPK, OPD): its coefficients, trims, sub
 * level, AUX gain and loudness can all differ, and gliding between two unrelated EQ sets (section by section)
 * passes through filters that boost the bass ("pocs" heard on hardware). The switch is done behind the output
 * ramp instead: 15 ms close, settle every glide and start the new EQ at rest, 15 ms open. */
void engine_profile_swap(void)
{
    swap = 1;
    quiet = 0;                                /* wait for the end of the batch */
    eq_hold = 1;                              /* the old set plays on while the ramp closes */
}

void engine_commit(void) { if (swap) commit = 1; }

/* A non-finite sample reached the post-stage (audio_out.c): every state of the engine back to rest. */
void engine_recover(void)
{
    bose_graph_init();
    eq_settle(1);
}
int engine_swap_pending(void) { return swap; }

float engine_frame_gate(void)
{
    const int want = (g_profile.flags & PF_STOCK_GRAPH) && bose_tables_ok() && !g_audio.headphones;
    if (switching || swap) {                  /* output ramp closing (audio_out.c) */
        if (!audio_out_silent()) return 0.0f;
        if (swap && !commit && ++quiet < SETTLE_FRAMES) return 0.0f;   /* more writes may come */
        if (switching || want != bose_on) select_preset(want);
        eq_settle(swap);
        protect_comp_reset();                 /* the new set starts at rest, the protection too */
        switching = swap = commit = eq_hold = 0;
        return 1.0f;
    }
    if (want != bose_on) {
        switching = 1;
        return 0.0f;
    }
    eq_preset(!bose_on, audio_route21_active());          /* layout / routage 2.1 changes */
    return 1.0f;
}

float engine_fill(const rx_buf_t *in, float *x[BG_IN_COUNT])
{
    const bg_prog_t *p = bose_graph_program();
    const int stereo = g_layout == LAYOUT_21;           /* 24-bit stereo stream: C, LFE, Ls, Rs unused */
    int c;
    for (c = 0; c < BG_IN_COUNT; c++) {
        const int32_t *src = c < BG_IN_AUX_L ? (*in)[0][c] : (*in)[1][c - BG_IN_AUX_L];
        const float s = (stereo && c >= BG_IN_C && c < BG_IN_AUX_L) ? 0.0f :
                        c < BG_IN_AUX_L ? p->usb_level / FULL_SCALE : 1.0f / FULL_SCALE;
        x[c] = bose_graph_input(c);
        k_i2f(x[c], src, s, FRAME_LEN);
    }
    return 1.0f / p->usb_level;
}

void engine_run(const float *usb_gain, float (*post)[FRAME_LEN])
{
    const bg_prog_t *p = bose_graph_program();
    const float k = g_layout == LAYOUT_51 ? DOWNMIX : 0.0f, kl = k != 0.0f ? 1.0f : 0.0f;
    float *x[EQ_BANK_LANES];
    bg_ctl_t ctl;
    int c;
    mtx[1] = mtx[2] = mtx[4] = mtx[5] = k;
    mtx[6] = kl;
    mtx[8] = mtx[10] = g_eq_gains.aux;
    ctl.usb = usb_gain;
    ctl.loud_d = loudness_gain_direct();
    ctl.loud_b = loudness_gain_band();
    ctl.baddata = g_gpio.sel ? 1.0f : 0.0f;
    ctl.mtx = mtx;
    ctl.ueq = g_audio.headphones ? ((g_dev_opts & OPT_HP_EQ) ? 1u << EQ_BANK_GLOBAL : 0u) : ~0u;
    PROF(PROF_INPUT);
    if (p->flags & BGP_WRAP_EQ) {
        x[0] = bose_graph_input(BG_IN_L);
        x[1] = bose_graph_input(BG_IN_R);
        eq_bank_run(EQ_BANK_GLOBAL, x);
    }
    PROF(PROF_GEQ);
    bose_graph_run(&ctl);
    PROF(PROF_GRAPH);
    for (c = 0; c < EQ_BANK_LANES; c++) x[c] = bose_graph_output(c);
    if (p->flags & BGP_WRAP_EQ) eq_bank_run(EQ_BANK_OUT, x);
    if (g_audio.headphones) {                  /* headphone feed: inner L / R = L, R; the rest silent */
        for (c = 0; c < OUT_COUNT; c++)
            if (c < 2) k_copy(post[c], x[c], FRAME_LEN);
            else k_zero(post[c], FRAME_LEN);
        return;
    }
    for (c = 0; c < 4; c++) k_scale(post[c], x[c], g_eq_gains.trim[c], FRAME_LEN);
    k_copy(post[OUT_BASS_A], x[4], FRAME_LEN);
    sub_level_apply(post[OUT_BASS_A], FRAME_LEN);
    k_copy(post[OUT_BASS_B], post[OUT_BASS_A], FRAME_LEN);
}
