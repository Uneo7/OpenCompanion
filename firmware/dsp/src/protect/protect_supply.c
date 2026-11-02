/* Supply (VRAW) dependent output ceiling and the supply / clip fault detector, port of two stock control
 * classes (decoded from re/dsp/out/img.dis; addresses below are instruction addresses of the stock code).
 *
 * 1. Class 0x1001BB6C, instance 0x100105C4 (inputs VRAW 0x10010148, pod gain 0x10009470; 0x1001014C = 1.0
 *    set at 0x10019F6C, 0x100105C0 = 0):
 *      V  = clamp(VRAW, 0, 12)                                            0x1001BBEC..0x1001BC20
 *      Vs = V < Vs ? V : 0.99866754 Vs + 0.00133244 V   (falls at once, rises with tau ~1.5 s)  ..0x1001BC9C
 *      x  = Vs / 10.5, x0 = 7.0035 / 10.5
 *      c  = x < x0 ? x0 (x / x0)^5 : x;   c = min(0.89145 c, 0.89145)     0x1001BD10..0x1001BDAC
 *      out1 = c -> final gain targets (0x10009490 -> thermal gain 0x1001B638 -> gain 0x10011C74)
 *      out0 = 10^(min(pg_dB + boost, +5) / 20), boost = -0.99804 - 20 log10(c)  (pre-gain, keeps the
 *             overall level when c drops; skipped when pg_dB <= -90)      0x1001BE14..0x1001BEB8
 *    So the outputs get -1 dB (0.89145) above 10.5 V, -1 dB x Vs/10.5 down to 7.0 V, then -5 dB per dB of
 *    supply below the knee; the pre-gain compensates up to +5 dB absolute.
 *    OpenCompanion: out1 is the final gain (with the thermal gain, gpio_in.c) after the band protection
 *    (protect_band.c), as in stock; out0's boost is added to the master volume BEFORE the OpenCompanion
 *    -12 dB ceiling, so near the ceiling the boost is clipped and the output simply gets quieter.
 *    Init (0x1001BB44..0x1001BB68): Vs = 10.5 V (state word 16 = word 10), so the ceiling starts at
 *    0.89145 and only a supply measured below 10.5 V lowers it.
 * 2. Class 0x1001B820, instance 0x10010670 (inputs: peaks of the 5 outputs after the final gain, meter
 *    0x10011C4C mode 0 with decay 0.999792 per sample; VRAW; flag 0x10010158 = CCLIP\ or RT6307 hot):
 *      lvl  = 16.55 x max(peaks)                                          0x1001B834..0x1001B8F4
 *      hold = max(flag, 0.9802 hold)                                      0x1001B8FC..0x1001B960
 *      s    = lvl < 0.25 ? -1 : (lvl > VRAW - 4 ? +1 : 0)                 0x1001B948, 0x1001B98C..0x1001B9B0
 *      ripple = max - min of the last 20 VRAW values (40 ms)              0x1001B9B4..0x1001BA88
 *      c1 = (s == -1 && ripple > 0.83) ? c1 + 1 : 0;  out0 = c1 >= 5000   0x1001BA8C..0x1001BAFC
 *      c2 = (hold > e^-1 && s != +1)   ? c2 + 1 : 0;  out1 = c2 >= 5000   0x1001BADC..0x1001BB40
 *    i.e. supply ripple while the output is quiet, or clip / heat flags that are not explained by output
 *    at the rail, for 10 s without interruption.
 * 3. Latch 0x10015910 (called from the frame task at 0x10015EA0): out0 || out1 -> mute (0x1001010C = 1.0,
 *    which mutes the pod curve through 0x1001C780), standby request byte 0x540E = 1, fault byte 0x540F = 1
 *    (both sent in the 'S' reply: the TAS enters standby), McASP0 PDIR bit 14 cleared. Cleared only by a
 *    reset (0x10019ECC). OpenCompanion: same latch; AXR0[14] is released by spk_mode.c. */
