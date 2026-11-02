/* DA705 / C672x registers used by boot1 (SPRS370E memory map, SPRU175D I2C, SPRU041J McASP).
 * Self-contained on purpose: boot1 is written once and must not depend on headers that keep changing. */
#ifndef BOOT_HW_H
#define BOOT_HW_H

#include <stdint.h>

#define REG(a)          (*(volatile uint32_t *)(a))

/* I2C modules: I2C0 = TAS link (slave 0x29), I2C1 = U7001 (master, device 0x50). */
#define I2C0_BASE       0x49000000u
#define I2C1_BASE       0x4A000000u
#define I2C_OAR         0x00u
#define I2C_IMR         0x04u
#define I2C_STR         0x08u
#define I2C_CLKL        0x0Cu
#define I2C_CLKH        0x10u
#define I2C_CNT         0x14u
#define I2C_DRR         0x18u
#define I2C_SAR         0x1Cu
#define I2C_DXR         0x20u
#define I2C_MDR         0x24u
#define I2C_EMDR        0x2Cu
#define I2C_PSC         0x30u

#define STR_AL          (1u << 0)
#define STR_NACK        (1u << 1)
#define STR_ARDY        (1u << 2)
#define STR_RRDY        (1u << 3)
#define STR_XRDY        (1u << 4)
#define STR_SCD         (1u << 5)
#define STR_AAS         (1u << 9)
#define STR_BB          (1u << 12)

#define MDR_IRS         (1u << 5)
#define MDR_TRX         (1u << 9)
#define MDR_MST         (1u << 10)
#define MDR_STP         (1u << 11)
#define MDR_STT         (1u << 13)
#define MDR_FREE        (1u << 14)

/* McASP0 pins used as GPIO (measured stock values, re/dsp/notes/hardware.md section 10). */
#define MCASP0_BASE     0x44000000u
#define MCASP1_BASE     0x45000000u
#define MCASP2_BASE     0x46000000u
#define MCASP_PFUNC     0x10u
#define MCASP_PDIR      0x14u
#define MCASP_PDOUT     0x18u
#define MCASP_PDCLR     0x20u
#define MCASP_GBLCTL    0x44u
#define DSPINT_BIT      (1u << 9)   /* AXR0[9] = DSPINT, pin 127 (uncertainties.md #11) */

/* dMAX (SPRU795D) */
#define DMAX_DEDR       0x60000010u
#define DMAX_DTCR0      0x60000080u
#define DMAX_DTCR1      0x600000A0u

/* PLL controller (Bose boot driver 0x10002774, hardware.md section 1) */
#define PLL_BASE        0x41000000u
#define PLLCSR          (PLL_BASE + 0x100u)
#define PLLM            (PLL_BASE + 0x110u)
#define PLLDIV0         (PLL_BASE + 0x114u)
#define PLLDIV1         (PLL_BASE + 0x118u)
#define PLLDIV2         (PLL_BASE + 0x11Cu)
#define PLLDIV3         (PLL_BASE + 0x120u)
#define PLLCMD          (PLL_BASE + 0x138u)
#define PLLSTAT         (PLL_BASE + 0x13Cu)
#define ALNCTL          (PLL_BASE + 0x140u)
#define CFGBRIDGE       0x40000024u

/* RTI digital watchdog (reboot, see firmware include/reboot_seq.h) and program cache. */
#define RTIDWDCTRL      0x42000090u
#define RTIDWDPRLD      0x42000094u
#define RTIWDKEY        0x4200009Cu
#define L1PICR          0x20000004u

#endif
