#include "config.h"
#include "descriptors.h"
#include "ee_layout.h"
#include "hid.h"
#include "i2c.h"
#include "power.h"
#include "servo.h"
#include "stream.h"
#include "tas1020b.h"
#include "uac.h"
#include "uac_notify.h"
#include "usb_core.h"

#define HID_GET_REPORT  0x01
#define HID_SET_IDLE    0x0A

/* Descriptors are looked up in the data-area desc_tab of the audio mode, one 6-byte entry at a time read into the
 * EP0 IN buffer, which is free while a setup packet is decoded (EP0 NAKs until handle_setup() clears SETUP). */
_Static_assert(USB_EE_BASE == EE_AREA_START, "the core streams the data area from the EEPROM");

_Static_assert(EE_DESC_TAB == EE_ADDR(desc_tab_51) && EE_DESC_TAB == 0x180C && EE_DESC_ENTRY == 6 &&
               EE_ADDR(desc_tab_21) == 0x180C + 43 && EE_LEN(desc_tab_51) == 43, "constants of the asm");
_Static_assert(AUDIO_MODE_51 == 0 && AUDIO_MODE_21 == 1, "desc_tab = 0x180C + audio_mode x 43");
_Static_assert(EP0_IN_BUF == 0xFA18 && (I2C_EEPROM | I2C_SUBADDR) == 0xD0, "constants of the asm");

static __data uint16_t desc_addr;
static __data uint8_t desc_type, desc_index;

/* In assembly for size. SDCC treats a __naked function as a leaf and overlays its parameter with the locals of the
 * I2C helpers: both parameters are copied to statics before the call. */
__code const uint8_t *usb_app_descriptor(uint8_t type, uint8_t index) __naked
{
    (void)type;                                 /* DPL */
    (void)index;                                /* _usb_app_descriptor_PARM_2 */
    __asm
        mov  _desc_type, dpl
        mov  _desc_index, _usb_app_descriptor_PARM_2
        mov  a, _audio_mode                     ; table of the mode: 0x180C + audio_mode x 43
        mov  b, #43
        mul  ab
        add  a, #0x0C
        mov  _desc_addr, a
        mov  (_desc_addr + 1), #0x18
    00001$:
        mov  _i2c_read_PARM_2, _desc_addr       ; one 6-byte entry into the EP0 IN buffer
        mov  (_i2c_read_PARM_2 + 1), (_desc_addr + 1)
        mov  _i2c_read_PARM_3, #0x18            ; EP0_IN_BUF = 0xFA18
        mov  (_i2c_read_PARM_3 + 1), #0xFA
        mov  _i2c_read_PARM_4, #6
        mov  dpl, #0xD0                         ; I2C_EEPROM | I2C_SUBADDR
        lcall _i2c_read
        mov  a, dpl
        jz   00003$                             ; I2C error: stall, the host retries
        mov  dptr, #0xFA18
        movx a, @dptr
        jz   00003$                             ; type 0: end of the table, stall
        cjne a, _desc_type, 00002$
        inc  dptr
        movx a, @dptr
        cjne a, _desc_index, 00002$
        inc  dptr
        movx a, @dptr                           ; length LE16 -> usb_desc_len
        mov  _usb_desc_len, a
        inc  dptr
        movx a, @dptr
        mov  (_usb_desc_len + 1), a
        inc  dptr
        movx a, @dptr                           ; EEPROM address LE16 -> DPTR (returned "code" pointer)
        mov  r7, a
        inc  dptr
        movx a, @dptr
        mov  dph, a
        mov  dpl, r7
        ret
    00002$:
        mov  a, _desc_addr                      ; next entry
        add  a, #6
        mov  _desc_addr, a
        jnc  00001$
        inc  (_desc_addr + 1)
        sjmp 00001$
    00003$:
        mov  dptr, #0x0000
        ret
    __endasm;
}

uint8_t usb_app_setup(void)
{
    if (SETUP_REQTYPE == 0x21 && SETUP_REQUEST == HID_SET_IDLE) {
        usb_ep0_ack();
        return 1;
    }
    return uac_setup();                         /* GET/SET_REPORT on EP0: not supported, use the endpoints */
}

uint8_t usb_app_ep0_out(const __xdata uint8_t *buf, uint8_t len)
{
    return uac_ep0_out(buf, len);
}

void usb_app_configured(uint8_t configuration)
{
    stream_set_alt(0);
    if (configuration) {
        hid_init();
        uac_notify_init();
    }
}

uint8_t usb_app_set_interface(uint8_t ifc, uint8_t alt)
{
    if (ifc == IF_AS) {
        if (alt > ALT_COUNT) {
            return 0;
        }
        stream_set_alt(alt);
        return 1;
    }
    return alt == 0;
}

uint8_t usb_app_get_interface(uint8_t ifc)
{
    return ifc == IF_AS ? stream_alt : 0;
}

void usb_app_event(uint8_t vec)
{
    switch (vec) {
    case VEC_SOF:
        usb_suspended = 0;                      /* a suspend vector serviced after the reset would otherwise hold
                                                 * USB "inactive" */
        if (usb_configuration) {
            servo_sof();
        }
        break;
    case VEC_OUT_EP(EP_HID_VENDOR):
        hid_rx_pending = 1;
        break;
    case VEC_SUSPEND:
        usb_suspended = 1;
        break;
    case VEC_RESUME:
        usb_suspended = 0;
        break;
    case VEC_RESET:
        usb_suspended = 0;
        stream_set_alt(0);
        servo_reset();
        break;
    default:
        break;
    }
}
