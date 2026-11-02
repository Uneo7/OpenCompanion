/* Blocking, polled I2C1 master for U7001 (AT24C512, device 0x50, 2-byte addresses). */
#ifndef BOOT_EE_H
#define BOOT_EE_H

#include <stdint.h>

enum { EE_OK = 0, EE_ERR_IO = 2, EE_ERR_RANGE = 1, EE_ERR_VERIFY = 3 };

void ee_init(void);
int  ee_read(uint32_t addr, uint8_t *dst, uint32_t n);        /* any length inside the EEPROM */
int  ee_write(uint32_t addr, const uint8_t *src, uint32_t n); /* inside one page and the write window,
                                                                  waits for the write cycle, reads back */
#endif
