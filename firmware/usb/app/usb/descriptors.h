/* USB descriptor constants. The descriptors themselves are in the U6002 data area (data/ee_data.c). */
#ifndef DESCRIPTORS_H
#define DESCRIPTORS_H

#include <stdint.h>

/* One configuration descriptor per audio mode (audio/stream.h): 9 + IAD 8 + AC interface 9 + AC class-specific
 * + interrupt EP 7 + AS alt 0 9 + AS alt 1 46 + HID 32 */
#define AC_CS_TOTAL_51        80                               /* AC header 9 + clock 8 + IT 17 + FU 34 + OT 12 */
#define AC_CS_TOTAL_21        64                               /* AC header 9 + clock 8 + IT 17 + FU 18 + OT 12 */
#define CONFIG_51_LEN         (120 + AC_CS_TOTAL_51)           /* 200 */
#define CONFIG_21_LEN         (120 + AC_CS_TOTAL_21)           /* 184 */
#define HID_DESC_OFF_51       (CONFIG_51_LEN - 32 + 9)         /* HID class descriptor: HID last in mode 5.1 */
#define HID_DESC_OFF_21       (9 + 9)                          /* HID first in mode 2.1 (interface 0) */
#define AC_INT_INTERVAL       4                                /* ms, EP 0x83 bInterval */
#define AC_INT_MSG_LEN        6                                /* ADC-2 6.1: always 6 bytes */

/* wMaxPacketSize = 49 frames: 48 kHz + 1 frame (FMT-2 2.3.1.1). */
#define CH_51          6          /* mode 5.1: L R C LFE Ls Rs, as the stock IT (0x059F: 6 channels, 0x003F) */
#define CH_20          2          /* mode 2.1: L R (the DSP derives the subwoofer) */
#define CFG_51         0x0000003Fu   /* FL FR FC LFE BL BR (ADC-2 4.1, bmChannelConfig) = stock wChannelConfig */
#define CFG_20         0x00000003u   /* FL FR */
#define ISO_MAXPKT_51  (49 * CH_51 * 2)   /* 588: 16-bit in 2-byte subslots (stock: 576, 48 frames) */
#define ISO_MAXPKT_20  (49 * CH_20 * 4)   /* 392: 24-bit in 4-byte subslots (audio/stream.c: why not 3) */

/* 1/256 dB (ADC-2 5.2.5.7.2). MAX - MIN must be a whole number of RES steps (usbaudio2.sys). */
#define VOL_MIN     ((int16_t)0x8100)     /* -127 dB */
#define VOL_MAX     ((int16_t)0x0000)     /*    0 dB */
#define VOL_RES     0x0100                /*    1 dB */

#define HID_VENDOR_REP_LEN    25

#endif /* DESCRIPTORS_H */
