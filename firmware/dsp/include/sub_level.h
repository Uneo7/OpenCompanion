/* Subwoofer level and its source (device setting "sub level source", OCC1 config record byte 9, which was the
 * bass compensation source: same byte, same values, so existing records keep their meaning of "knob" / "app").
 *   SUB_SRC_KNOB (0, default, as the stock speaker): the rear bass knob (ramp.c, g_ramp.bass) sets the sub
 *                level: centre detent 0 dB, -12 dB fully left, +6 dB fully right, dead zone around the centre
 *                (sub_level.c); the profile's sub_level_cdb is not used.
 *   SUB_SRC_APP  (1): a device setting (since 0.2.3: OCC1 config record byte 10, DEVL / OPS), not a profile
 *                setting: it stays when profiles are switched and with the read-only Bose profile; the
 *                profile's sub_level_cdb is ignored (reserved). The knob is still read and reported.
 * The stock bass compensation (all-pass + polynomial(knob) x 4th-order low-pass, 0x1001C5EC / mixer
 * 0x10010938) is not run in any preset: bass tone goes through the EQ (sub target, global EQ). Every change
 * (source, knob, profile value, sub trim) is smoothed per sample (~40 ms): no click. */
#ifndef SUB_LEVEL_H
#define SUB_LEVEL_H

enum { SUB_SRC_KNOB, SUB_SRC_APP };

void  sub_level_set_source(int src);  /* RAM only (persisted by profiles_set_sub_source()) */
int   sub_level_source(void);
void  sub_level_set_app_db(float db);   /* "app" level, dB (RAM; persisted by profiles_set_sub_level()) */
float sub_level_app_db(void);
float sub_level_knob_db(void);        /* knob position as a level, dB (-12..+6), whatever the source */
float sub_level_db(void);             /* level in use, dB (before the sub trim) */
void  sub_level_rate(void);
void  sub_level_init(void);
/* Once per frame: bass output x (sub level x sub trim), smoothed, in place. */
void  sub_level_apply(float *bass, int n);

#endif
