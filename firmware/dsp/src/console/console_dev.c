/* DEV: device settings of the OCC1 config record (include/profiles.h). Ends with '\r'; replies end with
 * "\r>", errors are "\r!".
 *   DEV          SR:<sub level source 0 knob / 1 app> SL:<app sub level, dB x10> KN:<rear knob as a sub level, dB x10>
 *                AC:<2.1 routing in effect 0/1> LY:<input layout 1 5.1, 2 2.1 mode> OP:<options 0..255>
 *   DEV<k><v>    sets one setting, applied at once and saved in the config record: k = B sub level source (0/1),
 *                L app sub level (dB x10, -120..120, saved 1 s after the last change), O options (decimal
 *                0..255, profiles.h OPT_*); reply = DEV + " R:<result>"
 *                (0 saving, poll OPI; 5 applied for this session but not saved: config area not ours).
 *                Busy (EEPROM job running) or bad argument: "\r!", nothing changed. */
#include <string.h>
#include "oc.h"
#include "rate.h"
#include "ramp.h"
#include "inputs.h"
#include "profiles.h"
#include "sub_level.h"
#include "console_util.h"

int console_dev(const uint8_t *msg, int len, uint8_t *o, int max)
{
    static const char LAB[] = "SR:\0 SL:\0 KN:\0 AC:\0 LY:\0 OP:\0 R:";
    static const char KEYS[] = "BLO";               /* DEV_SUB_SOURCE, DEV_SUB_LEVEL, DEV_OPTIONS */
    long v[7];
    int i, r = -1;
    if (len < 4 || memcmp(msg, "DEV", 3)) return -1;
    if (len > 4) {
        for (i = 0; i < DEV_OPTIONS && KEYS[i] != msg[3]; i++) ;
        if (KEYS[i] != msg[3] || !cu_parse_int(msg + 4, len - 5, v) ||
            (i == DEV_SUB_LEVEL ? v[0] < -120 || v[0] > 120 : (unsigned long)v[0] > 255u) ||
            (r = profiles_set_dev(i, i == DEV_SUB_LEVEL ? SUB_LEVEL_CODE((int)v[0]) : (int)v[0])) == PR_BUSY ||
            r == PR_ERR_ARG)
            return cu_str(o, 0, max, "\r!");
    }
    v[0] = sub_level_source();
    v[1] = cu_fx(sub_level_app_db(), 10.0f);
    v[2] = cu_fx(sub_level_knob_db(), 10.0f);
    v[3] = audio_route21_active();
    v[4] = g_layout;
    v[5] = g_dev_opts;
    v[6] = r;
    return cu_str(o, cu_fields(o, 0, max, LAB, v, r < 0 ? 6 : 7), max, "\r>");
}
