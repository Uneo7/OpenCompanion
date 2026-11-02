/* Graph interpreter of the engine: runs a program (include/bose_graph.h bg_prog_t): the stock module program of
 * bose_graph_data.c ("Bose" preset) or a built-in one (bose_progs.c, "neutral" preset).
 * Every module class is a port of the stock class (re/dsp/notes/classes_audio.md); channels are processed in
 * order, input by input, as the stock code does, so the in-place reuse of the stock buffer pool is exact
 * (gen_bose_graph.py checks that no output overwrites an input still to be read).
 *   gain    0x1002C5B0  out = t in (constant targets; hooked gains use per-sample arrays from the controls)
 *   mixer   0x1002C260  out[k] = gA in[k] + gB in[n + k]; hooked mixers smooth their targets per sample,
 *                       g[n] = b (t[n] + t[n-1]) + p g[n-1], b = (1 - p) / 2, p = -c (stock word P2)
 *   sum     0x1002D668  weighted sum N -> 1          adder 0x1001CE10   router 0x1001C928 (copy)
 *   delay   0x1001AD94  delay line (< 1 frame)             biquad 0x1002C904 (per-channel coefficient set)
 *   meter   0x1002CDDC  block power (mode 1) -> spatial control 0x10027978 (bose_spatial.c)
 *   bass    0x10010938  stock bass compensation: only its fixed all-pass branch 0x100108C0 (it sets the bass
 *                       phase / polarity against the satellites); the knob-driven low-pass branch is not run
 *                       (the knob sets the sub level instead, README "Sub level")
 *   ueq     user EQ bank (eq.c), built-in programs only
 * Every per-sample loop is a block kernel (kern.h). Both branches of the BADDATA selector run continuously, as
 * stock (no transient when it switches). */
#include "bose_graph.h"
#include "biquad.h"
#include "rate.h"
#include "inputs.h"
#include "eq.h"

#define NHMIX 2                     /* hooked mixers: 0 = BADDATA 0x10010818, 1 = spatial 0x10010B58 */
#define HMIX_OUT 6

/* Work RAM in IRAM_HI (lnk.cmd .bose_ram): uninitialised, written by bose_graph_init() / _rate() before use. */
#ifdef __TI_COMPILER_VERSION__
#pragma DATA_SECTION(w, ".bose_ram")
#pragma DATA_SECTION(coef, ".bose_ram")
#pragma DATA_SECTION(z, ".bose_ram")
#pragma DATA_SECTION(dline, ".bose_ram")
#endif
static float w[BG_NWIRES][FRAME_LEN];
static float coef[BG_NF];           /* P->f with the biquad sets re-mapped to the current rate */
static float z[BG_NZ];
static float dline[BG_DLEN];
static const bg_prog_t *P = &bg_prog_bose;   /* program being run (bose_graph_select) */
static uint16_t dlen[BG_NDL];
DSP_SCRATCH(tv)
static float tv[FRAME_LEN];
static float meter[4], spat[12];
static float hm_cur[NHMIX][2 * HMIX_OUT], hm_prev[NHMIX][2 * HMIX_OUT], hm_pole[NHMIX];
static float scratch[4][FRAME_LEN], zdummy[4][2];                  /* unused lanes of k_sec4 */
static const float czero[4] = { 0.0f, 0.0f, 0.0f, 0.0f };
static float ga[FRAME_LEN], gb[FRAME_LEN];
/* All-pass branch of the stock bass compensation (biquad 0x100108C0, coefficients 0x10009C14) */
static const biquad_coef_t AP_SEC = { -2.05765986f, 1.05835032f, 1.94458425f, -0.945235074f };
static biquad_t ap;

/* y = x g (acc = 0) or y += x g (acc = 1); g = per-sample gains if non-null, else the constant t (kern.h:
 * scalar gain kept in a register, as the stock vector multiply 0x1002D880). */
