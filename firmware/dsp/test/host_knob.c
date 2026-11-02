/* Host check of the pod knob path (LOCAL mode, and the filtered knob the 'S' reply carries): IVOL ratio -> stock knob filter (volume_curve.c pod_filter, once per
 * 2 ms frame) -> stock pod curve 0x10010564 -> per-sample gain smoother of the loudness gains (k_smooth, pole
 * 0.998692 at 48 kHz, loudness.c).
 *   host_knob            prints: (1) gain jitter (peak-to-peak dB over 4 s) for a still knob read with noise
 *                        (IVOL ratio 0.5 +- 1 % uniform, every frame; the stock converter's count noise is
 *                        +-1 count of ~100); (2) the largest per-sample gain step (dB) and per-frame pod step
 *                        (%) while the knob turns from 0 to full in 0.5 s (zipper).
 * Measured: jitter 0.11 dB (the stock filter itself: the noise passes its 0.05 / 0.95 one-pole,
 * 0.3 dB per % on this part of the pod curve), per-sample step 0.013 dB. Bounds: jitter < 0.2 dB, step < 0.05 dB. */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include "oc.h"
#include "kern.h"
#include "volume_curve.h"

static float db(float g) { return 20.0f * log10f(g > 1e-9f ? g : 1e-9f); }

int main(void)
{
    const float p = 0.998692f, b = (1.0f - p) * 0.5f;
    float g[FRAME_LEN], cur = 0.0f, prev = 0.0f, smooth = 0.0f, pct = 0.0f, lo = 1e9f, hi = -1e9f;
    float step = 0.0f, pstep = 0.0f, last = 0.0f;
    int f, i;
    srand(1);
    for (f = 0; f < 3000; f++) {                       /* still knob at half travel, noisy reading */
        const float r = 0.5f + 0.01f * (2.0f * (float)rand() / RAND_MAX - 1.0f);
        pct = pod_filter(r, &smooth, pct);
        k_smooth(g, &cur, &prev, powf(10.0f, volume_curve_db(&VC_STOCK_POD, pct) / 20.0f), b, p, FRAME_LEN);
        if (f >= 1000)
            for (i = 0; i < FRAME_LEN; i++) {
                if (db(g[i]) < lo) lo = db(g[i]);
                if (db(g[i]) > hi) hi = db(g[i]);
            }
    }
    printf("still knob, noisy IVOL: gain jitter %.4f dB peak-to-peak (pod %.2f %%)\n", hi - lo, pct);
    smooth = 0.0f; pct = 0.0f; cur = prev = powf(10.0f, -71.3f / 20.0f); last = db(cur);
    for (f = 0; f < 500; f++) {                        /* 0 -> full travel in 250 frames, then still */
        const float r = f < 250 ? (float)f / 250.0f : 1.0f, p0 = pct;
        pct = pod_filter(r, &smooth, pct);
        if (fabsf(pct - p0) > pstep) pstep = fabsf(pct - p0);
        k_smooth(g, &cur, &prev, powf(10.0f, volume_curve_db(&VC_STOCK_POD, pct) / 20.0f), b, p, FRAME_LEN);
        for (i = 0; i < FRAME_LEN; i++) {
            if (f > 20 && fabsf(db(g[i]) - last) > step) step = fabsf(db(g[i]) - last);
            last = db(g[i]);
        }
    }
    printf("knob sweep 0..99 %% in 0.5 s: largest per-sample gain step %.4f dB, largest pod step %.2f %% per frame\n",
           step, pstep);
    return 0;
}
