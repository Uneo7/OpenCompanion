/*
 * UAC2 class requests (ADC-2 5.2) to the AudioControl interface: Clock Source 5 sampling frequency (GET CUR/RANGE,
 * SET CUR 44100 / 48000; RANGE tables streamed from the U6002 data area) and Feature Unit 3 master mute / volume.
 * Everything else stalls (ADC-2 5.2.1). GET requests are truncated to wLength: hosts first read 2 bytes of a RANGE.
 * SET CUR volume 0x8000 (silence) must be accepted: below MIN is stored as MIN, above MAX as MAX.
 */
#include "config.h"
#include "descriptors.h"
#include "ee_layout.h"
#include "stream.h"
#include "tas1020b.h"
#include "uac.h"
#include "tick.h"
#include "uac_notify.h"
#include "usb_core.h"

#define UAC2_CUR       0x01
#define UAC2_RANGE     0x02
#define CS_SAM_FREQ    0x01
#define CS_MUTE        0x01
#define CS_VOLUME      0x02

__data int16_t uac_volume = (int16_t)0xC900;   /* -55 dB, stock initial 0xC8CF on the 1 dB grid */
__bit uac_mute;
__bit uac_volume_dirty;
__bit uac_mute_dirty;
__bit uac_rate_dirty;

static __data uint8_t pending;             /* control targeted by the SET CUR data stage */
#define PENDING_MUTE    1
#define PENDING_VOLUME  2

_Static_assert(VOL_MIN == (int16_t)0x8100 && VOL_MAX == 0, "uac_apply_volume clamps on the high byte");

static uint8_t set_cur(uint8_t what)
{
    pending = what;
    usb_ep0_receive();
    return 1;
}

static uint8_t clock_source(uint8_t get)
{
    if (SETUP_VALUE_H != CS_SAM_FREQ) {
        return 0;
    }
    if (!get) {
        return 0;                               /* fixed 48 kHz clock (0.5.1): frequency control read-only */
    }
    if (SETUP_REQUEST == UAC2_RANGE) {
        usb_ep0_send_code(EE_PTR(freq_range), EE_LEN(freq_range));
    } else {
        usb_ep0_send_code(EE_PTR(freq_range) + 2, 4);   /* GET CUR = MIN of the single subrange: 48000 */
    }
    return 1;
}

static uint8_t feature_unit(uint8_t get)
{
    uint8_t b[1];

    usb_t[1] = (uint8_t)(tick_now() >> 8);      /* any FU request: the host's reaction time (INFO 24) */

    if (SETUP_VALUE_L != 0) {
        return 0;                               /* master channel only */
    }
    if (SETUP_VALUE_H == CS_MUTE) {
        if (SETUP_REQUEST != UAC2_CUR) {
            return 0;
        }
        if (!get) {
            return set_cur(PENDING_MUTE);
        }
        b[0] = uac_mute;
        usb_ep0_send_ram(b, 1);
        return 1;
    }
    if (SETUP_VALUE_H != CS_VOLUME) {
        return 0;
    }
    if (!get) {
        return set_cur(PENDING_VOLUME);
    }
    if (SETUP_REQUEST == UAC2_RANGE) {
        usb_ep0_send_code(EE_PTR(vol_range), EE_LEN(vol_range));
        return 1;
    }
    usb_ep0_send_ram((__data uint8_t *)&uac_volume, 2);    /* SDCC keeps int16 little-endian, as USB */
    return 1;
}

uint8_t uac_setup(void)
{
    uint8_t get = SETUP_REQTYPE & 0x80;

    pending = 0;
    if ((SETUP_REQTYPE & 0x7F) != 0x21 || SETUP_INDEX_L != IF_AC ||
        (SETUP_REQUEST != UAC2_CUR && SETUP_REQUEST != UAC2_RANGE)) {
        return 0;                               /* class + interface recipient + AC interface only */
    }
    if (!get && SETUP_REQUEST != UAC2_CUR) {
        return 0;                               /* SET RANGE does not exist (ADC-2 5.2.2) */
    }
    if (SETUP_INDEX_H == UAC_ID_CLOCK) {
        return clock_source(get);
    }
    if (SETUP_INDEX_H == UAC_ID_FU) {
        return feature_unit(get);
    }
    return 0;
}

void uac_apply_volume(int16_t v)
{
    /* Clamp with the high byte only: MAX = 0x0000, MIN = 0x8100 (descriptors.h, _Static_assert above) */
    uint8_t h = (uint8_t)((uint16_t)v >> 8);
    if (!(h & 0x80)) {
        v = VOL_MAX;                            /* >= 0 dB */
    } else if (h == 0x80) {
        v = VOL_MIN;                            /* 0x8000 (silence) .. 0x80FF: below MIN */
    }
    if (v != uac_volume) {
        uac_volume = v;
        uac_volume_dirty = 1;
    }
}

void uac_apply_mute(uint8_t m)
{
    if ((m != 0) != uac_mute) {
        uac_mute = !uac_mute;
        uac_mute_dirty = 1;
    }
}

uint8_t uac_ep0_out(const __xdata uint8_t *buf, uint8_t len)
{
    uint8_t p = pending;
    int16_t v;

    (void)buf;                                  /* always the EP0 OUT buffer: read at its fixed address */
    pending = 0;
    if (p == PENDING_MUTE && len) {
        uac_apply_mute(XREG(EP0_OUT_BUF) & 1);
        return 1;
    }
    v = *(volatile __xdata int16_t *)EP0_OUT_BUF;   /* little-endian, as SDCC's int16 */
    if (p == PENDING_VOLUME && len >= 2) {
        uac_apply_volume(v);
        return 1;
    }
    return 0;
}
