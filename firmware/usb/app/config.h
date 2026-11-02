#ifndef CONFIG_H
#define CONFIG_H

/* USB identity (0.5.0, user decision 2026-11-02): the stock Bose VID:PID 05A7:1020, in both audio modes.
 * OpenCompanion is told apart from the stock firmware (same VID:PID) by bcdDevice (stock 0x0100, ours >= 0x0500) and
 * the strings (stock "Bose Corporation" / "Bose USB Audio", ours "OpenCompanion" / "Companion 5 (OpenCompanion)"),
 * README "USB identity". 0.4.x used the pid.codes test IDs 1209:0001 / 1209:0002 (the host tools still accept
 * them). Mode 2.1 keeps a distinct Windows audio device node through its interface numbers (HID 0, audio 1..2;
 * mode 5.1: audio 0..1, HID 2 as stock), not through a PID: README "Audio mode". Bump bcdDevice on every
 * descriptor change: Windows caches descriptors and driver binding on VID/PID/bcdDevice. */
#define USB_VID              0x05A7
#define USB_PID              0x1020
#define USB_BCD_DEVICE       0x0504

#define FW_VERSION_MAJOR     0
#define FW_VERSION_MINOR     5
#define FW_VERSION_PATCH     4
/* Exactly 8 characters, like the stock "01.01.03" (0x15CA). */
#define FW_VERSION_STRING8   'O', 'C', '-', '0', '.', '5', '.', '4'
#define PROTOCOL_VERSION     4             /* docs/protocol_usb_hid.md */

/* Identity announced by the ROM DFU after "reboot to DFU" (header copy at XDATA 0xFBF0); 05A7:1020 = stock Bose. */
#define DFU_VID              0x05A7
#define DFU_PID              0x1020

/* --- Endpoint buffer space: 1304 bytes, 0xFA10..0xFF27 (DM SLES025B 6.4.2) -------------------------------
 *   0xFA10   8  EP0 OUT                       (common/usb_core.h)
 *   0xFA18   8  EP0 IN
 *   0xFA20  64  EP2 IN   vendor HID (0x82)
 *   0xFA60  64  EP2 OUT  vendor HID (0x02)
 *   0xFAA0   8  EP3 IN   UAC2 AudioControl interrupt (0x83), 6-byte interrupt data message (usb/uac_notify.c)
 *   0xFAA8  960 EP1 OUT  isochronous audio, single circular buffer (DM 2.2.7.4.1), same size as stock
 *   0xFE68   96  free
 *   0xFEC8   96  XDATA variables (linker: --xram-loc 0xFEC8 --xram-size 96)
 * The whole space is used; nothing is left to the ROM USB engine. */
#define EP2_IN_BUF           0xFA20u
#define EP2_OUT_BUF          0xFA60u
#define AC_INT_BUF           0xFAA0u
#define ISO_BUF              0xFAA8u
/* Must stay below 1024 bytes (BSIZ < 0x80): 1056 broke the DMA (audio/stream.c). 960 = stock. */
#define ISO_BUF_SIZE         960u
#define HID_REPORT_SIZE      64
#define HID_REQ_DATA         (EP2_OUT_BUF + 3)   /* request payload (docs/protocol_usb_hid.md) */
#define HID_REP_DATA         (EP2_IN_BUF + 4)    /* reply payload, 60 bytes */

/* Interface numbers per audio mode (0.5.0): 5.1 = AC 0, AS 1, HID 2 (as stock); 2.1 = HID 0, AC 1, AS 2, so that
 * Windows binds the 2.1 audio function to another child device node (MI_01) with its own mix format. At run time:
 * audio/stream.h IF_AC / IF_AS (= audio_mode, audio_mode + 1). */
#define IF_AC_51             0
#define IF_AS_51             1
#define IF_HID_51            2
#define IF_HID_21            0
#define IF_AC_21             1
#define IF_AS_21             2
#define USB_NUM_INTERFACES   3
#define EP_ISO_OUT           1
#define EP_HID_VENDOR        2
#define EP_AC_INT            3             /* UAC2 AudioControl interrupt IN (ADC-2 4.8.2.1) */

/* IT 1, FU 3, OT 4 as in the stock descriptor at 0x059F */
#define UAC_ID_IT            1
#define UAC_ID_FU            3
#define UAC_ID_OT            4
#define UAC_ID_CLOCK         5

#define I2C_DSP              0x29          /* DSP I2C0 slave (stock DspTransact 0x0425) */
#define I2C_EEPROM           0x50          /* U6002 AT24C64 (stock ProcessHidOutReport 't'/'T') */

#endif /* CONFIG_H */
