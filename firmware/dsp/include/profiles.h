/* Profiles stored in the DSP EEPROM (U7001): 4 slots, each kept as two copies (A/B) so a power loss
 * during a save never destroys the previous version.
 *
 * EEPROM map, layout v2 (boot/docs/LAYOUT.md): the profile area is 0xC200..0xDAFF, between the firmware
 * image body (0x0200..0xC1FF) and boot1 (0xDB00..0xFF7F, must never be written by the firmware):
 *   0xC200 + (2*slot + copy) * 0x300   copy of a slot (0x300 bytes = 6 pages), 0xC200..0xD9FF
 *       +0   magic "OCP1"   +4 seq (LE32, higher = newer)   +8 length (LE16) = PROFILE_SIZE
 *       +10  PROFILE_VERSION   +11 reserved   +12 CRC-32 of the profile bytes (LE32)
 *       +16  profile_t (616 bytes, see params.h)
 *   0xDA00 / 0xDA80                    config copies A/B (16 bytes): +0 "OCC1"  +4 seq (LE32)
 *       +8 boot slot (0xFF = defaults)  +9 sub level source (0 knob = default, 1 app; sub_level.h; was the
 *       bass compensation source, same values; 0 in records written before it existed)  +10 sub level of the "app" source (since 0.2.3, profile protocol 8): 128 + dB x 10
 *       (8..248 = -12.0..+12.0 dB); 0 or 1 = a record written before (it held "routage 2.1"): 0 dB until set  +11 device options (OPT_* below; 0 = stock, as in older records)  +12 CRC-32 of bytes
 *       0..11 (LE32)
 *   layout v3: the last page (+0x280..+0x2FF) of each of the 8 copies is a graph data page
 *       (include/bose_tables.h): never part of a record, never written by the firmware
 *
 * A save writes the copy that does not hold the newest valid version: profile pages first, header page
 * last, then reads the whole copy back. A copy is only written if it is blank (all 0xFF) or already starts
 * with an OpenCompanion magic: foreign data is never overwritten (on an EEPROM that still holds the stock
 * image, which ends at 0xDAD8, every copy reads as foreign and saving is refused).
 * The EEPROM driver refuses any write outside [PROF_AREA_LO, PROF_AREA_HI) (include/eeprom.h). */
#ifndef PROFILES_H
#define PROFILES_H

#include <stdint.h>
#include "params.h"

#define PROFILE_SLOTS   4

/* Layout v2 addresses (boot/docs/LAYOUT.md, boot1/include/layout.h). */
#define PROF_AREA_LO    0xC200u             /* first slot copy */
#define PROF_COPY_SIZE  0x300u
#define PROF_CFG_BASE   0xDA00u             /* config copy A; copy B at +PROF_CFG_STRIDE */
#define PROF_CFG_STRIDE 0x80u
#define PROF_AREA_HI    0xDB00u             /* end of the profile area = start of boot1 (exclusive) */

/* Marker read by the host tooling (oc-bootimg / oc-ramload refuse a firmware ELF without it): this
 * firmware stores its profiles in the layout v2 area, never over boot1. Value = layout version. */
extern const uint32_t oc_eeprom_layout_v2, oc_eeprom_layout_v3;
#define SLOT_NONE       0xFF

/* Device options, OCC1 config record byte 11 (console OPC / OPU). 0 = every stock default.
 *   bits 0..2  standby delay before the silence standby request: 0 stock 2 min, 1 off (never), 2 5 min,
 *              3 10 min, 4 20 min, 5 30 min, 6 60 min, 7 = 2 min (reserved)
 *   bit 3      the pod double press (mute then unmute within 600 ms) does NOT cycle the profiles
 *   bit 4      headphone EQ: the active profile's global EQ (target 0) also plays on the headphone feed; clear
 *              = flat feed (stock). Stored and reported now; applied once the audio engine has the headphone
 *              feed (README "Headphones")
 *   bits 5..7  signed n = -4..+3: silence threshold ("AUX wake threshold" in the app) = stock x 4^n power, 6.02 dB per step;
 *              0 = stock -71.1 dBFS (mean L/R block power after the master volume), -4 = -95.2, +3 = -53.0
 *              dBFS. Sound above it counts as activity (resets the timer, cancels the request). */
#define OPT_DELAY       0x07
#define OPT_NO_DTAP     0x08
#define OPT_HP_EQ       0x10

/* Result of the last EEPROM job (OPI) */
enum { PR_OK, PR_BUSY, PR_ERR_IO, PR_ERR_VERIFY, PR_ERR_CRC, PR_ERR_PROTECTED, PR_ERR_EMPTY, PR_ERR_ARG };

typedef struct {
    uint8_t valid;                  /* a copy passed the CRC check */
    uint8_t writable;               /* both copies are blank or ours */
    uint8_t color;
    uint32_t seq;
    char name[PROFILE_NAME_LEN];
} slot_info_t;

void profiles_init(void);           /* boot: scans the EEPROM and applies the boot slot (blocking) */
void profiles_poll(void);           /* main loop: EEPROM jobs + pod mute double press */

int  profiles_save(int slot);       /* queue a job; returns PR_OK if started */
int  profiles_load(int slot);
int  profiles_set_boot(int slot);   /* slot or SLOT_NONE */
/* Sub level source (device setting, config record): applied at once unless PR_BUSY / PR_ERR_ARG is returned;
 * PR_ERR_PROTECTED = applied for this session but not saved (config copies not ours). */
/* Device setting i (DEV_*, config record byte 9 + i) := v, same rules as the sub level source above. */
enum { DEV_SUB_SOURCE, DEV_SUB_LEVEL, DEV_OPTIONS };
#define SUB_LEVEL_CODE(db10)  (128 + (db10))        /* config byte 10 <-> dB x 10, -120..+120 */
#define SUB_LEVEL_SET(code)   ((code) >= SUB_LEVEL_CODE(-120) && (code) <= SUB_LEVEL_CODE(120))
int  profiles_set_dev(int i, int v);
#define profiles_set_sub_source(s)  profiles_set_dev(DEV_SUB_SOURCE, (s))      /* SUB_SRC_* */
/* Sub level of the "app" source, dB x 10 (-120..+120): applied at once (smoothed), saved in the config record
 * 1 s after the last change (debounced: a slider can send many). */
#define profiles_set_sub_level(db10) profiles_set_dev(DEV_SUB_LEVEL, SUB_LEVEL_CODE(db10))
#define profiles_set_options(o)     profiles_set_dev(DEV_OPTIONS, (o) & 0xFF)   /* OPT_* byte */
void profiles_modified(void);       /* the active profile was changed (OPW / OPD) */
void profiles_cycle(void);          /* load the next valid slot (pod mute double press) */

int  profiles_busy(void);
int  profiles_last_result(void);
int  profiles_active(void);         /* slot the active profile comes from, SLOT_NONE if none */
int  profiles_is_modified(void);
int  profiles_boot_slot(void);
const slot_info_t *profiles_slot(int slot);

#endif
