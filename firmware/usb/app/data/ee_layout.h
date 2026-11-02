/*
 * U6002 data area: constant firmware data stored in the TAS EEPROM instead of the 6016-byte code RAM.
 * U6002 (AT24C64, 32-byte pages): 0x0000 TI boot header, 0x0012..0x15F5 stock code, 0x0012..0x1791 largest
 * OpenCompanion image, 0x1800..0x1FFF data area: it overlaps neither image, so it survives code updates.
 *
 * Data area: [0..3] "OCDA", [4] EE_LAYOUT_VERSION (change it whenever struct ee_data changes), [5] 0,
 * [6..7] payload length LE, [8..11] CRC-32 (zlib) of the payload LE; then struct ee_data, then 0xFF.
 * If the area is invalid the firmware never connects to USB; the speaker keeps working on AUX.
 * Host side: app/crates/oc-tas-image (data_area).
 */
#ifndef EE_LAYOUT_H
#define EE_LAYOUT_H

#include <stddef.h>
#include <stdint.h>

#include "descriptors.h"

#define EE_AREA_START      0x1800u
#define EE_AREA_END        0x2000u        /* exclusive */
#define EE_HDR_LEN         12
#define EE_LAYOUT_VERSION  8

/* Audio mode record, 1 byte in the gap before the data area, outside the CRC. 2 = mode 2.1, anything else (blank,
 * torn write) = 5.1, so it can never stop enumeration. Written only by HID MODE (one verified byte); EE_WRITE
 * refuses 0x1792..0x17FF and host tools never write it, so the user's mode survives updates. */
#define EE_MODE_ADDR       0x17FFu
#define EE_MAGIC           'O', 'C', 'D', 'A'

#define EE_DESC_ENTRY      6        /* desc_tab entry: type, index, length LE16, EEPROM address LE16 */
#define EE_DESC_COUNT      7        /* device, configuration, strings 0..2, HID, HID report */
#define EE_DESC_TAB_LEN    (EE_DESC_COUNT * EE_DESC_ENTRY + 1)   /* 43, 0-terminated */

#define STR_MANUFACTURER_LEN  28
#define STR_PRODUCT_LEN       56

struct ee_data {
    uint8_t desc_tab_51[EE_DESC_TAB_LEN];  /* first member: address EE_DESC_TAB, used in assembly */
    uint8_t desc_tab_21[EE_DESC_TAB_LEN];  /* at EE_DESC_TAB + 43 x audio_mode */
    uint8_t knob_db[100];                  /* third member: fixed address 0x1862, used in assembly (pod_sync.c) */
    uint8_t device[18];                    /* device descriptor (both modes) */
    uint8_t config_51[CONFIG_51_LEN];
    uint8_t config_21[CONFIG_21_LEN];
    uint8_t str_langid[4];                 /* string 0 */
    uint8_t str_manufacturer[STR_MANUFACTURER_LEN];   /* string 1 */
    uint8_t str_product[STR_PRODUCT_LEN];  /* string 2 */
    uint8_t freq_range[14];                /* one subrange, 48000 (0.5.1); GET CUR at +2 */
    uint8_t vol_range[8];                  /* Volume RANGE (layout 2) */
    uint8_t hid_report[HID_VENDOR_REP_LEN];   /* vendor HID report descriptor */
};

#define EE_DESC_TAB  (EE_AREA_START + EE_HDR_LEN)   /* = EE_ADDR(desc_tab_51) (_Static_assert in usb/usb_app.c) */

#define EE_ADDR(m)   (EE_AREA_START + EE_HDR_LEN + offsetof(struct ee_data, m))
#define EE_LEN(m)    sizeof(((struct ee_data *)0)->m)
/* The USB core streams addresses >= USB_EE_BASE (= EE_AREA_START) from the EEPROM. */
#define EE_PTR(m)    ((__code const uint8_t *)EE_ADDR(m))

#endif /* EE_LAYOUT_H */
