#ifndef USB_CONFIG_H
#define USB_CONFIG_H

#include "tas1020b.h"

/* SOF drives the ACG servo. PSOF is not used: the servo counts missed frames from the MCLK capture. */
#define USB_IMASK (USBSTA_RSTR | USBSTA_SUSR | USBSTA_RESR | USBSTA_SOF | USBSTA_SETUP)

/* The core reads addresses >= USB_EE_BASE from the EEPROM instead of code space. */
#define USB_EE_DEV  0x50
#define USB_EE_BASE 0x1800u              /* = EE_AREA_START */

#endif /* USB_CONFIG_H */
