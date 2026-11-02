/* Port of the stock piecewise volume curve module (function 0x1001B140), used by the stock firmware for the
 * USB volume (object 0x100106FC) and the pod volume (object 0x10010564). Parameters are the values held by
 * these objects in the stock RAM image (re/dsp/out/rt.bin):
 *   bp[0..n+1]  breakpoints, bp[0] = top (0 dB), bp[n+1] = bottom clamp
 *   slope[0..n] dB per percent; the slopes are CUMULATIVE: below bp[1] both slope[0] and slope[1] apply...
 *   floor       minimum dB
 * Algorithm, as in the stock code:
 *   x = clamp(x, bp[n+1], bp[0]); d = bp[0] - x; db = 0
 *   for i = 0..n: db -= slope[i] * d; d -= bp[i] - bp[i+1]; stop when d < 1.0   (stock quirk: not 0)
 *   db = max(db, floor)
 * Not ported: the second input of the module (when non-zero, subtracts word 19 = 12 dB; its source is not
 * identified) and the mute input (OpenCompanion mutes separately). */
#include "volume_curve.h"

const volume_curve_t VC_STOCK_POD = {
    2, { 99.0f, 35.0f, 4.0f, 0.0f, 0.0f }, { 0.3f, 1.1f, 6.0f, 0.0f }, -71.3f
};

float volume_curve_db(const volume_curve_t *c, float x)
{
    float d, db = 0.0f;
    int i;
    if (x > c->bp[0]) x = c->bp[0];
    if (x < c->bp[c->n + 1]) x = c->bp[c->n + 1];
    d = c->bp[0] - x;
    for (i = 0; i <= c->n; i++) {
        db -= c->slope[i] * d;
        d -= c->bp[i] - c->bp[i + 1];
        if (d < 1.0f) break;
    }
    return db < c->floor_db ? c->floor_db : db;
}

float volume_pct_from_uac(int16_t v)
{
    return ((float)v + 32768.0f) / 327.68f;         /* stock handler of 'V' (0x10010100) */
}

/* Stock pod knob filter 0x100162B4 / 0x10016420, once per ramp frame (2 ms): one-pole 0.05 / 0.95 on the IVOL
 * ratio (in double, as stock), x 99 %, then: <= 0.25 % -> 0, 0.25..0.75 % -> hold the previous output,
 * 0.75..1 % -> 1 %, above: the value. Returns the pod volume (stock 0x100100FC); *smooth is the filter state. */
float pod_filter(float ratio, float *smooth, float prev)
{
    float pct, out;
    double x;
    pct = (float)(((double)ratio * 0.05 + (double)*smooth * 0.95) * 99.0);
    *smooth = pct / 99.0f;
    x = pct;
    out = pct;
    if (!(x > 0.25)) { x = 0.0; out = 0.0f; }
    if (x > 0.25 && !(x > 0.75)) out = prev;                    /* dead band: hold */
    x = out;
    if (x > 0.75 && !(x > 1.0)) out = 1.0f;
    return out;
}

/* Reported pod knob (whole %), direction-aware hysteresis on the filtered knob p (pod_filter): it moves by
 * whole steps while the knob keeps turning the same way (p at least KNOB_STEP above / below it), but turns back
 * only after p has moved KNOB_REVERSE the other way. Noise around a still knob (measured on hardware: +-2 % on
 * the filtered value) can no longer walk it up and down; a real reversal shows after a 4 % turn.
 * Moving up it takes floor(p), down ceil(p): the reported value never overshoots the knob. *dir: +1 / -1, the
 * last move (0 at start). */
#define KNOB_STEP    1.0f
#define KNOB_REVERSE 4.0f
int knob_step(float p, int knob, int *dir)
{
    const float up = *dir < 0 ? KNOB_REVERSE : KNOB_STEP, down = *dir > 0 ? KNOB_REVERSE : KNOB_STEP;
    if (p >= (float)knob + up) {
        *dir = 1;
        return (int)p;
    }
    if (p <= (float)knob - down) {
        *dir = -1;
        return (int)p + ((float)(int)p < p);
    }
    return knob;
}

/* Plausibility of the IVOL reading (pod knob), once per ramp frame (2 ms). A real potentiometer changes by a
 * few counts per frame at most (full travel = cal, 80..120 counts; a fast turn over 0.1 s is ~2 counts per
 * frame; a pod at rest dithers by +-1..3 counts), and never reads above full scale. A floating input (pod
 * unplugged) jumps by tens of counts or reads past full scale. Each implausible frame adds IVOL_BAD to a score
 * (capped at IVOL_CAP), each plausible one removes 1: the knob is valid only while the score is 0, so it turns
 * invalid within a few frames and valid again 0.5 s after the reading has settled. Returns 1 if plausible. */
#define IVOL_JUMP   12u          /* counts per frame */
#define IVOL_OVER   6u           /* counts above the calibrated full scale */
#define IVOL_BAD    16
#define IVOL_CAP    256
int ivol_plausible(uint32_t count, float cal, uint32_t *last, int *score)
{
    const uint32_t d = count > *last ? count - *last : *last - count;
    const int bad = d > IVOL_JUMP || (float)count > cal + (float)IVOL_OVER;
    *last = count;
    if (bad) *score = *score + IVOL_BAD > IVOL_CAP ? IVOL_CAP : *score + IVOL_BAD;
    else if (*score > 0) (*score)--;
    return *score == 0;
}
