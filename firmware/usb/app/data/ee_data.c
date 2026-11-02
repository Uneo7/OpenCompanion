/*
 * Payload of the U6002 data area (data/ee_layout.h). NOT linked into the firmware: built on its own into
 * build/oc_tas/oc_tas_data.bin (scripts/mkeedata.py), written by the host at 0x1800, streamed by I2C at runtime.
 *
 * UAC2 descriptors follow the usbaudio2.sys constraints (README "USB Audio Class 2.0"): one Clock Source with
 * GET CUR/RANGE and no validity control; discrete rates as MIN = MAX, RES = 0; Feature Unit controls on the master
 * channel only (the driver ignores master controls if per-channel ones exist); 24-bit audio in 4-byte subslots,
 * never 3 (audio/stream.c); adaptive data endpoint, no feedback endpoint. Unit IDs match the stock UAC1 descriptor.
 */
#include "config.h"
#include "descriptors.h"
#include "ee_layout.h"
#include "stream.h"

#define LE16(x) ((x) & 0xFF), (((x) >> 8) & 0xFF)
/* desc_tab entry: type, index (wValue), length, EEPROM address of a member (+ offset) */
#define DESC(type, index, len, member, off) type, index, LE16(len), LE16(EE_ADDR(member) + (off))
#define LE32(x) LE16((x) & 0xFFFF), LE16((x) >> 16)

#define CS_INTERFACE   0x24
#define CS_ENDPOINT    0x25
#define AUDIO          0x01
#define AC_SUBCLASS    0x01
#define AS_SUBCLASS    0x02
#define IP_2_0         0x20          /* AF_VERSION_02_00 / IP_VERSION_02_00 (ADC-2 A.3, A.6) */

/* Pieces shared by the two configuration descriptors */
#define CONFIG_HEAD(len)                                                                                \
    9, 0x02, LE16(len), USB_NUM_INTERFACES, 1, 0, 0xC0, 50                      /* self-powered */
#define AC_IF(ifac)                                                                                     \
    8, 0x0B, ifac, 2, AUDIO, 0x00, IP_2_0, 0,                                   /* IAD, ADC-2 table 4-3 */ \
    9, 0x04, ifac, 0, 1, AUDIO, AC_SUBCLASS, IP_2_0, 0                          /* AC, 1 endpoint */
#define AC_HEAD(total, nch, cfg)                                                                        \
    9, CS_INTERFACE, 0x01, LE16(0x0200), 0x01, LE16(total), 0x00,               /* HEADER, desktop speaker */ \
    8, CS_INTERFACE, 0x0A, UAC_ID_CLOCK, 0x05, 0x01, 0, 0,  /* CLOCK_SOURCE: internal fixed 48 kHz, SOF-synced;   \
                                                             * frequency control read-only, no validity */    \
    17, CS_INTERFACE, 0x02, UAC_ID_IT, LE16(0x0101), 0, UAC_ID_CLOCK,           /* IT: USB streaming */  \
        nch, LE32(cfg), 0, LE16(0x0000), 0                                      /* cluster, no controls */
#define FU_MASTER(len)                                                                                  \
    len, CS_INTERFACE, 0x06, UAC_ID_FU, UAC_ID_IT,                              /* FU */                 \
        LE32(0x0000000F)                                                        /* master: mute + volume */
#define AC_TAIL                                                                                         \
    12, CS_INTERFACE, 0x03, UAC_ID_OT, LE16(0x0301), 0, UAC_ID_FU, UAC_ID_CLOCK, LE16(0x0000), 0, /* speaker */ \
    7, 0x05, 0x80 | EP_AC_INT, 0x03, LE16(AC_INT_MSG_LEN), AC_INT_INTERVAL      /* interrupt IN, ADC-2 4.8.2.1 */
