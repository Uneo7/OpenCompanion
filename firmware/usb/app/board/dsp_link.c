/*
 * DSP link, port of the stock DspTransact (0x0425) / InitDspLink (0x0C60).
 * Frames: 8 bytes on I2C 0x29, [cmd][seq 0, 1... / 0xFF last][len <= 5][5 payload bytes]; unused payload bytes
 * are stale, as the stock TAS sends them. A message without payload sends no frame at all ('S' carries 1 byte).
 * Handshake once the DSP pulls P1.6 low: pod 'X' 01 / no pod 'M' 00 'X' 00, then 'U' 55, 'F' <version>, 50 ms,
 * 'M' <P1.3>. 'R' [rate 24-bit BE Hz][layout: b0 stream open, b1 mode 2.1] is ours (the stock DSP answers ERROR,
 * never read); a restarted DSP only learns rate and layout from it.
 *
 * SAFETY: boot1 (U7001, never rewritten) jumps to garbage if the TAS reads any reply frame from 0x29 during its
 * ~0.1 s boot window (dsp/boot/docs/RESCUE.md). On any DSP restart dsp_link_down() stops ALL DSP traffic and
 * releases DSPINT; the handshake resumes only once the DSP pulls DSPINT low again. Triggers: DSP_DOWN_ERRORS
 * NACKs in a row (0x29 does not ack until boot1's window opens, long after 3 x 20 ms polls), and a relayed
 * loader REBOOT (E 06). Not JUMP (E 03): no boot1 window, and a JUMPed firmware keeps its DSPINT state so it
 * would never re-handshake. DSPINT cannot reveal a restart once the link is up (TAS drives it while USB active).
 */
#include <8051.h>

#include "config.h"
#include "dsp_link.h"
#include "hid.h"
#include "i2c.h"
#include "pod.h"
#include "stream.h"
#include "tas1020b.h"
#include "tick.h"
#include "uac.h"

#define FRAME_PAYLOAD   5
#define DSP_DOWN_ERRORS 3               /* I2C transfers in a row without acknowledge -> DSP restarting */
#define LINK_WAIT_256MS 118             /* ~30 s without DSPINT request -> link up anyway (0.4.9; see dsp_link_poll) */
#define STATUS_REPLY_MS 4               /* stock 20 ms; a late reply is an empty 'S' frame, ignored by pod_sync */

__bit dsp_link_up;
__bit dsp_link_busy;
__data uint8_t dsp_reply_len;
__data uint8_t dsp_reply_cmd;

static __xdata uint8_t frame[8];
static __xdata uint8_t scratch[8];
static __data uint8_t i2c_errors;      /* consecutive I2C failures (link down: wait start, tick / 256 ms) */
static __bit dspint_high;               /* DSPINT seen released since the link went down (request = a new low) */

#define REPLY DSP_REPLY
#define REPLY_MAX 59

/* P1.6 is released before anything reads it (power_poll_usb() may drive it low). While the link is down,
 * i2c_errors holds the start time (tick / 256 ms). */
void dsp_link_down(void)
{
    dsp_link_up = 0;
    dsp_link_busy = 1;
    P1_6 = 1;
    dspint_high = 0;
    i2c_errors = (uint8_t)(tick_now() >> 8);
}

static uint8_t i2c_failed(uint8_t st)
{
    if (++i2c_errors >= DSP_DOWN_ERRORS) {
        dsp_link_down();
    }
    return st;
}

/* Reply = the first read frame whose command is the request's (0.4.10). The stock DSP answers more slowly than the
 * 4 ms 'S' poll read, so its late 'S' reply can still be pending when a relayed 'E' is read: 0.4.9 returned it
 * ("[S] 00 00" for SVM). A first frame of another command is re-read every 10 ms, ~200 ms in all with the 20 ms
 * relay delay, then DSP_ERR_READ: never a reply of another command. The relay itself is atomic (no other frame
 * between a request and its reply: only the main loop sends frames, main_idle() never does).
 * In assembly for code size (~120 bytes). frame is accessed with MOVX @R0 and P2 = its page: the XDATA variables
 * start at 0xFEC8 so frame never crosses a page, and P2 is free (common/xreg.c).
 * nooverlay: SDCC treats an asm-body function as a leaf and would overlay its parameters with the locals of the
 * I2C helpers it calls. */
