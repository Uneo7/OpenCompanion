/* Binary console commands for the sound parameters and the stored profiles (see README "Parameters").
 * All arguments are little-endian, no '\r'. Replies: 0x06 (ACK) + data, or 0x15 (NAK) + error code.
 *   OPI                          -> 06 info (15 bytes, below)
 *   OPR <off:LE16> <n:u8 1..56>  -> 06 + n bytes of the active profile
 *   OPW <off:LE16> <data 1..55>  -> 06            (applied immediately, cross-faded)
 *   OPD                          -> 06            (active profile = flat defaults)
 *   OPX                          -> 06            (commit: a batch of OPW that changed the structure is complete;
 *                                                  installed behind the output ramp at once, proto >= 9)
 *   OPL <slot>                   -> 06            (load slot 0..3, asynchronous: poll OPI)
 *   OPV <slot>                   -> 06            (save the active profile to slot 0..3, asynchronous)
 *   OPB <slot|FF>                -> 06            (boot slot, FF = defaults, asynchronous)
 *   OPN <slot>                   -> 06 valid color seq:LE32 name[16]
 *   OPK                          -> 06            (active profile = built-in "Bose d'origine", read-only
 *                                                  constant; save it with OPV to keep a copy in a slot)
 *   OPQ                          -> 06 source(1) app:LE16 knob:LE16 value:LE16   (sub level, proto >= 5)
 *                                   source 0 knob / 1 app; app = the app sub level (proto >= 8: a device
 *                                   setting, config record byte 10; before: the profile's sub_level_cdb);
 *                                   knob = the rear knob as a level; value = the level in use; all signed
 *                                   1/100 dB (knob -1200..+600, centre detent 0)
 *   OPS <0|1>                    -> 06            (sub level source, saved in the config record, asynchronous;
 *                                   the app sub level is set with the text command DEVL<dB x10>, proto >= 8;
 *                                                  15 05 = applied for this session but not saved)
 *   OPC [<options>]              -> 06 0(1) in_effect(1) layout(1) options(1)   (device options byte, profiles.h
 *                                   OPT_*: standby delay, pod double press, headphone EQ, silence threshold;
 *                                   proto >= 4; with an argument: set, applied at once and saved in the config
 *                                   record, asynchronous; 15 01 busy: nothing changed, 15 05 applied but not
 *                                   saved, 15 07 bad arg). in_effect = 2.1 routing now, layout = 1 5.1 / 2 2.1.
 * OPI data: proto(1) profile_size:LE16 slots(1) active(1) flags(1) boot(1) valid_mask(1) writable_mask(1)
 *           last_result(1) sections:LE16 cpu_pct(1, measured: CPU) eq_flags(1)
 *   flags: bit0 modified since load/save, bit1 EEPROM job running, bit2 last job failed, bit3 stock graph
 *          tables missing (include/bose_tables.h: "Bose d'origine" falls back to the neutral preset) */
#include <string.h>
#include "oc.h"
#include "params.h"
#include "engine.h"
#include "profiles.h"
#include "eq.h"
#include "profile_bose.h"
#include "bose_tables.h"
#include "rate.h"
#include "ramp.h"
#include "sub_level.h"
#include "console_util.h"
#include "inputs.h"

#define ACK 0x06
#define NAK 0x15
#define OP_PROTO       9            /* 2: OPQ / OPS and the profile field bass_milli; 3: OPU, OPI flags bit3;
                                     * 4: OPC (device options), band enabled bit (params.h BAND_DISABLED);
                                     * 5: unified engine: OPQ / OPS = sub level source (bass compensation and
                                     *    bass_milli removed), crossover_hz reserved (stock crossover only);
                                     * 6: raw biquad pairs (types 7, 8) removed: turned off; PF_ADVANCED;
                                     * 8: app sub level = device setting (config byte 10, DEVL);
                                     * 9: OPX commits a structural batch of OPW (behind one output ramp) */
#define OPR_MAX        56
#define OPW_MAX        55

static int nak(uint8_t *out, int code)
{
    out[0] = NAK;
    out[1] = (uint8_t)code;
    return 2;
}

static int ack_result(uint8_t *out, int r)
{
    if (r != PR_OK) return nak(out, r);
    out[0] = ACK;
    return 1;
}

static int cmd_info(uint8_t *out)
{
    int s, last = profiles_last_result();
    uint8_t valid = 0, writable = 0;
    uint32_t sections = eq_active_sections();
    for (s = 0; s < PROFILE_SLOTS; s++) {
        if (profiles_slot(s)->valid) valid |= (uint8_t)(1u << s);
        if (profiles_slot(s)->writable) writable |= (uint8_t)(1u << s);
    }
    out[0] = ACK;
    out[1] = OP_PROTO;
    out[2] = (uint8_t)PROFILE_SIZE;
    out[3] = (uint8_t)(PROFILE_SIZE >> 8);
    out[4] = PROFILE_SLOTS;
    out[5] = (uint8_t)profiles_active();
    out[6] = (uint8_t)(profiles_is_modified() | (profiles_busy() << 1) | ((last != PR_OK && last != PR_BUSY) << 2) |
                       (!bose_tables_ok() << 3));      /* stock graph tables missing: "Bose d'origine" falls back */
    out[7] = (uint8_t)profiles_boot_slot();
    out[8] = valid;
    out[9] = writable;
    out[10] = (uint8_t)last;
    out[11] = (uint8_t)sections;
    out[12] = (uint8_t)(sections >> 8);
    out[13] = (uint8_t)(g_cpu.period ? (int)((float)g_cpu.avg * 100.0f / (float)g_cpu.period) : 0);   /* measured */
    out[14] = g_profile.flags;
    return 15;
}

