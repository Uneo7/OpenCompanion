/* McASP (SPRU041J, "Registers" chapter, configuration-bus register table). */
#ifndef REGS_MCASP_H
#define REGS_MCASP_H

#include "regs.h"

#define MCASP_PWRDEMU   0x004
#define MCASP_PFUNC     0x010
#define MCASP_PDIR      0x014
#define MCASP_PDOUT     0x018
#define MCASP_PDIN      0x01C   /* read; PDSET on write */
#define MCASP_PDCLR     0x020
#define MCASP_GBLCTL    0x044
#define MCASP_AMUTE     0x048
#define MCASP_DLBCTL    0x04C
#define MCASP_DITCTL    0x050
#define MCASP_RGBLCTL   0x060
#define MCASP_RMASK     0x064
#define MCASP_RFMT      0x068
#define MCASP_AFSRCTL   0x06C
#define MCASP_ACLKRCTL  0x070
#define MCASP_AHCLKRCTL 0x074
#define MCASP_RTDM      0x078
#define MCASP_RINTCTL   0x07C
#define MCASP_RSTAT     0x080
#define MCASP_RCLKCHK   0x088
#define MCASP_REVTCTL   0x08C
#define MCASP_XGBLCTL   0x0A0
#define MCASP_XMASK     0x0A4
#define MCASP_XFMT      0x0A8
#define MCASP_AFSXCTL   0x0AC
#define MCASP_ACLKXCTL  0x0B0
#define MCASP_AHCLKXCTL 0x0B4
#define MCASP_XTDM      0x0B8
#define MCASP_XINTCTL   0x0BC
#define MCASP_XSTAT     0x0C0
#define MCASP_XCLKCHK   0x0C8
#define MCASP_XEVTCTL   0x0CC
#define MCASP_SRCTL(n)  (0x180 + 4 * (n))
#define MCASP_XBUF(n)   (0x200 + 4 * (n))
#define MCASP_RBUF(n)   (0x280 + 4 * (n))

#define MCASP_REG(base, off) REG32((base) + (off))

/* GBLCTL ("Global Control Register"): receive bits 0..4, transmit bits 8..12 */
#define GBL_RCLKRST   (1u << 0)
#define GBL_RHCLKRST  (1u << 1)
#define GBL_RSRCLR    (1u << 2)
#define GBL_RSMRST    (1u << 3)
#define GBL_RFRST     (1u << 4)
#define GBL_XCLKRST   (1u << 8)
#define GBL_XHCLKRST  (1u << 9)
#define GBL_XSRCLR    (1u << 10)
#define GBL_XSMRST    (1u << 11)
#define GBL_XFRST     (1u << 12)

/* SRCTL SRMOD: 0 = inactive, 1 = transmit, 2 = receive */
#define SRMOD_INACTIVE 0u
#define SRMOD_TX       1u
#define SRMOD_RX       2u

#endif
