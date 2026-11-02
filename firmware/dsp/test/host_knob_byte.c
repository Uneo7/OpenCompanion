/* Host check of the pod knob byte of the 'S' reply (volume_curve.c pod_filter + knob_step).
 *   host_knob_byte
 * 1. Still knob at 78 %, noise models (60 s each, 2 ms frames):
 *      a. +-2 % uniform white noise on the filtered value (worst case of the hardware reading);
 *      b. +-2 % slow wander on the filtered value (0.7 Hz sine + 0.2 Hz sine);
 *      c. +-6 % uniform white noise on the raw IVOL ratio, through the stock filter (about +-2 % after it).
 *    Prints the number of byte changes after the first second and the range of values.
 * 2. A real turn: 50 -> 80 % in 300 ms, hold 0.5 s, back to 70 % in 100 ms: prints when the byte comes within
 *    2 % of each end (most of the lag is the stock filter itself, a 40 ms one-pole).
 * Bound kept by the firmware: noise a/b/c -> at most 2 changes and a range of 2 %; the byte within 2 % of the
 * knob 100 ms after the knob stops, after the turn and after the reversal. */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include "volume_curve.h"

static float uni(void) { return 2.0f * (float)rand() / (float)RAND_MAX - 1.0f; }

static void still(const char *name, int model)
{
    float smooth = 0.78f, pct = 78.0f;
    int knob = 78, dir = 0, f, changes = 0, lo = 99, hi = 0;
    for (f = 0; f < 30000; f++) {
        float p;
        const double t = f * 0.002;
        if (model == 2) {
            pct = pod_filter(0.78f + 0.06f * uni(), &smooth, pct);
            p = pct;
        } else {
            p = 78.0f + (model == 0 ? 2.0f * uni() : (float)(1.2 * sin(2 * 3.14159 * 0.7 * t) + 0.8 * sin(2 * 3.14159 * 0.2 * t)));
        }
        {
            const int k = knob_step(p, knob, &dir);
            if (f > 500) {
                changes += k != knob;
                if (k < lo) lo = k;
                if (k > hi) hi = k;
            }
            knob = k;
        }
    }
    printf("%s: %d changes, byte %d..%d\n", name, changes, lo, hi);
}

int main(void)
{
    float smooth = 0.5f, pct = 50.0f;
    int knob = 50, dir = 0, f, t79 = -1, t71 = -1;
    srand(1);
    still("a: +-2 % white on the filtered knob", 0);
    still("b: +-2 % slow wander", 1);
    still("c: +-6 % white on IVOL, stock filter", 2);
    for (f = 0; f < 1500; f++) {                       /* frames of 2 ms */
        const float r = f < 150 ? 0.5f + 0.3f * f / 150.0f : f < 400 ? 0.8f : f < 450 ? 0.8f - 0.1f * (f - 400) / 50.0f : 0.7f;
        pct = pod_filter(r, &smooth, pct);
        knob = knob_step(pct, knob, &dir);
        if (t79 < 0 && knob >= 78) t79 = f;
        if (f >= 400 && t71 < 0 && knob <= 72) t71 = f;
    }
    printf("turn 50 -> 80 %% (reached at 300 ms): byte >= 78 at %d ms; back to 70 %% (reached at 900 ms): byte <= 72 at %d ms\n",
           2 * t79, 2 * t71);
    return 0;
}
