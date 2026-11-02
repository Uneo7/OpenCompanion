/* McASP0 pins used as GPIO (re/dsp/notes/uncertainties.md items 7, 8, 9, 11; hardware.md section 6).
 * Values measured on the stock firmware (notes/stock_regs.json): PFUNC = 0x0000FFF0 (AXR0[4..15] GPIO),
 * PDOUT = 0x00000050, PDIR = 0x80000600 (outputs: AFSR0 = DISCHG, AXR0[10], AXR0[9] = DSPINT). That capture
 * was taken while AXR0[14] was released; with the stock firmware playing, PDIR = 0x80004600 (AXR0[14] driven).
 * DSPINT handshake: the TAS (InitDspLink) raises P1.6 and waits for the line to go low; the stock main
 * (0x10019D08) does PDCLR = 1<<9 and PDIR |= 1<<9 ("DSP ready"), and the 'U' message (stock code 7)
 * releases the pin again (PDIR &= ~(1<<9), control.md). With the RAM loader this setting is inherited from
 * the stock firmware; a flashed OpenCompanion image must do it itself.
 * AXR0[14]: speaker amplifier control (stock 0x100157E8 / 0x10015910, src/control/spk_mode.c). Stock drives it
 * low (PDCLR = 0x4000, then PDIR |= 0x4000) to let the speakers play and releases it (PDIR &= ~0x4000) for
 * the pod mute / headphones, the silence standby and the supply fault. Its PDOUT bit is never set by stock. */
#include "oc.h"
#include <c6x.h>
#include "regs_mcasp.h"

#define M0(off)     MCASP_REG(MCASP0_BASE, off)
#define DSPINT_BIT  (1u << 9)
#define AMP_BIT     (1u << 14)

/* Returns 1 on a cold boot (McASP0 at reset values), 0 when the pins were already set up by the stock
 * firmware (RAM loader): then the DSPINT and AXR0[14] states are kept, the TAS has already done its
 * handshake and the amplifiers stay as the stock firmware left them until spk_mode.c takes over. */
int mcasp0_gpio_init(void)
{
    uint32_t keep = 0;
    int cold = M0(MCASP_PFUNC) != 0x0000FFF0u;
    if (!cold) keep = M0(MCASP_PDIR) & (DSPINT_BIT | AMP_BIT);
    M0(MCASP_PFUNC) = 0x0000FFF0u;
    M0(MCASP_PDOUT) = 0x00000050u;                         /* DSPINT and AXR0[14] output values = 0 */
    M0(MCASP_PDIR)  = (0x80000600u & ~DSPINT_BIT) | keep;  /* DSPINT driven only once the link is ready */
    return cold;
}

/* PDIR is also read-modified-written by the ramp converter interrupts (calibration bit 6, ramp.c):
 * keep these updates atomic. */
void mcasp0_dspint_ready(void)
{
    unsigned csr = _disable_interrupts();
    M0(MCASP_PDCLR) = DSPINT_BIT;               /* drive low = "DSP ready" */
    M0(MCASP_PDIR) |= DSPINT_BIT;
    _restore_interrupts(csr);
}

void mcasp0_dspint_release(void)
{
    unsigned csr = _disable_interrupts();
    M0(MCASP_PDIR) &= ~DSPINT_BIT;
    _restore_interrupts(csr);
}

/* Stock 0x100158A8..0x100158C8: PDCLR = 0x4000 first, then PDIR |= 0x4000 (no high glitch). */
void mcasp0_amp_drive_low(void)
{
    unsigned csr = _disable_interrupts();
    M0(MCASP_PDCLR) = AMP_BIT;
    M0(MCASP_PDIR) |= AMP_BIT;
    _restore_interrupts(csr);
}

/* Stock 0x10015844, 0x100158F0, 0x10015964: PDIR &= ~0x4000 (PDOUT untouched). */
void mcasp0_amp_release(void)
{
    unsigned csr = _disable_interrupts();
    M0(MCASP_PDIR) &= ~AMP_BIT;
    _restore_interrupts(csr);
}
