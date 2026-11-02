/* Host check of the IVOL plausibility (volume_curve.c ivol_plausible: knob valid only with a real pod).
 *   host_ivol
 * cal = 100 counts. Prints the share of frames reported valid over 60 s for:
 *   a. a pod at rest at 38 % with +-3 counts of dither (more than seen on hardware): must stay valid;
 *   b. a pod turned back and forth over the full travel in 0.2 s, with +-2 counts of dither: must stay valid;
 *   c. a floating input: uniform random counts 0..160 every frame: must never be valid;
 *   d. a floating input that settles at a constant 38 counts: undetectable here (the TAS 'X' / pod mute line
 *      decide), reported for completeness;
 * and the time to turn invalid after a pod is unplugged (a -> c) and valid again after it is plugged (c -> a). */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include "volume_curve.h"

static unsigned rnd(unsigned n) { return (unsigned)rand() % n; }

static double run(int model, int frames)
{
    uint32_t last = 38;
    int score = 0, f, ok = 0;
    for (f = 0; f < frames; f++) {
        unsigned c;
        if (model == 0) c = 38 + rnd(7) - 3;
        else if (model == 1) {
            const double t = fmod(f * 0.002, 0.4), p = t < 0.2 ? t / 0.2 : (0.4 - t) / 0.2;
            const int v = (int)(p * 100.0) + (int)rnd(5) - 2;
            c = (unsigned)(v < 0 ? 0 : v > 100 ? 100 : v);
        } else if (model == 2) c = rnd(161);
        else c = 38;
        ok += ivol_plausible(c, 100.0f, &last, &score);
    }
    return 100.0 * ok / frames;
}

int main(void)
{
    uint32_t last = 38;
    int score = 0, f, t_off = -1, t_on = -1;
    srand(1);
    printf("a. pod at rest +-3 counts: valid %.1f %% of the frames\n", run(0, 30000));
    printf("b. full turns in 0.2 s: valid %.1f %%\n", run(1, 30000));
    printf("c. floating, random counts: valid %.1f %%\n", run(2, 30000));
    printf("d. floating, constant: valid %.1f %% (not detectable from IVOL)\n", run(3, 30000));
    for (f = 0; f < 3000; f++) {                       /* rest 2 s, unplug 2 s, plug back 2 s */
        const unsigned c = f < 1000 || f >= 2000 ? 38 + rnd(7) - 3 : rnd(161);
        const int ok = ivol_plausible(c, 100.0f, &last, &score);
        if (f >= 1000 && f < 2000 && t_off < 0 && !ok) t_off = f - 1000;
        if (f >= 2000 && t_on < 0 && ok) t_on = f - 2000;
    }
    printf("unplugged -> invalid after %d ms; plugged back -> valid after %d ms\n", 2 * t_off, 2 * t_on);
    return 0;
}
