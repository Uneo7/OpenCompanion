/* Boot-chain integration, see include/bootctl.h. Record formats: boot/docs/LAYOUT.md (header "OCI1" 48 bytes,
 * CRC-32 of bytes 0..43 at 44, flags at 24 with bit 0 HF_TRIAL; boot-state "OCBS": seq, attempts (byte 8),
 * confirmed (byte 9, 0xA5), CRC-32 of bytes 0..11 at 12), same values as boot1/include/image.h, trial.h. */
#include <string.h>
#include "oc.h"
#include "bootctl.h"
#include "eeprom.h"
#include "profiles.h"
#include "crc32.h"
#include "console_util.h"

#define HDR_MAGIC       0x3149434Fu   /* "OCI1" */
#define HDR_LEN         48u
#define HF_TRIAL        1u
#define STATE_MAGIC     0x5342434Fu   /* "OCBS" */
#define STATE_LEN       16u
#define STATE_CONFIRMED 0xA5u
#define RESCUE_FLAG     0x1002DFF0u   /* boot1 RAM_FLAG */
#define RESCUE_MAGIC    0x5352434Fu   /* "OCRS" */
#define TRIAL_FRAMES    5000u         /* 10 s of audio frames after the link is up */

enum { S_IDLE, S_UPD_CHK, S_UPD_WR, S_UPD_RD, S_HDR_RD, S_ST_RD, S_ST_WR, S_ST_VERIFY };

static int state, pending, cold_boot, upd_reboot;
static uint8_t trial = BT_WAIT, result = BR_NONE;
static uint8_t buf[EE_PAGE], rec[STATE_LEN];
static uint32_t hdr_seq;

void bootctl_init(int cold)
{
    cold_boot = cold;
    state = S_IDLE;
    pending = 0;
    trial = cold ? BT_WAIT : BT_SKIPPED_RAM;   /* a RAM-loaded test image must never confirm the EEPROM one */
}

int bootctl_upd(void)
{
    if (state != S_IDLE) return -1;
    state = S_UPD_CHK;                         /* first make sure the EEPROM holds the OpenCompanion layout */
    result = BR_BUSY;
    return 0;
}

void bootctl_rsc(void)
{
    *(volatile uint32_t *)RESCUE_FLAG = RESCUE_MAGIC;
    *(volatile uint32_t *)(RESCUE_FLAG + 4u) = ~RESCUE_MAGIC;
    g_reboot_request = 1;
}

static void finish(uint8_t r, uint8_t t)
{
    result = r;
    if (t != BT_WAIT) trial = t;
    if (r != BR_OK && state >= S_HDR_RD) trial = BT_ERROR;
    state = S_IDLE;
}

static int issue(void)
{
    switch (state) {
    case S_UPD_CHK: return eeprom_read(EE_HDR, buf, HDR_LEN);
    case S_UPD_WR:  return eeprom_write(EE_HDR, buf, EE_PAGE);
    case S_UPD_RD:  return eeprom_read(EE_HDR, buf, EE_PAGE);
    case S_HDR_RD:  return eeprom_read(EE_HDR, buf, HDR_LEN);
    case S_ST_RD:   return eeprom_read(EE_STATE, rec, STATE_LEN);
    case S_ST_WR:   return eeprom_write(EE_STATE, rec, STATE_LEN);
    default:        return eeprom_read(EE_STATE, buf, STATE_LEN);   /* S_ST_VERIFY */
    }
}

static int all_zero(const uint8_t *p, uint32_t n)
{
    while (n--) if (*p++) return 0;
    return 1;
}

