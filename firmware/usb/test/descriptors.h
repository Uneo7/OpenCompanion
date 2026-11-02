#ifndef DESCRIPTORS_H
#define DESCRIPTORS_H

#include <stdint.h>

/* Test identity: pid.codes "Test PID" (VID 0x1209, PID 0x0001), never used for released firmware. */
#define USB_VID 0x1209
#define USB_PID 0x0001

extern __code const uint8_t device_descriptor[18];
extern __code const uint8_t config_descriptor[41];
extern __code const uint8_t hid_report_descriptor[25];

/* Returns a pointer to string descriptor `index`, or 0 if it does not exist. */
__code const uint8_t *string_descriptor(uint8_t index);

#endif /* DESCRIPTORS_H */
