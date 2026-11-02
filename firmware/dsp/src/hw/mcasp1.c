/* McASP1: TDM audio link with the TAS (USB input) and the codec (AUX input, DAC output).
 * Values copied from the Bose firmware (init 0x10018CD4..0x10018EA0, start 0x10018F0C..0x100193F8):
 * clock slave (BCLK 12.288 MHz and FS 48 kHz from the TAS), 8 slots of 32 bits with 24 valid bits,
 * AXR0 = USB receive, AXR1 = codec ADC receive, AXR2 = DAC transmit. */
#include "regs_mcasp.h"
#include "oc.h"

#define M(off) MCASP_REG(MCASP1_BASE, off)

void mcasp1_init(void)
{
    M(MCASP_GBLCTL)  = 0;              /* everything in reset */
    M(MCASP_PWRDEMU) = 1;              /* free-running under emulation */
    M(MCASP_DLBCTL)  = 0;
    M(MCASP_DITCTL)  = 0;
    M(MCASP_REVTCTL) = 1;              /* DMA events off during configuration */
    M(MCASP_XEVTCTL) = 1;

    /* Receive */
    M(MCASP_RMASK)     = 0xFFFFFF00u;  /* 24 valid bits, left-justified */
    M(MCASP_RFMT)      = 0x000080F0u;  /* MSB first, 32-bit slot, no delay */
    M(MCASP_AFSRCTL)   = 0x00000402u;  /* 8-slot TDM, 1-bit FS (Bose value) */
    M(MCASP_ACLKRCTL)  = 0x000000A0u;  /* Bose value */
    M(MCASP_AHCLKRCTL) = 0;
    M(MCASP_RTDM)      = 0x000000FFu;  /* 8 active slots */
    M(MCASP_RINTCTL)   = 0x00000020u;  /* Bose value (no CPU interrupt used here) */
    M(MCASP_RCLKCHK)   = 0;

    /* Transmit */
    M(MCASP_XMASK)     = 0xFFFFFF00u;
    M(MCASP_XFMT)      = 0x000080F0u;
    M(MCASP_AFSXCTL)   = 0x00000400u;  /* 8-slot TDM, external FS */
    M(MCASP_ACLKXCTL)  = 0x00000080u;  /* external clock, receive synchronous to transmit */
    M(MCASP_AHCLKXCTL) = 0;
    M(MCASP_XTDM)      = 0x000000FFu;
    M(MCASP_XINTCTL)   = 0;
    M(MCASP_XCLKCHK)   = 0;

    /* Serializers */
    M(MCASP_SRCTL(0)) = SRMOD_RX;      /* USB */
    M(MCASP_SRCTL(1)) = SRMOD_RX;      /* codec ADC */
    M(MCASP_SRCTL(2)) = SRMOD_TX;      /* DAC */

    /* Pins: Bose values (0x10018DA4/0x10018DAC): PFUNC = 0x41000038 (AXR1[3..5] and AHCLKR as GPIO;
     * bit 24 is reserved and reads back 0, measured 0x40000038),
     * PDIR = 4 (AXR2 output). */
    M(MCASP_PFUNC) = 0x41000038u;
    M(MCASP_PDIR)  = 0x00000004u;
}

/* Start-up, exactly as the Bose firmware (0x10018F0C..0x100193F8): with external bit clock and frame sync
 * (ACLKRCTL/ACLKXCTL CLKxM = 0, AHCLKR used as GPIO) only the serializer clear, state machine and frame sync
 * bits are released, one bit at a time, each followed by a bounded read-back wait. The clock divider reset bits
 * (RCLKRST/RHCLKRST/XCLKRST/XHCLKRST) are never set: their read-back needs the internal clock dividers and
 * would never complete.
 * start_status: bit k set = step k read back in time (read it with BRR, address in the map). */
static int gbl_set(uint32_t bit)
{
    unsigned k;
    M(MCASP_GBLCTL) |= bit;
    for (k = 0; k < 200000u; k++)
        if (M(MCASP_GBLCTL) & bit) return 1;
    return 0;
}

static uint32_t start_status;


void mcasp1_start(void)
{
    static const uint32_t steps[6] = {
        GBL_RSRCLR, GBL_XSRCLR, GBL_RSMRST, GBL_XSMRST, GBL_RFRST, GBL_XFRST
    };
    int i;
    M(MCASP_RSTAT) = 0xFFFFu;          /* stock clears RSTAT/XSTAT with 0xFFFF (0x10018E90/0x10018E98) */
    M(MCASP_XSTAT) = 0xFFFFu;
    M(MCASP_REVTCTL) = 0;              /* DMA requests on (stock 0x10018E94/0x10018EA0) */
    M(MCASP_XEVTCTL) = 0;
    start_status = 0;
    for (i = 0; i < 6; i++)
        if (gbl_set(steps[i])) start_status |= 1u << i;
}

#if OC_STAGE == 3
/* Bring-up stage 3: McASP running but no DMA requests (dMAX not used). */
void mcasp1_start_no_dma(void)
{
    mcasp1_start();
    M(MCASP_REVTCTL) = 1;
    M(MCASP_XEVTCTL) = 1;
}
#endif

