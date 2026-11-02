#ifndef UAC_H
#define UAC_H

#include <stdint.h>

extern __data int16_t uac_volume;          /* Feature Unit volume CUR, 1/256 dB, sent to the DSP with 'V' */
extern __bit uac_mute;                     /* Feature Unit mute CUR, sent to the DSP with 'm' */
extern __bit uac_volume_dirty;             /* set on change, cleared by dsp_link_poll */
extern __bit uac_mute_dirty;
extern __bit uac_rate_dirty;

uint8_t uac_setup(void);                   /* class request to the AudioControl interface; 0 = stall */
uint8_t uac_ep0_out(const __xdata uint8_t *buf, uint8_t len);

/* Sets CUR (clamped to the range) and flags the DSP update if it changed. Used by SET CUR and by the pod
 * (board/pod_sync.c, which then queues the interrupt data message, usb/uac_notify.c; SET CUR never does). */
void uac_apply_volume(int16_t v);
void uac_apply_mute(uint8_t m);

#endif /* UAC_H */
