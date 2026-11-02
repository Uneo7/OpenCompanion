/*
 * Pod knob and pod mute -> host OS: the OS volume is the only master volume; the pod changes this speaker only and
 * the OS slider follows (Feature Unit CUR + UAC2 interrupt message). Active only while a USB host is active; without
 * one the DSP applies the knob itself (LOCAL mode).
 * Knob: absolute, through the stock pod curve table knob_db; the DSP already filters the value as stock, the TAS
 * adds backlash hysteresis (a reversal must move KNOB_HYST %). SET CUR from the host never touches the knob state:
 * no feedback loop with the OS slider.
 * Mute: every debounced pod mute change is mirrored to the Feature Unit mute, plus the current state once when the
 * host becomes active (the TAS always powers up muted, as stock). Headphones assert the same MUTE line, so plugging
 * them also mutes the OS (accepted).
 */
#include "config.h"
#include "dsp_link.h"
#include "ee_layout.h"
#include "hid.h"
#include "i2c.h"
#include "pod.h"
#include "pod_sync.h"
#include "power.h"
#include "tick.h"
#include "uac.h"
#include "uac_notify.h"

#define POLL_MS          20
#define KNOB_UNMUTE      2      /* knob % that ends a knob mute (entered at 0 %) */
#define KNOB_HYST        3      /* > the 2 % noise span measured on hardware */

__bit pod_knob_ok;
__data uint8_t pod_knob;
__data uint8_t pod_applied;

static __bit last_mute;
static __bit knob_muted;                /* the Feature Unit mute was set by the knob reaching 0 % (0.5.4) */
static __bit knob_up;                   /* initially down: a resting knob moves the volume at most once */
static __data uint8_t t_pass;          /* low byte of the tick: 8-bit elapsed time (POLL_MS < 256) */

/* Constants of the assembly in knob_follow(): knob_db is read into the vendor HID IN buffer after the 'S' reply. */
_Static_assert(EE_ADDR(knob_db) == 0x1862 && 0x62 + 99 < 0x100, "knob_db address in knob_follow()");
_Static_assert(HID_REP_DATA + 1 + DSP_S_LEN == 0xFA2A && (I2C_EEPROM | I2C_SUBADDR) == 0xD0, "knob_follow() asm");

/* Applies when the reading continues the last direction, or when |d| >= KNOB_HYST. On an I2C error nothing
 * changes and the next poll retries. In assembly for code RAM. */
static void knob_follow(void) __naked
{
    __asm
        mov  a, _pod_knob
        clr  c
        subb a, _pod_applied                    ; d
        jz   00004$                             ; no change
        mov  r7, a
        rlc  a                                  ; C = sign of d = down
        jb   _knob_up, 00001$
        jc   00003$                             ; down after down: apply
        sjmp 00002$
    00001$:
        jnc  00003$                             ; up after up: apply
    00002$:
        mov  a, r7                              ; reversal: |d| >= KNOB_HYST ?
        add  a, #(KNOB_HYST - 1)
        add  a, #(0xFF - 2 * (KNOB_HYST - 1))   ; carry when d + KNOB_HYST - 1 > 2 x (KNOB_HYST - 1)
        jnc  00004$
        mov  a, r7
        rlc  a                                  ; C = down
    00003$:
        cpl  c                                  ; C = up
        mov  _knob_up, c
        mov  a, _pod_knob                       ; apply: read knob_db[knob] (EEPROM 0x1862 + knob) into 0xFA2A
        add  a, #0x62
        mov  _i2c_read_PARM_2, a
        mov  (_i2c_read_PARM_2 + 1), #0x18
        mov  _i2c_read_PARM_3, #0x2A
        mov  (_i2c_read_PARM_3 + 1), #0xFA
        mov  _i2c_read_PARM_4, #1
        mov  dpl, #0xD0                         ; I2C_EEPROM | I2C_SUBADDR
        lcall _i2c_read
        mov  a, dpl
        jz   00004$                             ; I2C error: nothing changes, the next poll tries again
        mov  _pod_applied, _pod_knob
        mov  dptr, #0xFA2A
        movx a, @dptr                           ; attenuation, whole dB
        cpl  a
        inc  a
        mov  dph, a                             ; uac_apply_volume(-att x 256)
        mov  dpl, #0
        lcall _uac_apply_volume
        mov  dpl, #NOTIFY_VOLUME                ; uac_notify_post(NOTIFY_VOLUME)
        ljmp _uac_notify_post
    00004$:
        ret
    __endasm;
}

/* Ignored: I2C error, late reply (empty frame), stock DSP (2-byte reply). The poll is also the DSP restart
 * detector (board/dsp_link.c). */
static void mute_report(uint8_t m)
{
    uac_apply_mute(m);
    uac_notify_post(NOTIFY_MUTE);
}

static void read_knob(void)
{
    uint8_t old;

    if (dsp_status() != DSP_OK || dsp_reply_cmd != 'S' || dsp_reply_len < DSP_S_LEN ||
        !(DSP_REPLY[DSP_S_FLAGS] & DSP_S_EXTENDED)) {
        return;
    }
    /* No pod (TAS detection at power-up, stock DetectMuteLine): never trust a knob value, whatever the DSP flags
     * say (0.5.2: an unplugged pod gave a floating 38 % pushed to the OS volume). */
    if (!pod_present || !(DSP_REPLY[DSP_S_FLAGS] & DSP_S_KNOB_VALID)) {
        pod_knob_ok = 0;
        return;
    }
    pod_knob = DSP_REPLY[DSP_S_KNOB];
    if (!pod_knob_ok) {
        pod_knob_ok = 1;
        pod_applied = pod_knob;                 /* start from here: no jump for earlier moves */
        return;
    }
    old = pod_applied;
    knob_follow();
    if (pod_applied == old) {
        return;
    }
    /* Knob mute (0.5.4): reaching 0 % sets the Feature Unit mute (a real mute instead of the -71 dB floor) unless
     * something else already muted; leaving it (>= KNOB_UNMUTE %: in practice the first step up, a reversal needs
     * KNOB_HYST) clears it, only if the knob set it and the pod is not muted. A pod-touch mute or an OS mute is never
     * undone by the knob. Same report path as the pod mute: 'm' to the DSP and the AC interrupt. Only on a change of
     * the applied knob value: an OS unmute while the knob rests at 0 % stays. */
    if (!pod_applied) {
        if (!uac_mute) {
            knob_muted = 1;
            mute_report(1);
        }
    } else if (knob_muted && pod_applied >= KNOB_UNMUTE) {
        knob_muted = 0;
        if (uac_mute && !pod_mute) {
            mute_report(0);
        }
    }
}

void pod_sync_poll(void)
{
    if (!usb_active || !dsp_link_up) {
        pod_knob_ok = 0;
        knob_muted = 0;
        last_mute = !pod_mute;                  /* first active pass reports the current pod mute state once */
        return;
    }
    /* The 'S' reply lands in the vendor HID IN buffer: never while a vendor reply waits for the host. */
    if ((uint8_t)((uint8_t)tick_now() - t_pass) < POLL_MS || !hid_tx_free()) {
        return;
    }
    t_pass = (uint8_t)tick_now();
    if (pod_present && pod_mute != last_mute) { /* no pod: no pod mute (0.5.2) */
        last_mute = pod_mute;
        if (pod_mute != uac_mute) {
            mute_report(pod_mute);
        }
    }
    read_knob();
}
