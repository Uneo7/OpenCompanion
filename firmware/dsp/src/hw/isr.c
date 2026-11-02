/* Interrupt setup and handlers. The vector table (vectors.asm) branches here.
 * INT8 = dMAX transfer completion (audio TCC 5/2, ramp converter TCC 8), INT9 = McASP0 start of frame
 * (dMAX interrupt event 26, ramp calibration timing), INT14 = I2C0 (TAS link). */
#include <c6x.h>
#include "regs.h"
#include "oc.h"
#include "prof.h"
#include "ramp.h"

#ifndef OC_STAGE
#define OC_STAGE 4
#endif

extern void vectors(void);              /* vectors.asm, 1 KB aligned */

/* INT8 has a higher hardware priority than INT14 (I2C). The handler is tiny, but if dMAX misbehaves
 * (interrupt storm) the I2C slave must still be serviced, otherwise it stretches SCL and the whole TAS bus
 * dies. So the I2C slave is also polled from here. */
interrupt void isr_dmax_tcc(void)
{
    PROF_ISR_IN
    uint32_t tcc = dmax_take_tcc();
    if (tcc & TCC_RX_BIT)
        audio_io_on_rx_done();
#if OC_STAGE >= 4
    ramp_on_dma_irq();                  /* TCC 8 (DTCR1), stock 0x1001A484 */
#endif
    i2c_slave_isr();
    PROF_ISR_OUT;
}

/* INT9, enabled by ramp_init() (stock 0x1001A360). */
interrupt void isr_mcasp0_frame(void)
{
    PROF_ISR_IN
    ramp_on_frame_start();
    PROF_ISR_OUT;
}

interrupt void isr_i2c(void)
{
    PROF_ISR_IN
    i2c_slave_isr();
    PROF_ISR_OUT;
}

interrupt void isr_nmi(void)
{
    /* TODO: log the cause (dMAX / EMIF NMI) */
}

void isr_init(void)
{
    ISTP = (unsigned int)vectors;
    ICR = 0xFFFFu;                      /* clear pending interrupts */
    IER |= (1u << 1) | (1u << INT_DMAX_TCC) | (1u << INT_I2C_SPI);   /* NMIE + INT8 + INT14 */
}

void isr_global_enable(void)
{
    CSR |= 1u;                          /* GIE */
}
