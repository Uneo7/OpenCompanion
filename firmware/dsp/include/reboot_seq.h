/* Software reboot of the DA705 back to its ROM boot loader (which reloads the EEPROM image = stock firmware).
 * Shared by the firmware (RST console command) and the RAM loader (LD_REBOOT).
 *
 * 1. Stop everything that could write RAM or hold the I2C bus: interrupts, dMAX, McASPs, I2C0.
 * 2. RTI digital watchdog (SPRS370E table 4-38, 0x42000090..0x420000A0). The C672x RTI derives from the
 *    TMS470/TMS570 RTI: writing 0xA98559DA to RTIDWDCTRL enables the watchdog, a wrong key written to
 *    RTIWDKEY (or the counter reaching zero) resets the DSP. Verified on hardware: after a reboot the RTI
 *    watchdog registers read their reset values (re/dsp/notes/hardware.md section 10).
 * 3. Fallback if no reset happened: jump to the ROM reset entry (0x00000000) with the program cache
 *    invalidated. The ROM boot loader reprograms the PLL and I2C1 itself. */
#ifndef REBOOT_SEQ_H
#define REBOOT_SEQ_H

#include <stdint.h>
#include <c6x.h>

#define RB_REG(a)        (*(volatile uint32_t *)(a))
#define RB_DMAX_DEDR     RB_REG(0x60000010u)
#define RB_DMAX_DTCR0    RB_REG(0x60000080u)
#define RB_DMAX_DTCR1    RB_REG(0x600000A0u)
#define RB_MCASP_GBLCTL(b) RB_REG((b) + 0x44u)
#define RB_I2C0_MDR      RB_REG(0x49000024u)
#define RB_RTIDWDCTRL    RB_REG(0x42000090u)
#define RB_RTIDWDPRLD    RB_REG(0x42000094u)
#define RB_RTIWDKEY      RB_REG(0x4200009Cu)
#define RB_L1PICR        RB_REG(0x20000004u)

static inline void reboot_to_rom(void)
{
    volatile uint32_t k;
    void (*rom_reset)(void) = (void (*)(void))0x00000000u;

    _disable_interrupts();
    IER = 0;
    RB_DMAX_DEDR = 0xFFFFFFFFu;
    RB_DMAX_DTCR0 = 0xFFu;
    RB_DMAX_DTCR1 = 0xFFu;
    RB_MCASP_GBLCTL(0x44000000u) = 0;
    RB_MCASP_GBLCTL(0x45000000u) = 0;
    RB_MCASP_GBLCTL(0x46000000u) = 0;
    RB_I2C0_MDR = 0;                       /* I2C0 in reset: releases SDA/SCL */

    RB_RTIDWDPRLD = 0x0000000Fu;           /* shortest expiration */
    RB_RTIDWDCTRL = 0xA98559DAu;           /* enable the digital watchdog */
    RB_RTIWDKEY = 0x0000DEADu;             /* wrong key: immediate reset if supported */
    for (k = 0; k < 4000000u; k++) ;       /* > 50 ms even at full speed */

    RB_L1PICR = 0x80000000u;               /* still alive: invalidate L1P and enter the ROM */
    for (k = 0; k < 1000u; k++) ;
    rom_reset();
}

#endif
