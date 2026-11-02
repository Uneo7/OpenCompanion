/* DSPINT handshake (AXR0[9], pin 127 <-> TAS P1.6), same as the firmware's src/mcasp0_gpio.c.
 * The stock TAS InitDspLink (u6002_tas.bin 0x0C60) does SETB P1.6, then loops on JB P1.6 with NO timeout
 * until the DSP pulls the line low; only then does it send X / U / F and start relaying HID commands.
 * Rescue must therefore pull DSPINT low, otherwise the stock TAS never talks to it. */
#include "rescue.h"
#include "hw.h"

#define M0(off) REG(MCASP0_BASE + (off))

void dspint_ready(void)
{
    if (M0(MCASP_PFUNC) == 0x0000FFF0u)
        return;     /* pins already set up (started from a running firmware): TAS link already established */
    M0(MCASP_PFUNC) = 0x0000FFF0u;              /* AXR0[4..15] GPIO (measured stock values) */
    M0(MCASP_PDOUT) = 0x00000050u;              /* DSPINT output value 0 */
    M0(MCASP_PDCLR) = DSPINT_BIT;
    M0(MCASP_PDIR) = 0x80000600u;               /* outputs: AFSR0, AXR0[10], AXR0[9] = DSPINT low */
}

void dspint_release(void)
{
    M0(MCASP_PDIR) &= ~DSPINT_BIT;              /* stock 'U' handler (code 7, control.md) */
}
