/*
 * Vendor HID protocol (docs/protocol_usb_hid.md), 64-byte reports without report ID.
 *   request: [cmd][tag][len][payload, len <= 61]
 *   reply  : [cmd][tag][status][len][payload, len <= 60]
 * The OUT buffer stays NAKed until the request is handled: requests are strictly sequential.
 */
#include <8051.h>

#include "config.h"
#include "dsp_link.h"
#include "eeprom.h"
#include "hid.h"
#include "pod.h"
#include "pod_sync.h"
#include "power.h"
#include "protocol.h"
#include "servo.h"
#include "stream.h"
#include "tas1020b.h"
#include "tick.h"
#include "uac.h"
#include "uac_notify.h"
#include "usb_core.h"

#define CMD_INFO        0x01
#define CMD_DSP         0x10
#define CMD_EE_READ     0x20
#define CMD_EE_BEGIN    0x21
#define CMD_EE_WRITE    0x22
#define CMD_EE_COMMIT   0x23
#define CMD_EE_END      0x24
#define CMD_MODE        0x30
#define CMD_REBOOT_DFU  0x7E
#define CMD_REBOOT      0x7F
_Static_assert(ACTION_DFU == CMD_REBOOT_DFU && ACTION_REBOOT == CMD_REBOOT && ACTION_MODE == CMD_MODE,
               "dispatch() stores the command code");


__data uint8_t protocol_action;

static __data uint8_t rq_len;
static __data uint8_t rp_len;
#define RX      HID_RX
#define TX      HID_TX
#define ARG(i)  RX[RQ_DATA + (i)]
#define OUT     (TX + RP_DATA)

/* Payload starts with 'O' 'C' <cmd> <~cmd> (MODE: then the mode byte). In assembly for size; ARG(3) == ~cmd is
 * tested as ARG(3) + cmd + 1 == 0. */
_Static_assert(EP2_OUT_BUF == 0xFA60 && RQ_CMD == 0 && RQ_DATA == 3, "constants of key_ok()");

static uint8_t key_ok(void) __naked
{
    __asm
        mov  a, _rq_len
        add  a, #0xFC                           ; carry if rq_len >= 4
        jnc  00001$
        mov  dptr, #0xFA60                      ; RX[RQ_CMD]
        movx a, @dptr
        mov  r7, a
        mov  dptr, #0xFA63                      ; ARG(0)
        movx a, @dptr
        cjne a, #0x4F, 00001$                   ; 'O'
        inc  dptr
        movx a, @dptr
        cjne a, #0x43, 00001$                   ; 'C'
        inc  dptr
        movx a, @dptr
        xrl  a, r7
        jnz  00001$                             ; ARG(2) == cmd
        inc  dptr
        movx a, @dptr
        add  a, r7
        inc  a
        jnz  00001$                             ; ARG(3) == ~cmd
        mov  dpl, #1
        ret
    00001$:
        mov  dpl, #0
        ret
    __endasm;
}


/* Bit moves in assembly: SDCC bit-to-byte code is ~2x larger. */
static uint8_t flags0(void) __naked
{
    __asm
        mov  a, _usb_configuration              ; b3 configured
        jz   00001$
        mov  a, #0x08
    00001$:
        mov  c, _usb_present
        mov  acc.0, c
        mov  c, _usb_active
        mov  acc.1, c
        mov  c, _usb_suspended
        mov  acc.2, c
        mov  c, _dsp_link_up
        mov  acc.4, c
        mov  c, _dsp_link_busy
        mov  acc.5, c
        mov  c, _pod_present
        mov  acc.6, c
        mov  c, _pod_mute
        mov  acc.7, c
        mov  dpl, a
        ret
    __endasm;
}

static uint8_t flags1(void) __naked
{
    __asm
        clr  a
        mov  c, _uac_mute
        mov  acc.0, c
        mov  c, _pod_knob_ok
        mov  acc.1, c
        mov  c, _ee_session
        mov  acc.3, c
        mov  c, _pod_escape_armed
        mov  acc.4, c
        mov  c, 0x97                            ; P1.7 (BADDATA)
        mov  acc.5, c
        mov  dpl, a
        ret
    __endasm;
}

