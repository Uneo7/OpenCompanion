/* PLL: reproduces the exact sequence of the Bose boot driver (routine 0x10002774, hardware.md section 1).
 * CLKIN = 24.576 MHz (from the TAS) -> PLL x20 = 491.52 MHz; CPU /2 = 245.76 MHz;
 * SYSCLK2 /4 = 122.88 MHz (peripherals); SYSCLK3 /6 = 81.92 MHz. */
#include "regs_pll.h"
#include "oc.h"

static void delay(volatile uint32_t n)
{
    while (n--) { }
}

void pll_init(void)
{
    PLLCSR = 0;                 /* bypass */
    delay(4);
    PLLCSR = PLLCSR_PLLRST;     /* PLL reset */
    delay(4);
    PLLDIV0 = 0x8000;           /* /1 */
    PLLM    = 20;               /* x20 */
    PLLDIV1 = 0x8001;           /* /2 -> CPU */
    PLLDIV2 = 0x8003;           /* /4 -> peripherals */
    PLLDIV3 = 0x8005;           /* /6 */
    ALNCTL  = 7;
    PLLCMD  = 1;                /* GOSET */
    while (PLLSTAT & 1u) { }    /* wait for GOSTAT = 0 */
    delay(3);
    PLLCSR = 0;                 /* out of reset, still bypassed */
    delay(0x1200);              /* lock time */
    PLLCSR = PLLCSR_PLLEN;      /* PLL enabled */
    /* SPRS370E table 2-7: pulse CFGBRIDGE.CSPRST after a PLL change. The stock boot code does the same
     * (writes 1 then 0 at 0x10002950 / 0x10002958); measured stock value afterwards: 0. */
    REG32(0x40000024u) = 1u;
    REG32(0x40000024u) = 0u;
}
