/* Control pod: mute line (P1.3), presence, boot-time DFU escape. */
#ifndef POD_H
#define POD_H

#include <stdint.h>

extern __bit pod_present;                  /* the mute line went high after power-up (stock bit 0x08) */
extern __bit pod_mute;                     /* debounced mute state, 1 = muted */
extern __bit pod_mute_dirty;               /* 'M' to send (dsp_link_poll) */
extern __bit pod_plugged;                  /* pod detected after power-up: 'X' 01 to send (dsp_link_poll) */
extern __bit pod_escape_armed;

/* Debounce of the mute line, expanded inside the 1 ms Timer 1 interrupt (stock DebounceMuteInput 0x13CD, see
 * pod.c): a level must be seen 5 times in a row (count >= 5) and differ from the state to be accepted. */
extern __bit pod_isr_last;
extern __bit pod_isr_state;
extern volatile __bit pod_isr_changed;
extern __data uint8_t pod_isr_count;
#define POD_DEBOUNCE_TICK() do {                               \
    if (P1_3 != pod_isr_last) { pod_isr_count = 0; }           \
    pod_isr_last = P1_3;                                       \
    if (pod_isr_count < 5) { pod_isr_count++; }                \
    if (pod_isr_count >= 5 && pod_isr_last != pod_isr_state) { \
        pod_isr_state = pod_isr_last;                          \
        pod_isr_changed = 1;                                   \
        pod_isr_count = 0;                                     \
    }                                                          \
} while (0)
void pod_detect(void);                     /* stock DetectMuteLine; arms the boot escape if a pod is found */
void pod_poll(void);                       /* non-blocking, called from main_idle() */

#endif /* POD_H */