static void axpy(float *y, const float *x, const float *g, float t, int acc)
{
    if (g) (acc ? k_macv : k_scalev)(y, x, g, FRAME_LEN);
    else   (acc ? k_mac : k_scale)(y, x, t, FRAME_LEN);
}

/* Hooked mixers: init = 1 copies the RAM targets into the state (stock init), else the poles for g_fs. */
static void hmix_setup(int init)
{
    int o, i;
    for (o = 0; o < P->nops; o++) {
        const bg_op_t *op = &P->ops[o];
        const float *t = &P->f[op->par];
        const int h = op->hook == BG_H_SPATIAL;
        if (op->kind != BG_MIX || op->hook == BG_H_NONE) continue;
        if (init) {
            for (i = 0; i < 2 * HMIX_OUT; i++) hm_cur[h][i] = hm_prev[h][i] = t[i];
            continue;
        }
        hm_pole[h] = t[2 * HMIX_OUT];
    }
}

void bose_graph_rate(void)
{
    int i;
    for (i = 0; i < P->nf; i++) coef[i] = P->f[i];
    for (i = 0; i < P->ndl; i++) {
        dlen[i] = P->dl[i].d48;
    }
    hmix_setup(0);
    biquad_init(&ap, 0.940839529f, &AP_SEC, 1);
}

void bose_graph_init(void)
{
    k_zero(&w[0][0], BG_NWIRES * FRAME_LEN);
    k_zero(z, BG_NZ);
    k_zero(dline, BG_DLEN);
    hmix_setup(1);
    bose_spatial_init();
    bose_graph_rate();
}

void bose_graph_select(const bg_prog_t *prog)
{
    P = prog;
    bose_graph_init();
}

const bg_prog_t *bose_graph_program(void) { return P; }

float *bose_graph_input(int ch) { return w[ch]; }

float *bose_graph_output(int k) { return w[P->out[k]]; }

/* Per-sample trajectory of a smoothed gain, the stock mixer law g[n] = b (t + t[n-1]) + p g[n-1],
 * b = (1 - p) / 2 (k_smooth). Settled (|g - t| <= 1e-7 t): null, the constant gain t is used. */
static const float *smooth(float *g, int h, int k, float t)
{
    const float d = hm_cur[h][k] - t, p = hm_pole[h];
    if (d * d <= 1e-14f * t * t && hm_prev[h][k] == t) {
        hm_cur[h][k] = t;
        return 0;
    }
    k_smooth(g, &hm_cur[h][k], &hm_prev[h][k], t, (1.0f - p) * 0.5f, p, FRAME_LEN);
    return g;
}

static void op_mix(const bg_op_t *op, const uint8_t *io, const bg_ctl_t *ctl)
{
    const int n = op->nout, h = op->hook == BG_H_SPATIAL;
    int k;
    for (k = 0; k < n; k++) {
        float ta = P->f[op->par + 2 * k], tb = P->f[op->par + 2 * k + 1];
        const float *pa = 0, *pb = 0;
        if (op->hook != BG_H_NONE) {
            ta = h ? spat[2 * k] : 1.0f - ctl->baddata;
            tb = h ? spat[2 * k + 1] : ctl->baddata;
            pa = smooth(ga, h, 2 * k, ta);
            pb = smooth(gb, h, 2 * k + 1, tb);
        }
        axpy(w[io[2 * n + k]], w[io[k]], pa, ta, 0);
        if (pb || tb != 0.0f)
            axpy(w[io[2 * n + k]], w[io[n + k]], pb, tb, 1);
    }
}

/* Delay module: every stock delay is shorter than a frame (0..60 samples at 48 kHz), so its line holds the
 * last d input samples in order: y = line, x[0..n-d); line = x[n-d..n). */
static void op_delay(const bg_op_t *op, const uint8_t *io)
{
    const int d = dlen[op->par];
    float *line = &dline[P->dl[op->par].off];
    const float *x = w[io[0]];
    float *y = w[io[1]];
    if (y == x) {
        k_copy(tv, x, FRAME_LEN);
        x = tv;
    }
    if (d) k_copy(y, line, d);
    k_copy(y + d, x, FRAME_LEN - d);
    if (d) k_copy(line, x + FRAME_LEN - d, d);
}

