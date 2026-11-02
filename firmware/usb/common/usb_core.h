/*
 * Minimal polled USB device core for the TAS1020B (no ROM USB engine, no interrupts), shared by the
 * OpenCompanion firmware (app/) and the test firmware (test/).
 *
 * The core handles endpoint 0: setup decoding, standard requests, control read/write data and status
 * stages (DM SLES025B 2.2.7.1). Everything device-specific goes through the usb_app_* callbacks, which
 * each target implements. Each target also provides "usb_config.h" (USB_IMASK: USBIMSK bits to enable).
 */
#ifndef USB_CORE_H
#define USB_CORE_H

#include <stdint.h>

#include "usb_config.h"             /* target options (USB_IMASK, USB_EE_DEV) */

/* Endpoint 0 buffers: start of the endpoint buffer space (DM 6.4.2). The ROM scratch area 0xFA10..0xFA63 is
 * free because neither firmware uses the ROM USB engine. */
#define EP0_OUT_BUF    0xFA10u
#define EP0_IN_BUF     0xFA18u
#define EP0_SIZE       8

/* Standard request codes (USB 2.0 table 9-4) */
#define REQ_GET_STATUS        0x00
#define REQ_CLEAR_FEATURE     0x01
#define REQ_SET_FEATURE       0x03
#define REQ_SET_ADDRESS       0x05
#define REQ_GET_DESCRIPTOR    0x06
#define REQ_GET_CONFIGURATION 0x08
#define REQ_SET_CONFIGURATION 0x09
#define REQ_GET_INTERFACE     0x0A
#define REQ_SET_INTERFACE     0x0B

#define DESC_DEVICE      0x01
#define DESC_CONFIG      0x02
#define DESC_STRING      0x03
#define DESC_HID         0x21
#define DESC_HID_REPORT  0x22

/* Copy of the current setup packet: bmRequestType, bRequest, wValue (L, H), wIndex (L, H), wLength (L, H) */
extern __data uint8_t usb_setup[8];
#define SETUP_REQTYPE  usb_setup[0]
#define SETUP_REQUEST  usb_setup[1]
#define SETUP_VALUE_L  usb_setup[2]
#define SETUP_VALUE_H  usb_setup[3]
#define SETUP_INDEX_L  usb_setup[4]
#define SETUP_INDEX_H  usb_setup[5]

extern __data uint8_t usb_configuration;    /* bConfigurationValue, 0 = not configured */
extern __data uint16_t usb_desc_len;        /* set by usb_app_descriptor() */

void usb_init(void);
void usb_connect(void);
void usb_disconnect(void);
void usb_poll(void);                        /* services at most one pending vector */

/* Replies available to usb_app_setup() */
void usb_ep0_send_code(__code const uint8_t *data, uint16_t len);   /* control read from code space */
/* With USB_EE_DEV (usb_config.h), a "code" address >= USB_EE_BASE is an EEPROM address (I2C device USB_EE_DEV):
 * usb_ep0_send_code() then streams the data from the EEPROM, 8 bytes per EP0 packet. The code space of the
 * TAS1020B ends at 0x177F (program RAM, DM 6.1), so the two ranges cannot be confused. */
void usb_ep0_send_ram(const __data uint8_t *data, uint8_t len);     /* control read, len <= 8 */
void usb_ep0_ack(void);                                             /* control write without data: status */
void usb_ep0_receive(void);                                         /* control write: wait for <= 8 data bytes */

/* --- Callbacks implemented by each target --- */
/* Returns the descriptor (and sets usb_desc_len), or 0 to stall. wIndex is in usb_setup. */
__code const uint8_t *usb_app_descriptor(uint8_t type, uint8_t index);
/* Class and vendor requests: reply with one of the usb_ep0_* functions and return 1, or return 0 to stall. */
uint8_t usb_app_setup(void);
/* Data stage of a control write started with usb_ep0_receive(); return 0 to stall the status stage. */
uint8_t usb_app_ep0_out(const __xdata uint8_t *buf, uint8_t len);
void usb_app_configured(uint8_t configuration);
uint8_t usb_app_set_interface(uint8_t ifc, uint8_t alt);            /* 0 = stall */
uint8_t usb_app_get_interface(uint8_t ifc);
/* Any other vector (endpoints 1..7, SOF, suspend, resume, and reset after the core re-initialised EP0). */
void usb_app_event(uint8_t vec);

#endif /* USB_CORE_H */
