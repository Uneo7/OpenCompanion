/* Stock-scaled channel powers, the input of the stock loudness detector (0x1001B400) and of the silence
 * timer (0x100101D0 -> 0x1001B500).
 * Stock graph: USB channels x 0.340408 (input margin, gain 0x100106C8, whose targets are the USB volume curve
 * output) -> router 0x100102D0 -> mixers 0x100104EC / 0x10010514: L = USB L + 1.86209 x AUX L (same for R)
 * -> DC blocker 0x1001032C -> meter 0x10010368 (mode 1: P2 x sum x^2, P2 = 1/96) -> 0x1000916C[6]
 * (L, R, C, LFE, Ls, Rs). The ~5 Hz DC blocker is not reproduced (no effect on the decisions above a few Hz).
 * Assumption: stock samples are normalised to +-1.0 full scale, like ours (not verified in the stock
 * de-interleave 0x10027314). Measured independently of the OpenCompanion gain staging (AUX gain, profiles), so
 * the thresholds keep their stock meaning. */
#include "oc.h"
#include "inputs.h"
#include "kern.h"
#include "bose_graph.h"

#define USB_MARGIN   0.340408f      /* gain 0x100106C8, init / unity-volume value */
#define AUX_MIX      1.86209f       /* mixers 0x100104EC / 0x10010514, input B */

DSP_SCRATCH(buf)
static float buf[FRAME_LEN];

void level_detect(float *const in[BG_IN_COUNT], float scale, float usb_gain, float pow6[LD_CHANNELS])
{
    const float gu = USB_MARGIN * usb_gain * scale;
    int c;
    for (c = 0; c < LD_CHANNELS; c++) {
        float acc;
        if (c < 2) {                                  /* L, R + 1.86209 AUX */
            k_scale(buf, in[c], gu, FRAME_LEN);
            k_mac(buf, in[BG_IN_AUX_L + c], AUX_MIX, FRAME_LEN);
            acc = k_sumsq(buf, FRAME_LEN);
        } else {
            acc = k_sumsq(in[c], FRAME_LEN) * gu * gu;
        }
        pow6[c] = acc * (1.0f / FRAME_LEN);
    }
}
