/* McASP0 GPIO inputs and the stock protective actions (pin map: re/dsp/notes/uncertainties.md #11).
 *
 * Stock frame task 0x10015AE8 (every 2 ms):
 *   0x100156C8  BADDATA: PDIN bit 15 -> requested selection of the cross-fade selector 0x10010758
 *               (0x10010794 = 1 if high, else 0). The selector (class 0x1001AC00) switches the 6 USB channels
 *               between branch A, an all-pass (biquad 0x100107A0, coefficients 0x1000971C), and branch B, a
 *               4th-order low-pass (biquad 0x100107DC, coefficients 0x10009800), through mixer 0x10010818
 *               (c = -0.767327, ~0.1 ms) after a delay of 4 blocks. So a USB checksum error low-passes the USB
 *               input (masks the glitches) instead of muting it.
 *   0x10015980  thermistors and clip detect: PDIN bit 4 low (RT6206 hot) -> 0x1001015C = 1; bit 5 low
 *               (RT6307 hot) -> 0x1001015C = 1 and 0x10010158 = 1; bit 7 low (CCLIP\) -> 0x10010158 = 1;
 *               bit 8 low (BSCLIP\) -> clip flag byte only (console). Both variables are cleared first.
 *   0x1001B638  (object 0x1001063C, inputs 0x10009490, 0x1001015C, enable 0x10010160 = 1): per block
 *               g *= 10^(-0.0001/20) when hot, 10^(+0.0001/20) otherwise (exp(+-P4 ln10/20), 0x1001DA40 = exp),
 *               g = max(g, 0.22387), g = min(g, 1); targets of the final gain 0x10011C74 (5 outputs) =
 *               0x10009490 x g. That is 0.05 dB/s, down to -13 dB (260 s); the service manual says 6 dB max.
 *               0x10009490 comes from 0x1001BB6C (VRAW ceiling, 0.89145 = -1 dB above 10.5 V, protect_supply.c).
 *   0x10010158 (CCLIP / RT6307) feeds 0x1001B820 (supply / clip fault, protect_supply.c). BSCLIP has no
 *               stock action.
 * 0x10015700 (AXR0[9] high after 'U' = USB inactive -> USB mute) is in volume.c. The AXR0[14] output (mode
 * selector, standby and fault latch, 0x100157E8 / 0x10015910) is in spk_mode.c. */
#include "oc.h"
#include "kern.h"
#include "inputs.h"
#include "biquad.h"
#include "regs_mcasp.h"
#include "rate.h"
#include "mathx.h"

#define M0(off)       MCASP_REG(MCASP0_BASE, off)
#define THERM_DOWN    0.99998849f    /* 10^(-0.0001/20) */
#define THERM_FLOOR   0.22387f
#define FIN_P48       0.997385f      /* final gain 0x10011C74 smoothing pole at 48 kHz */
#define SEL_DELAY     4              /* blocks (selector word +60) */

gpio_in_t g_gpio;

static float fin_cur, fin_prev_t, fin_p, fin_b, th_down, th_up;
static int sel_count;

void gpio_in_rate(void)
{
    fin_p = FIN_P48;
    fin_b = (1.0f - fin_p) * 0.5f;
    th_down = THERM_DOWN;     /* same dB per second */
    th_up = 1.0f / th_down;
}

void gpio_in_init(void)
{
    gpio_in_rate();
    g_gpio.therm_gain = 1.0f;          /* 0x1001B638 word 10 init */
    fin_cur = fin_prev_t = 0.89145f;    /* stock init copies the final gain targets (0x10011C74) */
    sel_count = 0;
}

void gpio_in_frame(void)
{
    uint32_t pd = M0(MCASP_PDIN);
    int hot;
    g_gpio.pdin = pd;
    g_gpio.t1 = !(pd & (1u << 4));
    g_gpio.t2 = !(pd & (1u << 5));
    g_gpio.cclip = !(pd & (1u << 7));
    g_gpio.bsclip = !(pd & (1u << 8));
    g_gpio.baddata = (pd >> 15) & 1u;
    hot = g_gpio.t1 || g_gpio.t2;      /* 0x1001015C */
    if (hot) g_gpio.n_hot++;
    if (g_gpio.cclip) g_gpio.n_cclip++;
    if (g_gpio.bsclip) g_gpio.n_bsclip++;
    if (g_gpio.baddata) g_gpio.n_baddata++;

    /* 0x1001B638 */
    g_gpio.therm_gain *= hot ? th_down : th_up;
    if (!(g_gpio.therm_gain > THERM_FLOOR)) g_gpio.therm_gain = THERM_FLOOR;
    if (g_gpio.therm_gain > 1.0f) g_gpio.therm_gain = 1.0f;

    /* selector 0x10010758: the requested branch is applied after SEL_DELAY blocks */
    if (g_gpio.baddata != g_gpio.sel) {
        if (++sel_count >= SEL_DELAY) {
            g_gpio.sel = g_gpio.baddata;
            sel_count = 0;
        }
    } else {
        sel_count = 0;
    }
}

/* Branch A is the identity here (stock: all-pass 0x1000971C, phase only); branch B = stock low-pass. */
DSP_SCRATCH(fin_g)
static float fin_g[FRAME_LEN];

void gpio_in_out_gain(float (*out)[96], int nout, int n, float ceiling)
{
    float *const g = fin_g;
    int k;
    k_smooth(g, &fin_cur, &fin_prev_t, g_gpio.therm_gain * ceiling, fin_b, fin_p, n);   /* 0x10009490 x thermal */
    for (k = 0; k < nout; k++) k_scalev(out[k], out[k], g, n);
}