_Static_assert(REPLY_MAX == 59 && DSP_ERR_WRITE == 0x10 && DSP_ERR_READ == 0x11 && DSP_ERR_SEQ0 == 0x12 &&
               DSP_ERR_SEQ == 0x13 && DSP_ERR_LONG == 0x14 && FRAME_PAYLOAD == 5 && I2C_DSP == 0x29 &&
               (uint16_t)DSP_REPLY == 0xFA25, "constants of dsp_transact()");
static __data uint8_t tx_cmd, tx_seq, tx_try;

#pragma save
#pragma nooverlay
uint8_t dsp_transact(uint8_t cmd, const __xdata uint8_t *data, uint8_t len, uint8_t delay) __naked
{
    (void)cmd; (void)data; (void)len; (void)delay;
    __asm
        mov  _tx_cmd, dpl
        mov  _tx_seq, #0
    00001$:                                     ; while (len)
        mov  a, _dsp_transact_PARM_3
        jz   00005$
        mov  r7, #5
        clr  c
        subb a, r7
        jnc  00002$                             ; len >= 5: n = 5, len -= 5
        mov  r7, _dsp_transact_PARM_3           ; n = len, len = 0
        clr  a
    00002$:
        mov  _dsp_transact_PARM_3, a
        mov  0xA0, #>_frame
        mov  r0, #<_frame
        mov  a, _tx_cmd
        movx @r0, a                             ; frame[0] = cmd
        inc  r0
        mov  a, _dsp_transact_PARM_3
        jz   00003$
        mov  a, _tx_seq                         ; more frames follow: seq++
        inc  _tx_seq
        sjmp 00004$
    00003$:
        mov  a, #0xFF                           ; last frame
    00004$:
        movx @r0, a                             ; frame[1]
        inc  r0
        mov  a, r7
        movx @r0, a                             ; frame[2] = n
        mov  dpl, _dsp_transact_PARM_2
        mov  dph, (_dsp_transact_PARM_2 + 1)
    00006$:                                     ; frame[3..] = data[0..n) (n >= 1)
        inc  r0
        movx a, @dptr
        inc  dptr
        movx @r0, a
        djnz r7, 00006$
        mov  _dsp_transact_PARM_2, dpl          ; data += n
        mov  (_dsp_transact_PARM_2 + 1), dph
        clr  a                                  ; i2c_write(I2C_DSP, 0, frame, 8)
        mov  _i2c_write_PARM_2, a
        mov  (_i2c_write_PARM_2 + 1), a
        mov  _i2c_write_PARM_3, #_frame
        mov  (_i2c_write_PARM_3 + 1), #(_frame >> 8)
        mov  _i2c_write_PARM_4, #8
        mov  dpl, #0x29
        lcall _i2c_write
        mov  a, dpl
        jnz  00001$
        mov  dpl, #0x10                         ; DSP_ERR_WRITE
        ljmp _i2c_failed
    00005$:
        clr  a
        mov  _i2c_errors, a
        mov  _dsp_reply_len, a
        mov  a, _dsp_transact_PARM_4
        jnz  00007$
        mov  dpl, a                             ; no reply: DSP_OK
        ret
    00007$:
        mov  dpl, a
        mov  dph, #0
        lcall _delay_ms
        mov  _tx_seq, #0
        mov  _tx_try, #18                       ; 18 x 10 ms re-reads
    00008$:                                     ; i2c_read(I2C_DSP, 0, frame, 8)
        clr  a
        mov  _i2c_read_PARM_2, a
        mov  (_i2c_read_PARM_2 + 1), a
        mov  _i2c_read_PARM_3, #_frame
        mov  (_i2c_read_PARM_3 + 1), #(_frame >> 8)
        mov  _i2c_read_PARM_4, #8
        mov  dpl, #0x29
        lcall _i2c_read
        mov  a, dpl
        jnz  00009$
        mov  dpl, #0x11                         ; DSP_ERR_READ
        ljmp _i2c_failed
    00018$:                                     ; not our reply: re-read every 10 ms
        mov  a, _tx_seq
        jnz  00019$                             ; inside a multi-frame reply: give up
        djnz _tx_try, 00020$
    00019$:
        mov  dpl, #0x11                         ; DSP_ERR_READ
        ret
    00020$:
        mov  dptr, #10
        lcall _delay_ms
        sjmp 00008$
    00009$:
        mov  0xA0, #>_frame
        mov  r0, #<_frame
        movx a, @r0
        mov  _dsp_reply_cmd, a                  ; frame[0]
        cjne a, _tx_cmd, 00018$                 ; the reply of another command
        inc  r0
        movx a, @r0
        mov  r6, a                              ; last = frame[1]
        cjne a, #0xFF, 00010$
        sjmp 00011$
    00010$:
        cjne a, _tx_seq, 00015$                 ; neither 0xFF nor the frame index
    00011$:
        inc  r0
        movx a, @r0                             ; n = min(frame[2], 5)
        mov  r7, a
        add  a, #(0xFF - 5)
        jnc  00012$
        mov  r7, #5
    00012$:
        mov  a, r7
        add  a, _dsp_reply_len
        mov  r5, a                              ; new length
        add  a, #(0xFF - 59)
        jc   00017$                             ; > REPLY_MAX
        mov  a, #0x25                           ; DPTR = DSP_REPLY + dsp_reply_len
        add  a, _dsp_reply_len
        mov  dpl, a
        clr  a
        addc a, #0xFA
        mov  dph, a
        mov  a, r7
        jz   00014$
    00013$:
        inc  r0
        movx a, @r0
        movx @dptr, a
        inc  dptr
        djnz r7, 00013$
    00014$:
        mov  _dsp_reply_len, r5
        inc  _tx_seq
        cjne r6, #0xFF, 00008$
        mov  dpl, #0                            ; DSP_OK
        ret
    00015$:
        mov  a, _tx_seq                         ; DSP_ERR_SEQ0 (first frame) / DSP_ERR_SEQ
        mov  dpl, #0x12
        jz   00016$
        inc  dpl
    00016$:
        ret
    00017$:
        mov  dpl, #0x14                         ; DSP_ERR_LONG
        ret
    __endasm;
}
#pragma restore

