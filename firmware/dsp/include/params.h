/* User sound parameters (one "profile") and their serialized layout.
 *
 * The struct is the wire and storage format: the app reads/writes it by byte offset (OPR/OPW console
 * commands) and the EEPROM stores it as is. Little-endian, no padding (checked below). Layout:
 *
 *   off  size  field
 *     0     1  version          PROFILE_VERSION
 *     1     1  flags            bit0 bypass user EQ (global + per output + loudness)
 *                               bit1 link L/R (right outputs use the left outputs' bands)
 *                               bit2 loudness on: the stock dynamic loudness (0x1001AF08, loudness.c), up
 *                                    to +19 dB around 48 Hz at low level (skipped by bit0)
 *                               bit3 stock volume law: the USB volume follows the Bose percentage curve
 *                                    (stock 0x1001B140) instead of the UAC dB value (for the Bose profile)
 *                               bit4 Bose base (stock graph), else the neutral preset
 *                               bit5 app only: EQ edited per output ("Advanced"), not global ("Simple"); no DSP effect
 *     2     1  color            UI color index (opaque to the DSP)
 *     3     1  reserved
 *     4    16  name             UTF-8, NUL-padded (not necessarily NUL-terminated)
 *    20     2  sub_level_cdb    ignored since 0.2.3 (the app sub level is a device setting, config record byte
 *                               10, 0 dB in records written before); still clamped -1200..+1200
 *    22     2  crossover_hz     reserved, written as 0 (= the stock crossover, always on; the 60..250 Hz LR4
 *                               option was removed: other values saved by older firmware are ignored)
 *    24     2  aux_gain_cdb     AUX input gain, -2400..+1200
 *    26    10  trim_cdb[5]      output trims (inner L, inner R, outer L, outer R, sub), -1200..+600
 *    36     2  reserved         was bass_milli (software value of the removed bass compensation); written as 0.
 *                               The sub level of the "app" source is sub_level_cdb (include/sub_level.h)
 *    38     2  reserved
 *    40   576  band[6][12]      EQ bands: target 0 = global (stereo program), 1..5 = inner L, inner R,
 *                               outer L, outer R, sub. Each band is 8 bytes (eq_band_t below).
 *   616        (size)
 */
#ifndef PARAMS_H
#define PARAMS_H

#include <stddef.h>
#include <stdint.h>
#include "biquad.h"

#define PROFILE_VERSION   1
#define EQ_MAX_BANDS      12
#define EQ_TARGETS        6          /* global + 5 outputs */
#define EQ_OUTPUTS        5
#define PROFILE_NAME_LEN  16

enum { EQ_GLOBAL, EQ_INNER_L, EQ_INNER_R, EQ_OUTER_L, EQ_OUTER_R, EQ_SUB };
enum { BAND_OFF, BAND_PEAK, BAND_LOW_SHELF, BAND_HIGH_SHELF, BAND_HPF, BAND_LPF, BAND_NOTCH,
       BAND_TYPE_COUNT };   /* 7, 8: the former raw biquad pairs (removed, protocol 6): turned off */

#define PF_BYPASS    0x01
#define PF_LINK_LR   0x02
#define PF_LOUDNESS  0x04
#define PF_VOL_STOCK 0x08          /* ignored since layout v3: the volume is always the UAC dB value (volume.c) */
#define PF_STOCK_GRAPH 0x10        /* base = stock Bose graph (src/bose/), else the neutral preset (engine.c) */
#define PF_ADVANCED  0x20          /* app only: EQ edited per output ("Advanced") rather than global ("Simple"); no DSP effect */



/* One EQ band (8 bytes): off 0 type, 1 reserved, 2 freq_hz (20..20000), 4 gain_cdb (-1500..+1500,
 * ignored for HPF/LPF/notch), 6 q_milli (Q * 1000, 100..20000).
 * Enabled bit (OP_PROTO 4): type | BAND_DISABLED (0x81..0x86) = a parametric band switched off that keeps its
 * type, frequency, gain and Q; it is not compiled and not counted (CPU cap). BAND_OFF stays "no band". The bit
 * is only valid on BAND_PEAK..BAND_NOTCH (else params_sanitize() writes BAND_OFF). Profiles saved before have
 * bit 7 clear: unchanged, no version change; an older firmware reads a disabled band as BAND_OFF (type >=
 * BAND_TYPE_COUNT), so a profile saved by this one never plays differently there. */
#define BAND_DISABLED 0x80
/* Raw biquad pairs (types 7 / 8, BAND_RAW / BAND_RAW_DATA) were removed in profile protocol 6: the stock voicing
 * is the Bose base itself. params_sanitize() turns those slots off in profiles saved before. RAW_POLE_MAX stays:
 * it is the stability limit of the parametric base format (oc-device base/validate.rs). */
#define RAW_POLE_MAX 0.99999f       /* largest accepted pole radius */

typedef struct {
    uint8_t  type;
    uint8_t  reserved;
    uint16_t freq_hz;
    int16_t  gain_cdb;
    uint16_t q_milli;
} eq_band_t;

typedef struct {
    uint8_t   version;
    uint8_t   flags;
    uint8_t   color;
    uint8_t   reserved0;
    char      name[PROFILE_NAME_LEN];
    int16_t   sub_level_cdb;
    uint16_t  crossover_hz;
    int16_t   aux_gain_cdb;
    int16_t   trim_cdb[EQ_OUTPUTS];
    int16_t   reserved36;          /* was bass_milli */
    uint8_t   reserved1[2];
    eq_band_t band[EQ_TARGETS][EQ_MAX_BANDS];
} profile_t;

#define PROFILE_SIZE      616
#define PROFILE_BANDS_OFF 40
typedef char profile_size_check[(sizeof(profile_t) == PROFILE_SIZE) ? 1 : -1];
typedef char band_size_check[(sizeof(eq_band_t) == 8) ? 1 : -1];
typedef char res36_off_check[(offsetof(profile_t, reserved36) == 36) ? 1 : -1];

/* Rebuild flags (what must be recomputed after a change) */
#define DIRTY_TARGET(t)  (1u << (t))     /* EQ target t (0..5) */
#define DIRTY_GAINS      (1u << 10)
#define DIRTY_ALL        0x7FFu

extern profile_t g_profile;              /* active profile */

void     params_defaults(profile_t *p);
void     params_sanitize(profile_t *p);
/* Byte access to the active profile (console OPR/OPW). Writes are sanitized and applied immediately. */
int      params_read(uint32_t off, uint8_t *dst, uint32_t n);
int      params_write(uint32_t off, const uint8_t *src, uint32_t n);
/* Replaces the active profile (load from EEPROM, reset) and applies it. */
void     params_set_active(const profile_t *p);
void     params_apply(uint32_t dirty);

#endif
