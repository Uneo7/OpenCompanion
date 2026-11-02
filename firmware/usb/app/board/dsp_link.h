#ifndef DSP_LINK_H
#define DSP_LINK_H

#include <stdint.h>

#include "config.h"

/* dsp_transact() results (protocol status codes, docs/protocol_usb_hid.md) */
#define DSP_OK           0x00
#define DSP_ERR_WRITE    0x10            /* request frame not acknowledged */
#define DSP_ERR_READ     0x11            /* stock "ERROR1": reply frame read failed */
#define DSP_ERR_SEQ0     0x12            /* stock "ERROR2": first reply frame has a sequence number other than 0 / FF */
#define DSP_ERR_SEQ      0x13            /* stock "ERROR3": reply frames out of sequence */
#define DSP_ERR_LONG     0x14            /* stock "ERROR4": reply longer than the buffer */

extern __bit dsp_link_up;                /* handshake done, frames are being sent */
extern __bit dsp_link_busy;              /* waiting for DSPINT low (cold boot or DSP restart), stock bit 0x04 */
extern __data uint8_t dsp_reply_len;     /* payload length of the last reply */
extern __data uint8_t dsp_reply_cmd;     /* command byte of the last reply frame */

/* Reply payload of the last dsp_transact() with a delay: inside the vendor HID IN buffer (config.h
 * HID_REP_DATA + 1), so a transaction with a reply may only run while that buffer is free (hid_tx_free()). */
#define DSP_REPLY ((__xdata uint8_t *)(HID_REP_DATA + 1))

/* 'S' status reply of the OpenCompanion DSP (firmware/dsp/include/volume.h, 5 bytes):
 *   [0] standby request, [1] fault (stock, 2-byte reply of the stock DSP), [2] flags, [3] knob 0..99 %,
 *   [4] event counter. The TAS uses [2] and [3] only (board/pod_sync.c). */
#define DSP_S_LEN          5
#define DSP_S_FLAGS        2
#define DSP_S_KNOB         3
#define DSP_S_KNOB_VALID   0x08            /* VS_KNOB_VALID */
#define DSP_S_EXTENDED     0x80            /* VS_EXTENDED: absent from the stock DSP reply */

/* Sends `len` bytes (len >= 1) as frames; if delay_ms != 0, waits then reads the reply (at most 59 bytes) into
 * the vendor HID reply payload, after its first byte (config.h HID_REP_DATA + 1). Returns DSP_OK or DSP_ERR_*. */
uint8_t dsp_transact(uint8_t cmd, const __xdata uint8_t *data, uint8_t len, uint8_t delay_ms);
void dsp_link_start(void);                /* stock InitDspLink, at power-up */
void dsp_link_poll(void);                 /* sends pending 'm', 'V', 'M', 'R'; completes a pending handshake */
void dsp_link_down(void);                 /* DSP restarting: no DSP traffic until DSPINT is low again */

uint8_t dsp_status(void);                 /* 'S' poll: DSP_OK and the reply in DSP_REPLY, or DSP_ERR_* */

#endif /* DSP_LINK_H */