static int cmd_name(const uint8_t *a, int len, uint8_t *out)
{
    const slot_info_t *si;
    if (len != 1 || a[0] >= PROFILE_SLOTS) return nak(out, PR_ERR_ARG);
    si = profiles_slot(a[0]);
    out[0] = ACK;
    out[1] = si->valid;
    out[2] = si->color;
    out[3] = (uint8_t)si->seq;
    out[4] = (uint8_t)(si->seq >> 8);
    out[5] = (uint8_t)(si->seq >> 16);
    out[6] = (uint8_t)(si->seq >> 24);
    memcpy(out + 7, si->name, PROFILE_NAME_LEN);
    return 7 + PROFILE_NAME_LEN;
}

static void put16(uint8_t *p, long v)
{
    p[0] = (uint8_t)v;
    p[1] = (uint8_t)(v >> 8);
}

static int cmd_sub(uint8_t *out)
{
    out[0] = ACK;
    out[1] = (uint8_t)sub_level_source();
    put16(out + 2, cu_fx(sub_level_app_db(), 100.0f));      /* device setting since proto 8 */
    put16(out + 4, cu_fx(sub_level_knob_db(), 100.0f));
    put16(out + 6, cu_fx(sub_level_db(), 100.0f));
    return 8;
}

int console_params(const uint8_t *msg, int len, uint8_t *out, int max)
{
    const uint8_t *a = msg + 3;
    int n = len - 3;
    uint32_t off;
    if (len < 3 || msg[0] != 'O' || msg[1] != 'P' || max < 1 + OPR_MAX) return -1;
    switch (msg[2]) {
    case 'I':
        return cmd_info(out);
    case 'R':
        if (n != 3 || a[2] < 1 || a[2] > OPR_MAX) return nak(out, PR_ERR_ARG);
        off = (uint32_t)a[0] | ((uint32_t)a[1] << 8);
        if (params_read(off, out + 1, a[2]) < 0) return nak(out, PR_ERR_ARG);
        out[0] = ACK;
        return 1 + a[2];
    case 'W':
        if (n < 3 || n - 2 > OPW_MAX) return nak(out, PR_ERR_ARG);
        off = (uint32_t)a[0] | ((uint32_t)a[1] << 8);
        if (params_write(off, a + 2, (uint32_t)(n - 2)) < 0) return nak(out, PR_ERR_ARG);
        profiles_modified();
        out[0] = ACK;
        return 1;
    case 'D':                                          /* flat defaults */
    case 'K': {                                        /* built-in "Bose d'origine" */
        static profile_t p;
        if (msg[2] == 'K') {
            if (n != 0) return nak(out, PR_ERR_ARG);
            profile_bose_stock(&p);
        } else {
            params_defaults(&p);
        }
        params_set_active(&p);
        profiles_modified();
        out[0] = ACK;
        return 1;
    }
    case 'L':
        if (n != 1) return nak(out, PR_ERR_ARG);
        return ack_result(out, profiles_load(a[0]));
    case 'X':                                          /* commit a structural batch of OPW (proto 9) */
        engine_commit();
        out[0] = ACK;
        return 1;
    case 'C': {                                        /* device options */
        int r = PR_OK;
        if (n > 1) return nak(out, PR_ERR_ARG);
        if (n) r = profiles_set_options(a[0]);
        if (r != PR_OK) return nak(out, r);            /* 15 05: applied for this session, not saved */
        out[0] = ACK;
        out[1] = 0;                                    /* was the routage 2.1 setting (OPU, removed) */
        out[2] = (uint8_t)audio_route21_active();
        out[3] = g_layout;
        out[4] = (uint8_t)g_dev_opts;
        return 5;
    }
    case 'V':
        if (n != 1) return nak(out, PR_ERR_ARG);
        return ack_result(out, profiles_save(a[0]));
    case 'B':
        if (n != 1) return nak(out, PR_ERR_ARG);
        return ack_result(out, profiles_set_boot(a[0]));
    case 'N':
        return cmd_name(a, n, out);
    case 'Q':
        if (n != 0) return nak(out, PR_ERR_ARG);
        return cmd_sub(out);
    case 'S':
        if (n != 1) return nak(out, PR_ERR_ARG);
        return ack_result(out, profiles_set_sub_source(a[0]));
    default:
        return -1;
    }
}
