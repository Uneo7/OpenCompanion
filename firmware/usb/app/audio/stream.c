/*
 * Audio path, configured like the stock firmware (InitClockRegs 0x1338, InitAcg 0x11AB, InitEndpointRegs 0x0E91).
 * ACG2/MCLKO2 = fixed 24.576 MHz DSP clock, never servoed. ACG1/MCLKO1 = codec-port master clock, the only clock
 * locked to USB (24.576 MHz at 48 kHz, 22.5792 MHz at 44.1 kHz). TAS is codec-port clock master, DIVB = 2:
 * 8 slots x 32 bits per frame; slots 0..5 = L R C LFE Ls Rs. DSP McASP1 is slave.
 *
 * Formats, one per audio mode (alt 1 only):
 *   mode  USB subslot   frame  BPTS  DMATSL0  BPTSL  OEPCNF1
 *   5.1   2 B, 16 bit   12 B   2     0x3F     16     0xCB     = stock
 *   2.1   4 B, 24 bit    8 B   4     0x03     32     0xC7
 * The DSP word is the same in both: sample left-justified in the 32-bit slot (BYOR).
 * BPTSL is writable only with CPTEN = 0 (DM 6.5.4): a mode change briefly stops the codec port.
 *
 * Never use an ISO buffer >= 1024 bytes (bit 7 of OEPBSIZ): on hardware it broke the DMA count (1056 bytes ->
 * DMABCNT0 = 36). Stock 960 bytes fits 12-byte frames at 48 kHz (576 + 355 + 12 = 943).
 * Never use 3-byte subslots: on hardware Windows 24-bit audio landed one byte off in the slot (-48 dB).
 * [I] Codec (U4000) master clock assumed to be MCLKO1 (single clock domain).
 */
#include "config.h"
#include "servo.h"
#include "stream.h"
#include "tas1020b.h"
#include "uac.h"
#include "xreg.h"

__data uint8_t stream_alt;
__data uint8_t stream_rate;
__data uint8_t stream_slot_mask;
__data uint8_t audio_mode;

#define CPTCNF1_STOCK   0x38
#define CPTCNF2_SLOT32  0x05            /* TSL0L = 0, TSLL = 32 CSCLK; BPTSL added per format */
#define CPTCNF3_STOCK   0x34
#define CPTCNF4_STOCK   0x61
#define ISO_CNF(bytes)  (uint8_t)(EPCNF_EN | EPCNF_ISO | ((bytes) - 1))   /* BPS = frame bytes - 1 */
#define ISO_EP          OEP_BASE(EP_ISO_OUT)

/* OEPCNF1 comes last, in alt_regs */
static __code const uint8_t iso_regs[] = {
    XR(EP_BBAX(ISO_EP), EP_BUF_REG(ISO_BUF)),
    XR(EP_BSIZ(ISO_EP), ISO_BUF_SIZE / 8),
    XR(EP_DCNTX(ISO_EP), 0),
    XR(EP_DCNTY(ISO_EP), 0),
    XR(EP_BBAY(ISO_EP), 0),
    XR_END
};

#define ALT_REGS_LEN    7
static __code const uint8_t alt_regs[AUDIO_MODES][ALT_REGS_LEN] = {
    { XR(DMATSH0, DMATSH_BPTS(2)), XR(DMATSL0, 0x3F), XR(EP_CNF(ISO_EP), ISO_CNF(12)), XR_END }, /* 5.1: stock */
    { XR(DMATSH0, DMATSH_BPTS(4)), XR(DMATSL0, 0x03), XR(EP_CNF(ISO_EP), ISO_CNF(8)), XR_END },  /* 2.1: L R */
};


/* servo_init() writes ACG1FRQ between the two tables (stock order). */
static __code const uint8_t clock_regs_1[] = {
    XR(ACG1DCTL, 0),
    XR(ACGCTL, ACGCTL_MCLKO1EN | ACGCTL_DIVEN),
    XR_END
};

static __code const uint8_t clock_regs_2[] = {
    XR(ACG2DCTL, 0),
    XR(ACGCTL, ACGCTL_MCLKO1EN | ACGCTL_DIVEN | ACGCTL_MCLKO2EN | ACGCTL_MCLKO2S1),   /* stock: |= 0x82 */
    XR(ACG2FRQ2, 0x61),
    XR(ACG2FRQ1, 0xA8),
    XR(ACG2FRQ0, 0x00),                         /* LSB last: loads the synthesizer (DM 2.2.6.1) */
    XR(DMACTL0, EP_ISO_OUT),                    /* DMA off */
    /* Codec port (written with CPTEN = 0) */
    XR(CPTCNF1, CPTCNF1_STOCK),
    XR(CPTCNF2, CPTCNF2_SLOT32 | CPTCNF2_BPTSL_32),
    XR(CPTCNF3, CPTCNF3_STOCK),
    XR(CPTCNF4, CPTCNF4_STOCK),
    XR_END
};

/* 48 kHz only (0.5.1); stream_rate stays 0 (IDATA clear of sys/crt0.asm). */
void stream_init(void)
{
    GLOBCTL &= (uint8_t)~GLOBCTL_CPTEN;         /* codec port configuration is writable only with CPTEN = 0 */
    xreg_load(clock_regs_1);
    servo_init();
    xreg_load(clock_regs_2);
    GLOBCTL |= GLOBCTL_CPTEN;
}

void stream_dma(uint8_t on)
{
    DMACTL0 = on ? (DMACTL_DMAEN | EP_ISO_OUT) : EP_ISO_OUT;
}

void stream_set_alt(uint8_t alt)
{
    uint8_t c2;

    stream_dma(0);
    EP_CNF(ISO_EP) = 0;
    stream_alt = alt;
    stream_slot_mask = 0;
    uac_rate_dirty = 1;                         /* 'R' carries the layout to the DSP */
    if (!alt) {
        return;
    }
    _Static_assert(CPTCNF2_BPTSL_32 - CPTCNF2_BPTSL_16 == 0x20 && AUDIO_MODE_21 == 1, "c2 below");
    c2 = (CPTCNF2_SLOT32 | CPTCNF2_BPTSL_16) + (uint8_t)(audio_mode << 5);   /* BPTSL 16 (5.1) or 32 (2.1) */
    if (CPTCNF2 != c2) {                        /* BPTSL changed with the mode: C-port off, DM 6.5.4 */
        GLOBCTL &= (uint8_t)~GLOBCTL_CPTEN;
        CPTCNF2 = c2;
        GLOBCTL |= GLOBCTL_CPTEN;
    }
    xreg_load(iso_regs);
    xreg_load(alt_regs[0] + (uint8_t)(audio_mode * ALT_REGS_LEN));   /* 8-bit index: no int multiply */
    stream_slot_mask = (uint8_t)(audio_mode << 1) + 1;   /* bytes per slot - 1: 1 (5.1, 2 bytes), 3 (2.1, 4) */
    servo_reset();
    stream_dma(1);
}
