/* Peak hold since the last console read (LVI / LVO reset it), linear; the console converts it to dBFS. */
#include "oc.h"
#include "mathx.h"
#include "kern.h"

static float peak_hold[METER_COUNT];

void meters_init(void)
{
    int i;
    for (i = 0; i < METER_COUNT; i++) peak_hold[i] = 0.0f;
}

/* Peak of x s (s: the scale of the block relative to full scale). */
void meters_update(int idx, const float *x, int n, float s)
{
    float sq;
    meters_set(idx, k_peaksq(x, n, &sq) * s);
}

void meters_set(int idx, float pk)
{
    if (pk > peak_hold[idx]) peak_hold[idx] = pk;
}

float meters_peak(int idx, int reset)
{
    float v = peak_hold[idx];
    if (reset) peak_hold[idx] = 0.0f;
    return v;
}

int meters_to_db(float lin)
{
    float db;
    if (lin < 1.0e-5f) return -99;
    db = mx_lin_to_db(lin);
    if (db > 0.0f) return 0;
    return (int)(db - 0.5f);    /* round towards -inf for negative values */
}
