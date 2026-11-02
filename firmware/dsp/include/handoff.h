/* RAM loader -> firmware handoff of the TAS link state.
 *
 * The TAS sends 'X' (pod present), 'U' (USB up), 'm', 'M' and 'V' at link start-up and on change only, so a
 * firmware started by the RAM loader never sees them: the loader copies the stock firmware's view of the link
 * here before anything is overwritten, and the firmware adopts it once at start (src/handoff.c), then clears it.
 *
 * Stock sources (re/dsp/out/img.dis):
 *   0x10006D61 byte  pod present = 'X' byte != 0          (handler 0x1001494C, store 0x10014988)
 *   0x10006D60 byte  USB up, set by 'U' (task code 7)     (read at 0x1001570C)
 *   0x10010100 float USB volume % = (V + 32768) / 327.68  ('V' handler, xref 0x10014B98)
 *   0x10010108 float USB mute, 1.0 = muted                ('m' handler 0x100148F0; also 0x10015798)
 *   0x10010114 float pod mute, 1.0 = muted, 2.0 = normal  ('M', task code 4, 0x10015F1C)
 * Location: 0x10024E00, above the loader stack (top 0x10024DF8) and below the end of the stock .cinit table
 * (0x10024E3F): dead in the stock firmware, inside the LOADER area the firmware never loads into (lnk.cmd).
 * A cold boot (EEPROM image) normally finds no valid record: magic + complement + checksum. */
#ifndef HANDOFF_H
#define HANDOFF_H

#include <stdint.h>

#define HANDOFF_ADDR   0x10024E00u
#define HANDOFF_MAGIC  0x4F48434Fu        /* "OCHO" little-endian */

typedef struct {
    uint32_t magic;
    uint32_t inv;                         /* ~magic */
    uint8_t  pod_present, usb_up, usb_mute, pod_mute;
    float    usb_pct;                     /* stock 0x10010100 */
    uint32_t check;                       /* magic ^ word 2 ^ word 3 */
} handoff_t;

#define HANDOFF ((volatile handoff_t *)HANDOFF_ADDR)

static inline uint32_t handoff_word(const volatile void *p) { return *(const volatile uint32_t *)p; }

/* Loader side: capture the stock state (stock firmware still intact in RAM). */
static inline void handoff_capture(void)
{
    volatile handoff_t *h = HANDOFF;
    h->pod_present = *(volatile uint8_t *)0x10006D61u != 0;
    h->usb_up = *(volatile uint8_t *)0x10006D60u != 0;
    h->usb_mute = *(volatile float *)0x10010108u != 0.0f;
    h->pod_mute = *(volatile float *)0x10010114u == 1.0f;
    h->usb_pct = *(volatile float *)0x10010100u;
    h->magic = HANDOFF_MAGIC;
    h->inv = ~HANDOFF_MAGIC;
    h->check = HANDOFF_MAGIC ^ handoff_word(&h->pod_present) ^ handoff_word(&h->usb_pct);
}

#endif
