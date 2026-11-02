/* Master gain and the stock dynamic loudness (port of 0x1001AF08, instance 0x10010480), the only loudness
 * of the firmware (enabled by PF_LOUDNESS, skipped by PF_BYPASS; the "Bose d'origine" profile sets it).
 *
 * Stock chain (re/dsp/out/graph.txt): pod % -> curve 0x1001B140 -> gain 0x10009470 -> 0x1001BB6C ->
 * 0x100104E8 -> loudness 0x1001AF08 -> targets of a direct gain (0x100103FC) and of a band-pass path
 * (biquad 0x100103C0 then gain 0x10010430), summed by 0x1001CE10. The loudness module therefore applies the
 * pod gain itself. In OpenCompanion the gain it applies is the master volume (volume.c: USB volume, or the
 * pod curve in LOCAL mode); the detector input is measured before it, so the level estimate is the same as
 * stock (stock: detector after the USB volume, pod gain added in dB; here: detector before the master
 * volume, master dB added).
 *
 * 0x1001AF08 (module words w10.. = 94, 19, 111, -, 105, 0.2, 0.4, 74, 0.05673, -20, ..., w24 = 0.9998):
 *   env    = max(det, env * 0.9998)                      det = sum of the 6 channel powers (0x1001B400)
 *   env_dB = max(10 log10(env), -20)                     (0x1001AF54..0x1001AFB8; 0x1001CF24 = log10)
 *   L      = min(vol_dB + env_dB + 111, 105)
 *   boost  = L <= 74 ? 0.4 (74 - L) + 0.2 (94 - 74) : 0.2 (94 - L);  boost = min(boost, 19)
 *   direct = gain;  band = max(10^((vol_dB + boost) / 20) - gain, 0)
 * Gains 0x100103FC / 0x10010430: per-sample g[n] = b0 t[n] + b1 t[n-1] - c g[n-1], b0 = b1 = 0.00065407,
 * c = -0.998692 (~16 ms), both start at 0. Band-pass 0x100103C0 (coefficients 0x10009264): one section,
 * g = 0.00391165, (b1, b2, a1, a2) = (0, -1, 1.99211526, -0.992176712), centre ~48 Hz.
 * Without loudness: direct = gain, band = 0. The OpenCompanion safety ceiling and the mute scale both. */
#include "oc.h"
#include "inputs.h"
#include "biquad.h"
#include "mathx.h"
#include "rate.h"
#include "kern.h"

#define SM_C48   0.998692f            /* smoothing pole at 48 kHz */
#define ENV_P48  0.9998f              /* detector decay per block at 48 kHz */

loudness_t g_loud;

static float sm_p, sm_b, env_p;
static float env;
static float gd_cur, gd_prev_t, gb_cur, gb_prev_t;
static float gd[FRAME_LEN], gb[FRAME_LEN];

void loudness_rate(void)
{
    sm_p = SM_C48;
    sm_b = (1.0f - sm_p) * 0.5f;
    env_p = ENV_P48;
}

void loudness_init(void)
{
    loudness_rate();
    env = 0.0f;
    gd_cur = gd_prev_t = gb_cur = gb_prev_t = 0.0f;
    g_loud.gain_in = 0.0f;
    g_loud.g_direct = g_loud.g_band = 0.0f;
}

static void stock_loudness(float vol_db, const float pow6[LD_CHANNELS])
{
    float det = 0.0f, lvl, boost, g;
    int c;
    for (c = 0; c < LD_CHANNELS; c++) det += pow6[c];
    env *= env_p;
    if (det > env) env = det;
    g_loud.env_db = env > 0.0f ? mx_pow_db(env) : -20.0f;
    if (g_loud.env_db < -20.0f) g_loud.env_db = -20.0f;
    lvl = vol_db + g_loud.env_db + 111.0f;
    if (lvl > 105.0f) lvl = 105.0f;
    boost = lvl > 74.0f ? 0.2f * (94.0f - lvl) : 0.4f * (74.0f - lvl) + 0.2f * (94.0f - 74.0f);
    if (boost > 19.0f) boost = 19.0f;
    g_loud.level = lvl;
    g_loud.boost_db = boost;
    g = mx_db_to_lin(vol_db + boost) - g_loud.gain_in;
    g_loud.g_band = g > 0.0f ? g : 0.0f;
}

void loudness_frame(float master_db, int mute, float cap_lin, int enabled, const float pow6[LD_CHANNELS])
{
    float k;
    g_loud.gain_in = mute ? 0.0f : mx_db_to_lin(master_db);
    g_loud.enabled = enabled;
    g_loud.g_direct = g_loud.gain_in;
    g_loud.g_band = 0.0f;
    g_loud.boost_db = 0.0f;
    if (enabled && g_loud.gain_in > 0.0f)
        stock_loudness(master_db, pow6);
    /* OpenCompanion safety ceiling: the direct gain never exceeds cap_lin */
    k = (g_loud.g_direct > cap_lin) ? cap_lin / g_loud.g_direct : 1.0f;
    g_loud.g_direct *= k;
    g_loud.g_band *= k;
}

void loudness_gains(int n)
{
    k_smooth(gd, &gd_cur, &gd_prev_t, g_loud.g_direct, sm_b, sm_p, n);
    k_smooth(gb, &gb_cur, &gb_prev_t, g_loud.g_band, sm_b, sm_p, n);
}

const float *loudness_gain_direct(void) { return gd; }
const float *loudness_gain_band(void) { return gb; }
