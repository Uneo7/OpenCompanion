/* C672x PLL controller (base 0x41000000). Offsets taken from the Bose boot driver
 * (routine 0x10002774, re/dsp/notes/hardware.md section 1), consistent with the TI PLL controller. */
#ifndef REGS_PLL_H
#define REGS_PLL_H

#include "regs.h"

#define PLLCSR   REG32(PLL_BASE + 0x100)   /* bit0 PLLEN, bit3 PLLRST */
#define PLLM     REG32(PLL_BASE + 0x110)
#define PLLDIV0  REG32(PLL_BASE + 0x114)   /* bit15 = DxEN, bits 4..0 = ratio - 1 */
#define PLLDIV1  REG32(PLL_BASE + 0x118)
#define PLLDIV2  REG32(PLL_BASE + 0x11C)
#define PLLDIV3  REG32(PLL_BASE + 0x120)
#define PLLCMD   REG32(PLL_BASE + 0x138)   /* bit0 GOSET */
#define PLLSTAT  REG32(PLL_BASE + 0x13C)   /* bit0 GOSTAT */
#define ALNCTL   REG32(PLL_BASE + 0x140)

#define PLLCSR_PLLEN   (1u << 0)
#define PLLCSR_PLLRST  (1u << 3)

#endif