/* INFO bytes 4..24 are copied through info_src: < 0x80 = internal-RAM address, >= 0x80 = register 0xFFxx.
 * Flags and tick are computed first into R2..R5 of bank 0 (the firmware only uses bank 0, internal RAM is full).
 * In assembly for size. */
#define INFO_TMP(i) ((__data uint8_t *)(2 + (i)))      /* R2..R5: flags0, flags1, tick L, tick H */
#define REG_LO(r)   ((__data uint8_t *)(uint8_t)(uint16_t)&(r))
#define INFO_SRC_N  21
static __data uint8_t * __code const info_src[INFO_SRC_N] = {
    REG_LO(MEMCFG), INFO_TMP(0), INFO_TMP(1),                                         /* 4..6 */
    &stream_alt, &stream_rate,                                                          /* 7..8 */
    (__data uint8_t *)&uac_volume, (__data uint8_t *)&uac_volume + 1,                   /* 9..10 */
    (__data uint8_t *)&servo_off, (__data uint8_t *)&servo_off + 1,                     /* 11..12 */
    (__data uint8_t *)&servo_acc, (__data uint8_t *)&servo_acc + 1,                     /* 13..14 */
    &servo_resyncs,                                                                     /* 15 */
    REG_LO(DMABCNT0L), REG_LO(DMABCNT0H), REG_LO(GLOBCTL),                              /* 16..18 */
    INFO_TMP(2), INFO_TMP(3),                                                         /* 19..20 tick */
    &pod_knob, &pod_applied,                                                            /* 21..22 */
    &usb_t[0], &usb_t[1],                                                               /* 23..24 (uac_notify.h) */
};
_Static_assert(HID_REP_DATA == 0xFA24 && INFO_SRC_N + 4 == 25 && PROTOCOL_VERSION < 256, "constants of cmd_info()");
_Static_assert(FW_VERSION_MAJOR == 0 && FW_VERSION_MINOR == 5 && FW_VERSION_PATCH == 4 && PROTOCOL_VERSION == 4,
               "cmd_info() asm: update the version immediates");

/* Uses P2 = 0xFA for MOVX @R1 (P2 is free, common/xreg.c). */
static uint8_t cmd_info(void) __naked
{
    __asm
        lcall _tick_now                         ; uses no register of R2..R5 (tick.c)
        mov  r4, dpl
        mov  r5, dph
        lcall _flags0                           ; accumulator and carry only
        mov  r2, dpl
        lcall _flags1
        mov  r3, dpl
        mov  0xA0, #0xFA                        ; P2 = reply page
        mov  r1, #0x24                          ; OUT[0] = 0xFA24
        mov  a, #4                              ; PROTOCOL_VERSION
        movx @r1, a
        inc  r1
        clr  a                                  ; FW_VERSION_MAJOR
        movx @r1, a
        inc  r1
        mov  a, #5                              ; FW_VERSION_MINOR
        movx @r1, a
        inc  r1
        mov  a, #4                                   ; FW_VERSION_PATCH
        movx @r1, a
        inc  r1
        mov  r7, #0
    00001$:
        mov  dptr, #_info_src
        mov  a, r7
        movc a, @a+dptr
        jb   acc.7, 00002$
        mov  r0, a                              ; internal RAM
        mov  a, @r0
        sjmp 00003$
    00002$:
        mov  dpl, a                             ; register 0xFFxx
        mov  dph, #0xFF
        movx a, @dptr
    00003$:
        movx @r1, a
        inc  r1
        inc  r7
        cjne r7, #21, 00001$                    ; INFO_SRC_N
        mov  _rp_len, #25
        mov  dpl, #0                            ; ST_OK
        ret
    __endasm;
}

/* [dsp cmd][reply delay ms, 0 = no reply][data 1..59] -> [reply frame cmd][reply data] */
static uint8_t cmd_dsp(void)
{
    uint8_t st;
    if (rq_len < 3) {
        return ST_BAD_LEN;
    }
    if (!dsp_link_up) {
        return ST_STATE;
    }
    st = dsp_transact(ARG(0), RX + RQ_DATA + 2, rq_len - 2, ARG(1));
    if (st == DSP_OK && ARG(1)) {
        OUT[0] = dsp_reply_cmd;
        rp_len = 1 + dsp_reply_len;
        /* Loader REBOOT (E 06) acknowledged: the DSP restarts once this reply has been read (dsp_link.c SAFETY). */
        if (ARG(0) == 'E' && ARG(2) == 0x06 && DSP_REPLY[0] == 0x06) {
            dsp_link_down();
        }
    }
    return st;
}

