/* PLL: exact sequence of the Bose boot driver (routine 0x10002774, re/dsp/notes/hardware.md section 1), as in
 * the firmware's src/pll.c (validated on hardware, stage 2, also when the PLL is already running).
 * CLKIN 24.576 MHz (from the TAS) x20 = 491.52 MHz; CPU /2 = 245.76 MHz; SYSCLK2 /4 = 122.88 MHz. */
#include "boot.h"
#include "hw.h"

static void delay(volatile uint32_t n)
{
    while (n--) { }
}

void pll_init(void)
{
    REG(PLLCSR) = 0;            /* bypass */
    delay(4);
    REG(PLLCSR) = 8;            /* PLL reset */
    delay(4);
    REG(PLLDIV0) = 0x8000;
    REG(PLLM)    = 20;
    REG(PLLDIV1) = 0x8001;
    REG(PLLDIV2) = 0x8003;
    REG(PLLDIV3) = 0x8005;
    REG(ALNCTL)  = 7;
    REG(PLLCMD)  = 1;           /* GOSET */
    while (REG(PLLSTAT) & 1u) { }
    delay(3);
    REG(PLLCSR) = 0;            /* out of reset, still bypassed */
    delay(0x1200);              /* lock time */
    REG(PLLCSR) = 1;            /* PLLEN */
    REG(CFGBRIDGE) = 1;         /* SPRS370E table 2-7: pulse CSPRST after a PLL change (stock 0x10002950) */
    REG(CFGBRIDGE) = 0;
}
