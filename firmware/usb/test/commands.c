/*
 * Vendor HID commands of the test firmware (64-byte reports, no report ID).
 *
 * Request : [cmd][addr lo][addr hi][len] ...
 * Reply   : [cmd][addr lo][addr hi][len][data...]   or   [cmd | 0x80][error code]
 *
 *   0x01 READ_CODE   program space through MOVC (0x0000..0x177F = this firmware, 0x8000..0x9FFF = boot ROM)
 *   0x02 READ_XDATA  external data space through MOVX (0xFA10..0xFFFF; reading some registers may clear flags)
 *   0x03 READ_IDATA  internal RAM through indirect addressing (0x00..0xFF)
 *   0x10 INFO        [0x10][version major][version minor][MEMCFG][USBCTL][GLOBCTL] "OC-TAS-TEST"
 *   0x20 RESTORE_HEADER  the only write: puts the EEPROM header back to stock, guarded (restore_header.c)
 *   0x7F REBOOT_ROM  replies, then returns to the boot ROM (DFU mode while the EEPROM header DataType = 0x02)
 */
#include "commands.h"
#include "restore_header.h"
#include "tas1020b.h"
#include "test_usb.h"
#include "usb_core.h"

#define CMD_READ_CODE   0x01
#define CMD_READ_XDATA  0x02
#define CMD_READ_IDATA  0x03
#define CMD_INFO        0x10
#define CMD_REBOOT_ROM  0x7F

#define ERR_BAD_LENGTH  0x01
#define ERR_UNKNOWN     0x02

#define MAX_READ        (EP1_SIZE - 4)
#define VERSION_MAJOR   0
#define VERSION_MINOR   1

__data uint8_t reboot_requested;

static uint8_t error(__xdata uint8_t *out, uint8_t cmd, uint8_t code)
{
    out[0] = cmd | 0x80;
    out[1] = code;
    return 2;
}

uint8_t command_handle(const __xdata uint8_t *in, uint8_t len, __xdata uint8_t *out)
{
    static __code const char tag[] = "OC-TAS-TEST";
    uint8_t cmd = len ? in[0] : 0;
    uint16_t addr;
    uint8_t n, i;

    if (len < 4 && cmd != CMD_INFO && cmd != CMD_REBOOT_ROM && cmd != CMD_RESTORE_HEADER) {
        return error(out, cmd, ERR_BAD_LENGTH);
    }
    addr = in[1] | ((uint16_t)in[2] << 8);
    n = in[3];

    switch (cmd) {
    case CMD_READ_CODE:
    case CMD_READ_XDATA:
    case CMD_READ_IDATA:
        if (n == 0 || n > MAX_READ) {
            return error(out, cmd, ERR_BAD_LENGTH);
        }
        out[0] = cmd;
        out[1] = in[1];
        out[2] = in[2];
        out[3] = n;
        for (i = 0; i < n; i++, addr++) {
            if (cmd == CMD_READ_CODE) {
                out[4 + i] = *(__code const uint8_t *)addr;
            } else if (cmd == CMD_READ_XDATA) {
                out[4 + i] = *(volatile __xdata uint8_t *)addr;
            } else {
                out[4 + i] = *(__idata uint8_t *)(uint8_t)addr;
            }
        }
        return 4 + n;

    case CMD_INFO:
        out[0] = CMD_INFO;
        out[1] = VERSION_MAJOR;
        out[2] = VERSION_MINOR;
        out[3] = MEMCFG;
        out[4] = USBCTL;
        out[5] = GLOBCTL;
        for (i = 0; tag[i]; i++) {
            out[6 + i] = tag[i];
        }
        return 6 + i;

    case CMD_RESTORE_HEADER:
        return restore_header(out);

    case CMD_REBOOT_ROM:
        reboot_requested = 1;                  /* executed by main() once this reply is collected */
        out[0] = CMD_REBOOT_ROM;
        return 1;

    default:
        return error(out, cmd, ERR_UNKNOWN);
    }
}