static uint8_t cmd_ee(uint8_t cmd)
{
    uint16_t addr = ((uint16_t)ARG(0) << 8) | ARG(1);
    uint8_t st;

    switch (cmd) {
    case CMD_EE_READ:
        if (rq_len != 3) {
            return ST_BAD_LEN;
        }
        if (ARG(2) == 0 || ARG(2) > RP_MAX) {
            return ST_BAD_ARG;
        }
        st = ee_read(addr, ARG(2));
        if (!st) {
            rp_len = ARG(2);
        }
        return st;
    case CMD_EE_BEGIN:
        return key_ok() ? ee_begin() : ST_BAD_KEY;
    case CMD_EE_WRITE:
        if (rq_len < 3) {
            return ST_BAD_LEN;
        }
        return ee_write(addr, rq_len - 2);
    case CMD_EE_COMMIT:
        if (rq_len != 18) {
            return ST_BAD_LEN;
        }
        return ee_commit();
    default:                                    /* CMD_EE_END */
        ee_end();
        return ST_OK;
    }
}

/* [] -> [mode]; [key 'O' 'C' 30 CF][mode] -> mode record written, then re-enumeration (main.c run_action). */
static uint8_t cmd_mode(void)
{
    uint8_t st;

    if (rq_len) {
        if (rq_len != 5 || !key_ok()) {
            return ST_BAD_KEY;
        }
        if ((uint8_t)(ARG(4) - MODE_CODE_51) >= AUDIO_MODES) {
            return ST_BAD_ARG;
        }
        st = ee_write_mode();
        if (st) {
            return st;
        }
        audio_mode = ARG(4) - MODE_CODE_51;
        protocol_action = ACTION_MODE;
    }
    OUT[0] = audio_mode + MODE_CODE_51;
    rp_len = 1;
    return ST_OK;
}

static uint8_t dispatch(uint8_t cmd)
{
    if ((uint8_t)(cmd - CMD_EE_READ) <= CMD_EE_END - CMD_EE_READ) {
        return cmd_ee(cmd);                     /* 0x20..0x24 */
    }
    switch (cmd) {
    case CMD_INFO:      return cmd_info();
    case CMD_DSP:       return cmd_dsp();
    case CMD_MODE:      return cmd_mode();
    case CMD_REBOOT_DFU:
    case CMD_REBOOT:
        if (!key_ok()) {
            return ST_BAD_KEY;
        }
        protocol_action = cmd;                  /* ACTION_DFU / ACTION_REBOOT are the command codes */
        return ST_OK;
    default:
        return ST_UNKNOWN;
    }
}

void protocol_poll(void)
{
    static uint8_t t0;                          /* static: no push/pop */
    uint8_t st;

    if (!hid_rx_pending) {
        return;
    }
    t0 = (uint8_t)tick_now();
    while (!hid_tx_free() && (uint8_t)((uint8_t)tick_now() - t0) < 50) {   /* previous reply not collected yet */
        main_idle();
    }
    TX[RP_CMD] = RX[RQ_CMD];
    TX[RP_TAG] = RX[RQ_TAG];
    rq_len = RX[RQ_LEN];
    rp_len = 0;
    st = rq_len > RQ_MAX ? ST_BAD_LEN : dispatch(RX[RQ_CMD]);
    TX[RP_STATUS] = st;
    TX[RP_LEN] = rp_len;
    /* Zero-pad the reply, asm (MOVX @R1, P2 = 0xFA) */
    _Static_assert(EP2_IN_BUF == 0xFA20 && RP_DATA == 4 && HID_REPORT_SIZE == 64, "constants of the padding asm");
    __asm
        mov  a, _rp_len
        add  a, #0x24                           ; TX + RP_DATA + rp_len (rp_len <= 60)
        mov  r1, a
        mov  0xA0, #0xFA
        clr  a
    09902$:
        cjne r1, #0x60, 09903$                  ; end of the 64-byte IN buffer 0xFA20..0xFA5F
        sjmp 09904$
    09903$:
        movx @r1, a
        inc  r1
        sjmp 09902$
    09904$:
    __endasm;
    hid_rx_done();
    hid_tx_send();
}