static void send(uint8_t cmd, uint8_t len)
{
    dsp_transact(cmd, scratch, len, 0);
}

static void send1(uint8_t cmd, uint8_t value)
{
    scratch[0] = value;
    send(cmd, 1);
}

static __code const char fw_version[8] = { FW_VERSION_STRING8 };

/* E 00 probe: rescue / updater answer 06 'O' 'C' 'L'. The reply lands in the vendor HID IN buffer: the caller
 * checks hid_tx_free(). */
_Static_assert(DSP_OK == 0 && (uint16_t)DSP_REPLY == 0xFA25, "constants of dsp_is_loader()");
static uint8_t dsp_is_loader(void) __naked
{
    __asm
        mov  dptr, #_scratch                    ; scratch[0] = 0
        clr  a
        movx @dptr, a
        mov  _dsp_transact_PARM_2, #_scratch
        mov  (_dsp_transact_PARM_2 + 1), #(_scratch >> 8)
        mov  _dsp_transact_PARM_3, #1
        mov  _dsp_transact_PARM_4, #20           ; reply after 20 ms (stock delay of E)
        mov  dpl, #0x45                          ; 'E'
        lcall _dsp_transact
        mov  a, dpl
        jnz  00001$                              ; I2C / frame error
        mov  a, _dsp_reply_len
        add  a, #0xFC
        jnc  00001$                              ; fewer than 4 bytes
        mov  dptr, #0xFA25                       ; DSP_REPLY: 06 'O' 'C' 'L'
        movx a, @dptr
        cjne a, #0x06, 00001$
        inc  dptr
        movx a, @dptr
        cjne a, #0x4F, 00001$
        inc  dptr
        movx a, @dptr
        cjne a, #0x43, 00001$
        inc  dptr
        movx a, @dptr
        cjne a, #0x4C, 00001$
        mov  dpl, #1
        ret
    00001$:
        mov  dpl, #0
        ret
    __endasm;
}

