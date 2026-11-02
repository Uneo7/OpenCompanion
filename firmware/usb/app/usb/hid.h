#ifndef HID_H
#define HID_H

#include <stdint.h>

#include "config.h"

#define HID_RX ((__xdata uint8_t *)EP2_OUT_BUF)     /* last OUT report, valid while hid_rx_pending */
#define HID_TX ((__xdata uint8_t *)EP2_IN_BUF)      /* IN report being built */

extern __bit hid_rx_pending;

void hid_init(void);                       /* (re)arms the endpoints after SET_CONFIGURATION */
void hid_rx_done(void);                    /* releases the OUT buffer: the host may send the next report */
uint8_t hid_tx_free(void);                 /* non-zero: previous IN report collected by the host */
void hid_tx_send(void);                    /* sends HID_TX (64 bytes) */

#endif /* HID_H */
