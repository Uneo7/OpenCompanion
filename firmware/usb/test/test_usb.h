#ifndef TEST_USB_H
#define TEST_USB_H

#include <stdint.h>

/* Endpoint buffers after the EP0 buffers (common/usb_core.h). */
#define EP1_IN_BUF     0xFA20u   /* 64 bytes, interrupt IN 0x81 */
#define EP1_OUT_BUF    0xFA60u   /* 64 bytes, interrupt OUT 0x01 */
#define EP1_SIZE       64

/* Called when a full OUT report arrived on EP1; returns the IN report length (0 = none). */
uint8_t command_handle(const __xdata uint8_t *in, uint8_t len, __xdata uint8_t *out);

#endif /* TEST_USB_H */