static void step(void)
{
    switch (state) {
    case S_UPD_CHK:
        /* Never on the stock EEPROM: there 0x0100 is part of the stock AIS stream and zeroing it would leave
         * a DSP that cannot boot (no boot1 yet). Only a valid "OCI1" header (migrated layout) is invalidated. */
        if (oc_get32(buf) != HDR_MAGIC || oc_get32(buf + 44) != oc_crc32(buf, 44)) { finish(BR_REFUSED, BT_WAIT); break; }
        memset(buf, 0, EE_PAGE);
        state = S_UPD_WR;
        break;
    case S_UPD_WR:
        state = S_UPD_RD;
        break;
    case S_UPD_RD:
        if (!all_zero(buf, EE_PAGE)) { finish(BR_VERIFY, BT_WAIT); break; }
        finish(BR_OK, BT_WAIT);
        upd_reboot = 1;
        g_reboot_request = 1;                    /* boot1 -> rescue, reason 3 (no header) */
        break;
    case S_HDR_RD:
        if (oc_get32(buf) != HDR_MAGIC || oc_get32(buf + 44) != oc_crc32(buf, 44)) { finish(BR_OK, BT_NO_HEADER); break; }
        if (!(oc_get32(buf + 24) & HF_TRIAL)) { finish(BR_OK, BT_NOT_TRIAL); break; }
        hdr_seq = oc_get32(buf + 8);
        state = S_ST_RD;
        break;
    case S_ST_RD: {
        uint8_t attempts = 0;
        if (oc_get32(rec) == STATE_MAGIC && oc_get32(rec + 4) == hdr_seq && oc_get32(rec + 12) == oc_crc32(rec, 12)) {
            if (rec[9] == STATE_CONFIRMED) { finish(BR_OK, BT_CONFIRMED); break; }
            attempts = rec[8];
        }
        memset(rec, 0, STATE_LEN);
        oc_put32(rec, STATE_MAGIC);
        oc_put32(rec + 4, hdr_seq);
        rec[8] = attempts;
        rec[9] = STATE_CONFIRMED;
        oc_put32(rec + 12, oc_crc32(rec, 12));
        state = S_ST_WR;
        break;
    }
    case S_ST_WR:
        state = S_ST_VERIFY;
        break;
    default:                                     /* S_ST_VERIFY */
        finish(memcmp(buf, rec, STATE_LEN) ? BR_VERIFY : BR_OK, BT_WROTE);
        break;
    }
}

void bootctl_poll(void)
{
    if (state == S_IDLE) {
        if (trial == BT_WAIT && g_audio.usb_up && g_audio.frames >= TRIAL_FRAMES && !upd_reboot) {
            state = S_HDR_RD;                    /* link up (DSPINT done, 'U' seen) and 10 s of audio */
            result = BR_BUSY;
        }
        return;
    }
    if (pending) {
        int r = eeprom_poll();
        if (r == EE_BUSY) return;
        pending = 0;
        if (r != EE_DONE) { finish(BR_IO, BT_WAIT); return; }
        step();
        if (state == S_IDLE) return;
    }
    if (profiles_busy() || eeprom_busy()) return;   /* never overlap a profile job */
    if (issue()) { finish(BR_IO, BT_WAIT); return; }
    pending = 1;
}

/* UPD\r -> "UPD\r>" (started; reboots into rescue once the header page reads back as zeros)
 * RSC\r -> "RSC\r>" then reboot with the rescue flag
 * BST\r -> "BS:<state> R:<result> T:<trial> C:<cold>\r>" */
int console_boot(const uint8_t *msg, int len, uint8_t *out, int max)
{
    (void)len;
    if (!memcmp(msg, "UPD", 3))
        return cu_str(out, 0, max, bootctl_upd() ? "\r!" : "UPD\r>");
    if (!memcmp(msg, "RSC", 3)) {
        bootctl_rsc();
        return cu_str(out, 0, max, "RSC\r>");
    }
    if (!memcmp(msg, "BST", 3)) {
        static const char LAB[] = "BS:\0 R:\0 T:\0 C:";
        long v[4];
        v[0] = state;
        v[1] = result;
        v[2] = trial;
        v[3] = cold_boot;
        return cu_str(out, cu_fields(out, 0, max, LAB, v, 4), max, "\r>");
    }
    return -1;
}
