/* Read-only console commands for the hardware inputs (text, end with '\r', reply ends with "\r>").
 * Fixed-point integers: x10 = tenths, x1000 = thousandths.
 *   POD  PR:<pod present> KN:<knob % x10> MD:<H host / L local> RK:<reported knob %> EV:<events>
 *        DB:<master dB x10> LB:<loudness boost dB x10>
 *   PRT  FT:<fault> VR:<supply mV> T:<t1><t2> CC:<n> BC:<n> CG:<compressor dB x10> LG:<look-ahead limiter dB x10>
 *        supply / clip fault latched, VRAW, thermistors (1 = hot), frames with CCLIP (satellite amplifiers) /
 *        BSCLIP (bass amplifier), lowest compressor / limiter gain. Other fields: BRR of g_supply, g_gpio,
 *        g_standby (addresses in the map). */
#include <string.h>
#include "oc.h"
#include "inputs.h"
#include "ramp.h"
#include "volume.h"
#include "protect.h"
#include "rate.h"
#include "mathx.h"
#include "console_util.h"

static int fields(uint8_t *o, int n, int max, const char *lab, const long *v, int cnt)
{
    return cu_str(o, cu_fields(o, n, max, lab, v, cnt), max, ">");
}

int console_inputs(const uint8_t *msg, int len, uint8_t *o, int max)
{
    /* Labels separated by '\0' (cu_fields); POD is split around its text field " MD:H" / " MD:L". */
    static const char L_POD1[] = "PR:\0 KN:";
    static const char L_POD2[] = " RK:\0 EV:\0 DB:\0 LB:";
    static const char L_PRT[] = "FT:\0 VR:\0 T:\0\0 CC:\0 BC:\0 CG:\0 LG:";
    long v[8];
    (void)len;
    if (!memcmp(msg, "POD", 3)) {
        int n;
        v[0] = g_audio.pod_present;
        v[1] = cu_fx(g_ramp.pod_pct, 10.0f);
        n = cu_fields(o, 0, max, L_POD1, v, 2);
        n = cu_str(o, n, max, g_vol.host ? " MD:H" : " MD:L");
        v[0] = g_vol.knob;
        v[1] = g_vol.events;
        v[2] = cu_fx(g_vol.master_db, 10.0f);
        v[3] = cu_fx(g_loud.boost_db, 10.0f);
        return fields(o, n, max, L_POD2, v, 4);
    }
    if (!memcmp(msg, "PRT", 3)) {
        v[0] = g_supply.fault;
        v[1] = cu_fx(g_ramp.vraw, 1000.0f);
        v[2] = g_gpio.t1;
        v[3] = g_gpio.t2;
        v[4] = (long)g_gpio.n_cclip;
        v[5] = (long)g_gpio.n_bsclip;
        v[6] = cu_fx(mx_lin_to_db(protect_comp_gain()), 10.0f);
        v[7] = cu_fx(mx_lin_to_db(protect_lim_gain()), 10.0f);
        return fields(o, 0, max, L_PRT, v, 8);
    }
    return -1;
}
