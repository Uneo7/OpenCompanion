#ifndef UAC_NOTIFY_H
#define UAC_NOTIFY_H

#include <stdint.h>

/* Feature Unit controls that can be reported (bit = 1 << control selector) */
#define NOTIFY_MUTE    0x02         /* FU_MUTE_CONTROL 0x01 */
#define NOTIFY_VOLUME  0x04         /* FU_VOLUME_CONTROL 0x02 */

/* Host reaction time in 256 ms units (INFO bytes 23..24): [0] last message armed, [1] last FU request. */
extern __data uint8_t usb_t[2];

void uac_notify_init(void);         /* after SET_CONFIGURATION: endpoint armed, nothing pending */
void uac_notify_post(uint8_t what); /* queue "CUR changed" for NOTIFY_MUTE / NOTIFY_VOLUME */
void uac_notify_poll(void);         /* from main_idle(): sends one queued message when the endpoint is free */

#endif /* UAC_NOTIFY_H */
