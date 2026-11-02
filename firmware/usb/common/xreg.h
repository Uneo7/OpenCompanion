/* Table-driven writes to the TAS1020B XDATA registers 0xFF00..0xFFFF (code size: 2 bytes per write instead of 6). */
#ifndef XREG_H
#define XREG_H

#include <stdint.h>

/* One table entry: register (any XREG(0xFFxx) lvalue of tas1020b.h) and value. A table ends with XR_END. */
#define XR(reg, value)  (uint8_t)(uint16_t)&(reg), (uint8_t)(value)
#define XR_END          0x00             /* 0xFF00 is not a register */

/* Writes the [register low byte, value] pairs in order, up to XR_END. Leaves P2 = 0xFF (see xreg.c). */
void xreg_load(__code const uint8_t *table);

#endif /* XREG_H */