/* Biquad module: gains, then the sections 4 channels at a time (idle lanes: zero section on scratch). */
static void op_biquad(const bg_op_t *op, const uint8_t *io)
{
    const bg_bq_t *q = &P->bq[op->par];
    const int ni = op->nin, stride = 1 + 4 * q->nsec;
    float *y[4], *zz[4];
    const float *c[4];
    int k, i, s;
    for (k = 0; k < ni; k++)
        k_scale(w[io[ni + k]], w[io[k]], coef[q->coef + q->set[k] * stride], FRAME_LEN);
    for (k = 0; k < ni; k += 4) {
        for (i = 0; i < 4; i++) {
            const int ch = k + i, real = ch < ni;
            y[i] = real ? w[io[ni + ch]] : scratch[i];
            c[i] = real ? &coef[q->coef + q->set[ch] * stride + 1] : czero;
            zz[i] = real ? &z[q->z + 2 * q->nsec * ch] : zdummy[i];
        }
        for (s = 0; s < q->nsec; s++) {
            k_sec4(y, c, zz, FRAME_LEN);
            for (i = 0; i < 4 && k + i < ni; i++) {
                c[i] += 4;
                zz[i] += 2;
            }
        }
    }
}

void bose_graph_run(const bg_ctl_t *ctl)
{
    float *x[EQ_BANK_LANES];
    int o, k;
    for (o = 0; o < P->nops; o++) {
        const bg_op_t *op = &P->ops[o];
        const uint8_t *io = &P->io[op->io];
        const int ni = op->nin, no = op->nout, kind = op->kind;
        const float *wt = op->hook == BG_H_MTX ? &ctl->mtx[op->par] : &P->f[op->par];
        const float *g = op->hook == BG_H_USB ? ctl->usb : op->hook == BG_H_LOUD_D ? ctl->loud_d :
                         op->hook == BG_H_LOUD_B ? ctl->loud_b : 0;
        switch (kind) {
        case BG_GAIN:
            for (k = 0; k < ni; k++) axpy(w[io[ni + k]], w[io[k]], g, wt[k], 0);
            break;
        case BG_MIX:    op_mix(op, io, ctl); break;
        case BG_DELAY:  op_delay(op, io); break;
        case BG_BIQUAD: op_biquad(op, io); break;
        case BG_SUM:    /* weighted sum (weights: P->f, or the run-time matrix), or adder / router */
        case BG_ADD:
        case BG_COPY:
            for (k = 0; k < ni; k++) {
                const int j = kind == BG_SUM ? 0 : (k >= no ? k - no : k);   /* ni <= 2 no */
                axpy(w[io[ni + j]], w[io[k]], 0, kind == BG_SUM ? wt[k] : 1.0f, k >= no);
            }
            break;
        case BG_UEQ:
            if (!(ctl->ueq >> op->par & 1u)) break;
            for (k = 0; k < ni; k++) x[k] = w[io[k]];
            eq_bank_run(op->par, x);
            break;
        case BG_METER:
            for (k = 0; k < ni; k++) meter[k] = k_sumsq(w[io[k]], FRAME_LEN) * wt[0];
            break;
        case BG_SPATIAL:
            bose_spatial_block(meter, spat);
            break;
        case BG_BASS: { /* stock bass compensation 0x10010938: its all-pass branch only (README "Sub level") */
            biquad_t *b[4] = { &ap, 0, 0, 0 };
            const float *in[4];
            float *out[4];
            in[0] = w[io[0]];
            out[0] = w[io[1]];
            out[1] = scratch[1]; out[2] = scratch[2]; out[3] = scratch[3];
            biquad_run4(b, in, out, FRAME_LEN);
            break;
        }
        default: break;
        }
    }
}