static void link_finish(void)
{
    uint8_t loader;

    dsp_link_busy = 0;                          /* set again by dsp_link_down() if the DSP does not acknowledge */
    loader = dsp_is_loader();
    if (dsp_link_busy) {
        return;                                 /* no acknowledge: link down again, wait again */
    }
    if (loader) {
        /* No handshake frames to a loader: a 'U' would release its DSPINT and nothing takes it back. */
        dsp_link_up = 1;
        return;
    }

    if (pod_present) {
        send1('X', 1);
    } else {
        send1('M', 0);
        send1('X', 0);
    }
    send1('U', 'U');
    /* scratch = fw_version in asm for size; P2 = page of scratch (never crosses a page, P2 free). */
    __asm
        mov  0xA0, #>_scratch
        mov  r0, #<_scratch
        mov  dptr, #_fw_version
        mov  r7, #0
    09901$:
        mov  a, r7
        movc a, @a+dptr
        movx @r0, a
        inc  r0
        inc  r7
        cjne r7, #8, 09901$
    __endasm;
    send('F', 8);
    if (dsp_link_busy) {
        return;                                 /* the frames were not acknowledged: link down again, wait again */
    }
    dsp_link_up = 1;
    delay_ms(50);
    pod_mute = P1_3;
    send1('M', pod_mute);                       /* stock 0x0CEE: raw P1.3 */
    pod_mute_dirty = 0;
    uac_rate_dirty = 1;                         /* 'R' again: the DSP only learns the rate from it (dsp/README) */
    uac_volume_dirty = 1;                       /* and the OS volume / mute: a restarted DSP has lost them */
    uac_mute_dirty = 1;
}

void dsp_link_start(void)
{
    dsp_link_down();
    pod_detect();
}

void dsp_link_poll(void)
{
    if (!dsp_link_up) {
        /* Edge, not level: a loader that never got 'U' keeps DSPINT low and would re-trigger during its reboot.
         * Timeout: a DSP that never pulls DSPINT (JUMP warm start) would otherwise stay in LOCAL mode with USB
         * muted. Relayed messages are atomic: dsp_transact() never returns to the main loop mid-message. */
        if (P1_6) {
            dspint_high = 1;
        }
        if (dsp_link_busy && hid_tx_free() &&
            ((dspint_high && !P1_6) || (uint8_t)((uint8_t)(tick_now() >> 8) - i2c_errors) >= LINK_WAIT_256MS)) {
            i2c_errors = 0;
            link_finish();
        }
        return;
    }
    if (uac_mute_dirty) {
        uac_mute_dirty = 0;
        send1('m', uac_mute);
    }
    if (uac_volume_dirty) {
        uac_volume_dirty = 0;
        scratch[0] = (uint8_t)((uint16_t)uac_volume >> 8);
        scratch[1] = (uint8_t)uac_volume;
        send('V', 2);
    }
    if (pod_plugged) {
        pod_plugged = 0;
        send1('X', 1);                          /* pod plugged after power-up (board/pod.c) */
    }
    if (pod_mute_dirty) {
        pod_mute_dirty = 0;
        send1('M', pod_mute);
    }
    if (uac_rate_dirty) {
        uac_rate_dirty = 0;
        scratch[0] = 0;
        scratch[1] = 0xBB;                      /* 48000 = 0x00BB80, the only rate since 0.5.1 */
        scratch[2] = 0x80;
        scratch[3] = stream_alt | (uint8_t)(audio_mode << 1);  /* layout: b0 streaming, b1 mode 2.1 */
        send('R', 4);
    }
}

uint8_t dsp_status(void)
{
    scratch[0] = 0;
    return dsp_transact('S', scratch, 1, STATUS_REPLY_MS);
}

