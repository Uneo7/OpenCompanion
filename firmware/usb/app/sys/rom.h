#ifndef ROM_H
#define ROM_H

/* Re-enter the ROM DFU in RAM mode (mode 1): no EEPROM write. Announces DFU_VID:DFU_PID. Never returns. */
void rom_enter_dfu(void);
/* Full reboot through the ROM API "reboot" (BOOT_RebootToRom 0x9F41): the ROM reads the EEPROM header again
 * (DataType 1 -> EEPROM application, 2 -> DFU). Never returns. */
void rom_reboot(void);

#endif /* ROM_H */