#define AS_ALTS(ifas, nch, cfg, subslot, bits, maxpkt)                                                  \
    9, 0x04, ifas, 0, 0, AUDIO, AS_SUBCLASS, IP_2_0, 0,                         /* alt 0: no bandwidth */ \
    9, 0x04, ifas, 1, 1, AUDIO, AS_SUBCLASS, IP_2_0, 0,                         /* alt 1 */              \
    16, CS_INTERFACE, 0x01, UAC_ID_IT, 0x00, 0x01, LE32(0x00000001),            /* AS_GENERAL: PCM */    \
        nch, LE32(cfg), 0,                                                                              \
    6, CS_INTERFACE, 0x02, 0x01, subslot, bits,                                 /* FORMAT_TYPE_I */      \
    7, 0x05, EP_ISO_OUT, 0x09, LE16(maxpkt), 1,                                 /* iso OUT, adaptive */  \
    8, CS_ENDPOINT, 0x01, 0x00, 0x00, 0, LE16(0)                                /* EP_GENERAL */
#define HID_IF(ifhid)                                                                                   \
    9, 0x04, ifhid, 0, 2, 0x03, 0x00, 0x00, 0,                  /* vendor HID, 64-byte reports, no report ID */ \
    9, 0x21, 0x11, 0x01, 0x00, 1, 0x22, HID_VENDOR_REP_LEN, 0,                                          \
    7, 0x05, 0x80 | EP_HID_VENDOR, 0x03, LE16(HID_REPORT_SIZE), 1,                                      \
    7, 0x05, EP_HID_VENDOR, 0x03, LE16(HID_REPORT_SIZE), 1
/* Device descriptor: the stock VID:PID with our bcdDevice (config.h) */
#define DEVICE_DESC                                                                                     \
    18, 0x01,               /* DEVICE */                                                                \
    0x00, 0x02,             /* bcdUSB 2.00 (full-speed only: device_qualifier is stalled, USB 2.0 9.6.2) */ \
    0xEF, 0x02, 0x01,       /* Miscellaneous / Common class / IAD (IAD ECN, Microsoft "Support for IAD") */ \
    8,                      /* bMaxPacketSize0 */                                                       \
    LE16(USB_VID),          /* 05A7:1020, as the stock firmware (config.h) */                           \
    LE16(USB_PID),                                                                                      \
    LE16(USB_BCD_DEVICE),                                                                               \
    1, 2, 0,                /* manufacturer, product, no serial */                                      \
    1
/* The HID class descriptor is requested with wIndex = interface and wValue index 0 */
#define DESC_TAB(config, len, hidoff)                                                                   \
    DESC(0x01, 0, 18, device, 0),                                                                       \
    DESC(0x02, 0, len, config, 0),                                                                      \
    DESC(0x03, 0, 4, str_langid, 0),                                                                    \
    DESC(0x03, 1, STR_MANUFACTURER_LEN, str_manufacturer, 0),                                           \
    DESC(0x03, 2, STR_PRODUCT_LEN, str_product, 0),                                                     \
    DESC(0x21, 0, 9, config, hidoff),                                                                   \
    DESC(0x22, 0, HID_VENDOR_REP_LEN, hid_report, 0),                                                   \
    0                                                                           /* end */

