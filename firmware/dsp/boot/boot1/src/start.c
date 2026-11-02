#include <c6x.h>
#include "boot.h"
#include "hw.h"

static void quiesce(void)
{
    REG(DMAX_DEDR) = 0xFFFFFFFFu;               /* no dMAX transfer into RAM */
    REG(DMAX_DTCR0) = 0xFFu;
    REG(DMAX_DTCR1) = 0xFFu;
    REG(MCASP0_BASE + MCASP_GBLCTL) = 0;
    REG(MCASP1_BASE + MCASP_GBLCTL) = 0;
    REG(MCASP2_BASE + MCASP_GBLCTL) = 0;
    REG(I2C0_BASE + I2C_MDR) = 0;               /* releases SDA/SCL of the TAS link */
}

/* The image sees the same state as after a ROM boot: I2C0 in reset, McASP0 pins untouched (so the firmware
 * takes its cold-boot path and does the DSPINT handshake itself), interrupts off, PLL running. */
void start_app(uint32_t entry)
{
    volatile uint32_t k;
    REG(I2C0_BASE + I2C_MDR) = 0;
    REG(L1PICR) = 0x80000000u;                  /* invalidate the whole L1P (as the Bose boot driver) */
    for (k = 0; k < 1000u; k++) ;
    ((void (*)(void))entry)();
    for (;;) ;
}

/* Same sequence as the firmware's include/reboot_seq.h (RTI digital watchdog, verified on hardware:
 * hardware.md section 10), copied so that boot1 has no dependency on a changing header. */
void reboot_to_rom(void)
{
    volatile uint32_t k;
    IER = 0;
    quiesce();
    REG(RTIDWDPRLD) = 0x0000000Fu;
    REG(RTIDWDCTRL) = 0xA98559DAu;              /* enable the digital watchdog */
    REG(RTIWDKEY) = 0x0000DEADu;                /* wrong key: reset */
    for (k = 0; k < 4000000u; k++) ;
    REG(L1PICR) = 0x80000000u;                  /* still alive: enter the ROM reset entry */
    for (k = 0; k < 1000u; k++) ;
    ((void (*)(void))0x00000000u)();
    for (;;) ;
}
