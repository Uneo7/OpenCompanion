#ifndef POD_SYNC_H
#define POD_SYNC_H

#include <stdint.h>

extern __bit pod_knob_ok;                  /* the DSP reports a valid knob position */
extern __data uint8_t pod_knob;            /* knob position, 0..99 % (DSP 'S' byte 3, stock-filtered) */
extern __data uint8_t pod_applied;         /* knob position last applied to the Feature Unit volume */

void pod_sync_poll(void);                  /* main loop only (runs I2C transactions) */

#endif /* POD_SYNC_H */
