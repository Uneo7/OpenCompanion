#ifndef BOARD_H
#define BOARD_H

#include <stdint.h>

void board_init(void);
void delay_ms(uint16_t ms);
void reboot_to_rom(void);   /* back to the boot ROM: with DataType = 0x02 it re-enters DFU */

#endif /* BOARD_H */
