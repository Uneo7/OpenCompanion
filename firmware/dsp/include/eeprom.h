/* Non-blocking I2C1 master driver for the DSP boot EEPROM U7001 (Atmel AT24C512: 64 KB, 128-byte pages,
 * I2C address 0x50, ~5 ms write cycle).
 *
 * One job at a time (read of any length, or write of up to one page), advanced by eeprom_poll() from the
 * main loop so audio keeps running. A NACK on the device address (EEPROM busy with a previous write
 * cycle) restarts the job: this is the standard "ACK polling".
 *
 * Page policy (boot/docs/LAYOUT.md, RESCUE.md "Changes needed in the DSP firmware" #2). Writes are accepted
 * only inside:
 *   [EE_WRITE_LO, EE_WRITE_HI) = 0xC200..0xDAFF, the profile area of layout v2 (profiles.c), except the eight
 *                                graph data pages of layout v3 (include/bose_tables.h), written by the host only;
 *   [EE_HDR, EE_HDR + 128)     = the image header page, only to invalidate it (UPD, bootctl.c);
 *   [EE_STATE, EE_STATE + 128) = the boot-state page "OCBS" (trial confirmation, bootctl.c).
 * boot0 (0x0000..0x00FF), the image body (0x0200..0xC1FF), boot1 (0xDB00..0xFF7F) and the last page are
 * never written by the firmware. */
#ifndef EEPROM_H
#define EEPROM_H

#include <stdint.h>

#define EE_PAGE       128u
#define EE_WRITE_LO   0xC200u     /* = PROF_AREA_LO (checked in profiles.c) */
#define EE_WRITE_HI   0xDB00u     /* = PROF_AREA_HI = start of boot1 */
#define EE_HDR        0x0100u     /* image header page (boot1/include/layout.h EE_HDR) */
#define EE_STATE      0x0180u     /* boot-state page (EE_STATE) */

enum { EE_IDLE, EE_BUSY, EE_DONE, EE_ERROR };

void eeprom_init(void);
int  eeprom_read(uint16_t addr, uint8_t *dst, uint16_t n);          /* 0 = started, -1 = refused */
int  eeprom_write(uint16_t addr, const uint8_t *src, uint16_t n);   /* one page max, inside the window */
int  eeprom_poll(void);       /* EE_BUSY while running; EE_DONE / EE_ERROR once, then EE_IDLE */
int  eeprom_busy(void);
int  eeprom_read_wait(uint16_t addr, uint8_t *dst, uint16_t n);   /* blocking read (boot only): 1 = OK */

#endif
