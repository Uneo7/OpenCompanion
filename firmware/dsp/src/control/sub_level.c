#include "oc.h"
#include "sub_level.h"
#include "params.h"
#include "eq.h"
#include "ramp.h"
#include "rate.h"
#include "mathx.h"
#include "kern.h"

#define KNOB_FULL   1.6666667f       /* knob scale end (ramp.c, stock 0x10010104): +-5/3 */
#define DEAD_ZONE   0.05f            /* of the travel on each side of the centre detent: 0 dB */
#define DB_MIN      (-12.0f)
#define DB_MAX      6.0f
#define POLE48      0.9995f          /* per-sample smoothing at 48 kHz, ~42 ms */

static int src = SUB_SRC_KNOB;
static float app_db;                 /* device setting (config record byte 10), not per profile */
static float level_db, g_cur, g_prev, sm_p, sm_b;
DSP_SCRATCH(g_ramp_buf)
static float g_ramp_buf[FRAME_LEN];

void sub_level_set_source(int s) { src = s == SUB_SRC_APP ? SUB_SRC_APP : SUB_SRC_KNOB; }
void sub_level_set_app_db(float db) { app_db = db; }
float sub_level_app_db(void) { return app_db; }
int sub_level_source(void) { return src; }
float sub_level_db(void) { return level_db; }

/* Knob position u in -1..+1 (ramp.c already applies the stock hysteresis), dead zone, then linear in dB:
 * 0 dB at the centre, DB_MIN fully left (negative values), DB_MAX fully right. */
float sub_level_knob_db(void)
{
    float u = g_ramp.bass * (1.0f / KNOB_FULL), a = u < 0.0f ? -u : u;
    a = a <= DEAD_ZONE ? 0.0f : (a > 1.0f ? 1.0f : (a - DEAD_ZONE) * (1.0f / (1.0f - DEAD_ZONE)));
    return u < 0.0f ? a * DB_MIN : a * DB_MAX;
}

void sub_level_rate(void)
{
    sm_p = POLE48;
    sm_b = (1.0f - sm_p) * 0.5f;
}

void sub_level_init(void)
{
    sub_level_rate();
    level_db = 0.0f;
    g_cur = g_prev = 1.0f;
}

void sub_level_apply(float *bass, int n)
{
    level_db = src == SUB_SRC_APP ? app_db : sub_level_knob_db();
    k_smooth(g_ramp_buf, &g_cur, &g_prev, mx_db_to_lin(level_db) * g_eq_gains.trim[EQ_SUB - 1], sm_b, sm_p, n);
    k_scalev(bass, bass, g_ramp_buf, n);
}
