#ifndef EE_AREA_H
#define EE_AREA_H

#include <stdint.h>

extern __bit ee_area_ok;

/* Call once at boot, before the USB connection (uses the vendor HID IN buffer as scratch). */
void ee_area_check(void);

#endif /* EE_AREA_H */
