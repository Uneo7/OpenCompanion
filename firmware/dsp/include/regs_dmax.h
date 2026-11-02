/* dMAX (SPRU795D section 2.1 "Parameter RAM", section 2.3 "Registers"). */
#ifndef REGS_DMAX_H
#define REGS_DMAX_H

#include "regs.h"

#define DMAX_DEPR   REG32(DMAX_BASE + 0x08)   /* event polarity */
#define DMAX_DEER   REG32(DMAX_BASE + 0x0C)   /* event enable (write 1 to enable) */
#define DMAX_DEDR   REG32(DMAX_BASE + 0x10)   /* event disable */
#define DMAX_DEHPR  REG32(DMAX_BASE + 0x14)   /* high priority (HiMAX) */
#define DMAX_DELPR  REG32(DMAX_BASE + 0x18)   /* low priority (LoMAX) */
#define DMAX_DEFR   REG32(DMAX_BASE + 0x1C)
#define DMAX_DTCR0  REG32(DMAX_BASE + 0x80)   /* transfer complete codes 0..7 (write 1 to clear) */
#define DMAX_DTCR1  REG32(DMAX_BASE + 0xA0)   /* codes 8..15 */

/* Event entry, general purpose transfer (SPRU795D table 2-1) */
#define EV_QTSL(x)   ((uint32_t)(x) << 30)
#define EV_SYNC      (1u << 29)
#define EV_TCC(x)    ((uint32_t)(x) << 24)
#define EV_ATCINT    (1u << 23)
#define EV_TCINT     (1u << 22)
#define EV_RLOAD     (1u << 20)
#define EV_CC(x)     ((uint32_t)(x) << 18)
#define EV_ESIZE_32  (2u << 16)
#define EV_PTE(x)    ((uint32_t)(x) << 8)    /* PaRAM word where the transfer entry starts */
#define EV_ETYPE_GP  3u

/* McASP1 dMAX events (from the Bose firmware: event 6 = transmit, 7 = receive) */
#define DMAX_EVT_MCASP1_TX 6
#define DMAX_EVT_MCASP1_RX 7

/* Transfer entry: 11 words (SPRU795D figure 2-10, CC = 00) */
typedef struct {
    uint32_t src;         /* active source address */
    uint32_t dst;         /* active destination address */
    uint32_t counts;      /* PP | COUNT2(15) | COUNT1(8) | COUNT0(8), active */
    uint32_t idx0;        /* DSTINDX0 << 16 | SRCINDX0 (in elements) */
    uint32_t idx1;
    uint32_t idx2;
    uint32_t ref_counts;  /* reference counters */
    uint32_t src_reload0;
    uint32_t dst_reload0;
    uint32_t src_reload1;
    uint32_t dst_reload1;
} dmax_xfer_t;

#define DMAX_COUNTS(c2, c1, c0) (((uint32_t)(c2) << 16) | ((uint32_t)(c1) << 8) | (uint32_t)(c0))
#define DMAX_IDX(dst, src)      ((((uint32_t)(dst) & 0xFFFFu) << 16) | ((uint32_t)(src) & 0xFFFFu))

#endif