__code const struct ee_data ee_data = {
    /* .desc_tab_51 */ { DESC_TAB(config_51, CONFIG_51_LEN, HID_DESC_OFF_51) },
    /* .desc_tab_21 */ { DESC_TAB(config_21, CONFIG_21_LEN, HID_DESC_OFF_21) },

    /* .knob_db: pod knob 0..99 % -> attenuation in dB, the stock pod volume curve (DSP volume_curve.c
     * VC_STOCK_POD), rounded to whole dB; checked by scripts/mkeedata.py. */ {
    71, 71, 71, 70, 63, 61, 60, 58, 57, 56,     /*  0.. 9 % */
    54, 53, 51, 50, 49, 47, 46, 44, 43, 42,     /* 10..19 % */
    40, 39, 37, 36, 35, 33, 32, 30, 29, 28,     /* 20..29 % */
    26, 25, 23, 22, 21, 19, 19, 19, 18, 18,     /* 30..39 % */
    18, 17, 17, 17, 17, 16, 16, 16, 15, 15,     /* 40..49 % */
    15, 14, 14, 14, 14, 13, 13, 13, 12, 12,     /* 50..59 % */
    12, 11, 11, 11, 11, 10, 10, 10,  9,  9,     /* 60..69 % */
     9,  8,  8,  8,  8,  7,  7,  7,  6,  6,     /* 70..79 % */
     6,  5,  5,  5,  5,  4,  4,  4,  3,  3,     /* 80..89 % */
     3,  2,  2,  2,  2,  1,  1,  1,  0,  0,     /* 90..99 % */
    },

    /* .device */ { DEVICE_DESC },

    /* .config_51: mode 5.1, alt 1 = 6 channels, 16 bits in 2 bytes (stock format) */ {
    CONFIG_HEAD(CONFIG_51_LEN),
    AC_IF(IF_AC_51),
    AC_HEAD(AC_CS_TOTAL_51, CH_51, CFG_51),
    FU_MASTER(34), LE32(0), LE32(0), LE32(0), LE32(0), LE32(0), LE32(0), 0,     /* channels 1..6: none */
    AC_TAIL,
    AS_ALTS(IF_AS_51, CH_51, CFG_51, 2, 16, ISO_MAXPKT_51),
    HID_IF(IF_HID_51)
    },

    /* .config_21: mode 2.1, alt 1 = 2 channels, 24 bits in 4 bytes (the DSP adds the subwoofer). HID first
     * (interface 0), audio function on interfaces 1..2: another Windows device node than mode 5.1 (config.h) */ {
    CONFIG_HEAD(CONFIG_21_LEN),
    HID_IF(IF_HID_21),
    AC_IF(IF_AC_21),
    AC_HEAD(AC_CS_TOTAL_21, CH_20, CFG_20),
    FU_MASTER(18), LE32(0), LE32(0), 0,                                         /* channels 1..2: none */
    AC_TAIL,
    AS_ALTS(IF_AS_21, CH_20, CFG_20, 4, 24, ISO_MAXPKT_20),
    },

    /* .str_langid */ { 4, 0x03, 0x09, 0x04 },
    /* .str_manufacturer */ {
    STR_MANUFACTURER_LEN, 0x03,
    'O', 0, 'p', 0, 'e', 0, 'n', 0, 'C', 0, 'o', 0, 'm', 0, 'p', 0, 'a', 0, 'n', 0, 'i', 0, 'o', 0, 'n', 0
    },
    /* .str_product */ {
    STR_PRODUCT_LEN, 0x03,
    'C', 0, 'o', 0, 'm', 0, 'p', 0, 'a', 0, 'n', 0, 'i', 0, 'o', 0, 'n', 0, ' ', 0, '5', 0, ' ', 0, '(', 0,
    'O', 0, 'p', 0, 'e', 0, 'n', 0, 'C', 0, 'o', 0, 'm', 0, 'p', 0, 'a', 0, 'n', 0, 'i', 0, 'o', 0, 'n', 0, ')', 0
    },

    /* .freq_range: Sampling Frequency Control RANGE (layout 3), one discrete subrange: 48 kHz only (0.5.1) */ {
    LE16(1),
    LE32(48000), LE32(48000), LE32(0),
    },

    /* .vol_range: Volume Control RANGE (layout 2), 1 subrange [wMIN wMAX wRES] */ {
    LE16(1), LE16(VOL_MIN), LE16(VOL_MAX), LE16(VOL_RES),
    },

    /* .hid_report: vendor HID, 64-byte input and output reports, no report ID */ {
    0x06, 0x00, 0xFF,       /* Usage Page (vendor 0xFF00) */
    0x09, 0x01,             /* Usage 1 */
    0xA1, 0x01,             /* Collection (Application) */
    0x15, 0x00,             /*   Logical Minimum 0 */
    0x26, 0xFF, 0x00,       /*   Logical Maximum 255 */
    0x75, 0x08,             /*   Report Size 8 */
    0x95, 0x40,             /*   Report Count 64 */
    0x09, 0x01, 0x81, 0x02, /*   Input (data, var, abs) */
    0x09, 0x01, 0x91, 0x02, /*   Output (data, var, abs) */
    0xC0                    /* End Collection */
    },
};

/* An array initialiser shorter than its member is zero-padded silently: check the descriptor lengths. */
_Static_assert(sizeof(struct ee_data) == 2 * EE_DESC_TAB_LEN + 18 + CONFIG_51_LEN + CONFIG_21_LEN + 4 +
               STR_MANUFACTURER_LEN + STR_PRODUCT_LEN + 14 + 8 + HID_VENDOR_REP_LEN + 100,
               "struct ee_data has padding");