#include "oc.h"
#include "kern.h"
#include "inputs.h"
#include "protect.h"
#include "ramp.h"
#include "mathx.h"
#include "rate.h"

#define HIST          20
#define FAULT_BLOCKS  5000u
#define PEAK_DECAY48  0.999792f
#define HOLD_DECAY48  0.9802f
#define VS_RISE48     0.99866754f

supply_t g_supply;

static float hist[HIST];
static float peak[OUT_COUNT];
static float peak_p, hold_p, vs_p;
static uint32_t c1_max;

void supply_rate(void)
{
    peak_p = PEAK_DECAY48;
    hold_p = HOLD_DECAY48;
    vs_p = VS_RISE48;
    c1_max = FAULT_BLOCKS;
}

void supply_init(void)
{
    int i;
    for (i = 0; i < HIST; i++) hist[i] = 0.0f;
    for (i = 0; i < OUT_COUNT; i++) peak[i] = 0.0f;
    g_supply.vs = 10.5f;                                /* init 0x1001BB50: state w16 = w10 = 10.5 V */
    g_supply.ceiling = 0.89145f;
    g_supply.boost_db = 0.0f;
    g_supply.hold = 0.0f;
    g_supply.c1 = g_supply.c2 = 0;
    g_supply.fault = 0;
    supply_rate();
}

/* 0x1001BB6C: once per frame, before the gains are used. */
void supply_frame(void)
{
    float v = g_ramp.vraw, x, c;
    const float x0 = 7.0035f / 10.5f;
    if (v > 12.0f) v = 12.0f;
    if (v < 0.0f) v = 0.0f;
    g_supply.vs = v < g_supply.vs ? v : g_supply.vs * vs_p + v * (1.0f - vs_p);
    x = g_supply.vs / 10.5f;
    c = x < x0 ? x0 * mx_exp2(5.0f * mx_log2(x / x0)) : x;
    c *= 0.89145f;
    if (c > 0.89145f) c = 0.89145f;
    g_supply.ceiling = c;
    g_supply.boost_db = c > 0.0f ? -0.99804f - mx_lin_to_db(c) : 0.0f;
    if (g_supply.boost_db < 0.0f) g_supply.boost_db = 0.0f;
}

/* 0x1001B820 + 0x10015910: after the final gain, on the DAC samples. */
void supply_detect(float (*out)[96], int n)
{
    float lvl = 0.0f, mx, mn, f;
    int k, i;
    for (k = 0; k < OUT_COUNT - 1; k++) {               /* stock meters 5 outputs (one bass) */
        const float p = k_decay_peak(out[k], peak[k], peak_p, n);
        peak[k] = p;
        if (p > lvl) lvl = p;
    }
    lvl *= 16.55f;
    f = (g_gpio.cclip || g_gpio.t2) ? 1.0f : 0.0f;      /* 0x10010158 (gpio_in.c, 0x10015980) */
    g_supply.hold *= hold_p;
    if (f > g_supply.hold) g_supply.hold = f;
    for (i = HIST - 1; i > 0; i--) hist[i] = hist[i - 1];
    hist[0] = g_ramp.vraw;
    mx = mn = hist[0];
    for (i = 1; i < HIST; i++) {
        if (hist[i] > mx) mx = hist[i];
        if (hist[i] < mn) mn = hist[i];
    }
    if (lvl < 0.25f && mx - mn > 0.83f) { if (g_supply.c1 < c1_max) g_supply.c1++; } else g_supply.c1 = 0;
    if (g_supply.hold > 0.36787944f && !(lvl >= 0.25f && lvl > g_ramp.vraw - 4.0f)) {
        if (g_supply.c2 < c1_max) g_supply.c2++;
    } else {
        g_supply.c2 = 0;
    }
    if ((g_supply.c1 >= c1_max || g_supply.c2 >= c1_max) && !g_supply.fault) {
        g_supply.fault = 1;                             /* latched until reset (stock 0x10019ECC) */
        g_standby.request = 1;
        g_standby.fault = 1;
    }
}
