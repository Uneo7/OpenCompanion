/* USB descriptors of the test firmware: one vendor-defined HID interface, 64-byte reports. */
#include "descriptors.h"

__code const uint8_t device_descriptor[18] = {
    18, 0x01,               /* bLength, DEVICE */
    0x10, 0x01,             /* bcdUSB 1.10 */
    0x00, 0x00, 0x00,       /* class defined at interface level */
    8,                      /* bMaxPacketSize0 */
    USB_VID & 0xFF, USB_VID >> 8,
    USB_PID & 0xFF, USB_PID >> 8,
    0x01, 0x00,             /* bcdDevice 0.01 */
    1, 2, 3,                /* manufacturer, product, serial strings */
    1                       /* one configuration */
};

__code const uint8_t hid_report_descriptor[25] = {
    0x06, 0x00, 0xFF,       /* Usage Page (vendor 0xFF00) */
    0x09, 0x01,             /* Usage 1 */
    0xA1, 0x01,             /* Collection (Application) */
    0x15, 0x00,             /*   Logical Minimum 0 */
    0x26, 0xFF, 0x00,       /*   Logical Maximum 255 */
    0x75, 0x08,             /*   Report Size 8 */
    0x95, 0x40,             /*   Report Count 64 */
    0x09, 0x01, 0x81, 0x02, /*   Input (data, var, abs) */
    0x09, 0x01, 0x91, 0x02, /*   Output (data, var, abs) */
    0xC0                    /* End Collection */
};

__code const uint8_t config_descriptor[41] = {
    9, 0x02, 41, 0,         /* CONFIGURATION, wTotalLength 41 */
    1, 1, 0,                /* 1 interface, value 1, no string */
    0xC0, 50,               /* self-powered, 100 mA */

    9, 0x04, 0, 0, 2,       /* INTERFACE 0, alt 0, 2 endpoints */
    0x03, 0x00, 0x00, 0,    /* HID, no subclass, no protocol, no string */

    9, 0x21, 0x11, 0x01,    /* HID 1.11 */
    0x00, 1, 0x22,          /* no country, 1 descriptor: report */
    sizeof(hid_report_descriptor), 0,

    7, 0x05, 0x81, 0x03, 64, 0, 1,   /* EP1 IN, interrupt, 64 bytes, 1 ms */
    7, 0x05, 0x01, 0x03, 64, 0, 1    /* EP1 OUT, interrupt, 64 bytes, 1 ms */
};

static __code const uint8_t str_langid[] = { 4, 0x03, 0x09, 0x04 };
static __code const uint8_t str_manufacturer[] = {
    28, 0x03, 'O', 0, 'p', 0, 'e', 0, 'n', 0, 'C', 0, 'o', 0, 'm', 0, 'p', 0, 'a', 0, 'n', 0, 'i', 0, 'o', 0, 'n', 0
};
static __code const uint8_t str_product[] = {
    22, 0x03, 'T', 0, 'A', 0, 'S', 0, ' ', 0, 't', 0, 'e', 0, 's', 0, 't', 0, ' ', 0, 'F', 0
};
static __code const uint8_t str_serial[] = { 10, 0x03, '0', 0, '0', 0, '0', 0, '1', 0 };

__code const uint8_t *string_descriptor(uint8_t index)
{
    switch (index) {
    case 0: return str_langid;
    case 1: return str_manufacturer;
    case 2: return str_product;
    case 3: return str_serial;
    default: return 0;
    }
}
