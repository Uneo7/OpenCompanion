/* Coefficient values are the stock ones, taken from bg_f / bg_bq (BADDATA low-pass 0x10009800, loudness
 * band-pass 0x10009264, selector 0x10010818). The stock program keeps the stock bass timing (not delayed). */
#include "bose_graph.h"
#include "eq.h"

/* USB channels into the stock graph at the stock input margin (gain 0x100106C8 target, -9.36 dB), so that
 * both presets play at about the same level for the same OS volume (README "Bose d'origine"). */
#define BG_USB_LEVEL 0.340408176f

const bg_prog_t bg_prog_bose = {
    bg_ops, bg_io, bg_bq, bg_dl, bg_f, bg_out, BG_NF, BG_NOPS, BG_NBQ, BG_NDL, BGP_WRAP_EQ, BG_USB_LEVEL
};

/* Wires: 0..7 inputs (L, R, C, LFE, Ls, Rs, AUX L, AUX R), then outputs 0..4; 8..10 L, R, S; 11..13 scratch. */
static const bg_op_t nt_ops[] = {
    { BG_GAIN,   6, 6, BG_H_USB,     0, 0 },      /* USB mute ramp */
    { BG_SUM,    3, 1, BG_H_MTX,    12, 0 },      /* L = L + k C + k Ls */
    { BG_SUM,    3, 1, BG_H_MTX,    16, 3 },      /* R = R + k C + k Rs */
    { BG_SUM,    1, 1, BG_H_MTX,    20, 6 },      /* S = kl LFE */
    { BG_BIQUAD, 3, 3, BG_H_NONE,   22, 1 },      /* BADDATA low-pass branch (stock set bg_bq[1]) */
    { BG_MIX,    6, 3, BG_H_BADDATA, 28, 20 },    /* selector (stock targets and pole, bg_f[20..32]) */
    { BG_SUM,    2, 1, BG_H_MTX,    37, 7 },      /* L += aux AUX L */
    { BG_SUM,    2, 1, BG_H_MTX,    40, 9 },      /* R += aux AUX R */
    { BG_BIQUAD, 3, 3, BG_H_NONE,   43, 3 },      /* loudness band-pass (stock set bg_bq[3]) */
    { BG_GAIN,   3, 3, BG_H_LOUD_D, 49, 0 },      /* master volume (direct) */
    { BG_GAIN,   3, 3, BG_H_LOUD_B, 55, 0 },      /* loudness band gain */
    { BG_ADD,    6, 3, BG_H_NONE,   61, 0 },
    { BG_UEQ,    2, 0, BG_H_NONE,   70, EQ_BANK_GLOBAL },
    { BG_COPY,   4, 4, BG_H_NONE,   72, 0 },      /* inner L, inner R, outer L, outer R */
    { BG_SUM,    3, 1, BG_H_MTX,    80, 11 },     /* bass = S + (L + R) / 2 */
    { BG_UEQ,    5, 0, BG_H_NONE,   84, EQ_BANK_OUT },
};

static const uint8_t nt_io[] = {
    0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5,
    0, 2, 4, 8,
    1, 2, 5, 9,
    3, 10,
    8, 9, 10, 11, 12, 13,
    8, 9, 10, 11, 12, 13, 8, 9, 10,
    8, 6, 8,
    9, 7, 9,
    8, 9, 10, 11, 12, 13,
    8, 9, 10, 8, 9, 10,
    11, 12, 13, 11, 12, 13,
    8, 9, 10, 11, 12, 13, 8, 9, 10,
    8, 9,
    8, 9, 8, 9, 0, 1, 2, 3,
    10, 8, 9, 4,
    0, 1, 2, 3, 4,
};

static const uint8_t nt_out[5] = { 0, 1, 2, 3, 4 };

/* Coefficients: the stock table bg_f and biquad sets bg_bq (BADDATA low-pass, loudness band-pass); only the
 * sets the program uses are run, but all are re-mapped at a rate change (bose_graph_rate). */
const bg_prog_t bg_prog_neutral = {
    nt_ops, nt_io, bg_bq, 0, bg_f, nt_out, BG_NF, sizeof(nt_ops) / sizeof(nt_ops[0]), BG_NBQ, 0, BGP_BASS_DELAY, 1.0f
};
