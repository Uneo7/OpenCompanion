/*
 * Polled USB device core for the TAS1020B (DM SLES025B 2.2.7, 6.4, 6.5).
 * While USBSTA.SETUP is set, data/status stages are NAKed; writing VECINT clears it, so EP0 is fully prepared
 * before acknowledging the vector. Data stages start with DATA1, status stages are zero-length DATA1 packets.
 */
#include "tas1020b.h"
#include "usb_config.h"
#include "usb_core.h"
#include "xreg.h"
#ifdef USB_EE_DEV
#include "i2c.h"
#endif

#define NO_ADDRESS 0xFF

__data uint8_t usb_setup[8];
__data uint8_t usb_configuration;
__data uint16_t usb_desc_len;

static __data uint8_t pending_address = NO_ADDRESS;

static __code const uint8_t *in_ptr;
static __data uint16_t in_left;
static __data uint8_t in_active;
static __data uint8_t in_zlp;
static __data uint8_t out_expected;         /* control write data stage pending */

static __code const uint8_t ep0_regs[] = {
    XR(EP_BBAX(OEP_BASE(0)), EP_BUF_REG(EP0_OUT_BUF)),
    XR(EP_BSIZ(OEP_BASE(0)), EP0_SIZE / 8),
    XR(EP_CNF(OEP_BASE(0)), EPCNF_EN | EPCNF_IE),
    XR(EP_DCNTX(OEP_BASE(0)), 0),               /* ready to receive */
    XR(EP_BBAX(IEP_BASE(0)), EP_BUF_REG(EP0_IN_BUF)),
    XR(EP_BSIZ(IEP_BASE(0)), EP0_SIZE / 8),
    XR(EP_CNF(IEP_BASE(0)), EPCNF_EN | EPCNF_IE),
    XR(EP_DCNTX(IEP_BASE(0)), EPDCNT_NACK),     /* nothing to send */
    XR_END
};

static void ep0_init(void)
{
    xreg_load(ep0_regs);
    in_active = 0;
    out_expected = 0;
}

void usb_init(void)
{
    USBFADR = 0;
    usb_configuration = 0;
    pending_address = NO_ADDRESS;
    ep0_init();
    USBIMSK = USB_IMASK;
}

void usb_connect(void)
{
    USBCTL = USBCTL_CONT | USBCTL_FEN;
}

void usb_disconnect(void)
{
    USBCTL &= (uint8_t)~(USBCTL_CONT | USBCTL_FEN);
}

static void ep0_stall(void)
{
    EP_CNF(OEP_BASE(0)) |= EPCNF_STALL;
    EP_CNF(IEP_BASE(0)) |= EPCNF_STALL;
}

void usb_ep0_ack(void)
{
    EP_CNF(IEP_BASE(0)) |= EPCNF_TOGGLE;
    EP_DCNTX(IEP_BASE(0)) = 0;
}

/* Data stage of a control write: the first OUT packet is DATA1 and TOGGLE must match (DM 2.2.7.1.1). */
void usb_ep0_receive(void)
{
    out_expected = 1;
    EP_CNF(OEP_BASE(0)) |= EPCNF_TOGGLE;
    EP_DCNTX(OEP_BASE(0)) = 0;
}

static void ep0_status_out(void)
{
    EP_CNF(OEP_BASE(0)) |= EPCNF_TOGGLE;
    EP_DCNTX(OEP_BASE(0)) = 0;
}

static void ep0_load_next(void)
{
    volatile __xdata uint8_t *buf = (volatile __xdata uint8_t *)EP0_IN_BUF;
    uint8_t n = in_left > EP0_SIZE ? EP0_SIZE : (uint8_t)in_left;
    uint8_t i;
#ifdef USB_EE_DEV
    /* The I2C read cannot interleave with another transfer: i2c.c never yields and usb_poll only runs between
     * transfers. On failure (EEPROM in a write cycle) the request is stalled and the host retries. */
    if (n && (uint8_t)((uint16_t)in_ptr >> 8) >= (uint8_t)(USB_EE_BASE >> 8)) {    /* no 0-byte I2C read */
        if (!i2c_read(USB_EE_DEV | I2C_SUBADDR, (uint16_t)in_ptr, buf, n)) {
            ep0_stall();
            in_active = 0;
            return;
        }
    } else
#endif
    for (i = 0; i < n; i++) {
        buf[i] = in_ptr[i];
    }
    in_ptr += n;
    in_left -= n;
    EP_DCNTX(IEP_BASE(0)) = n;                  /* NACK = 0: armed */
}

static uint16_t setup_wlength(void)
{
    return usb_setup[6] | ((uint16_t)usb_setup[7] << 8);
}

void usb_ep0_send_code(__code const uint8_t *data, uint16_t len)
{
    uint16_t wlength = setup_wlength();
    if (len > wlength) {
        len = wlength;
    }
    in_ptr = data;
    in_left = len;
    in_zlp = (len < wlength) && ((len % EP0_SIZE) == 0);
    in_active = 1;
    EP_CNF(IEP_BASE(0)) |= EPCNF_TOGGLE;        /* first data packet is DATA1 */
    ep0_load_next();
}

void usb_ep0_send_ram(const __data uint8_t *data, uint8_t len)
{
    volatile __xdata uint8_t *buf = (volatile __xdata uint8_t *)EP0_IN_BUF;
    uint8_t i;
    if (usb_setup[7] == 0 && len > usb_setup[6]) {
        len = usb_setup[6];
    }
    for (i = 0; i < len; i++) {
        buf[i] = data[i];
    }
    in_left = 0;
    in_zlp = 0;
    in_active = 1;
    EP_CNF(IEP_BASE(0)) |= EPCNF_TOGGLE;
    EP_DCNTX(IEP_BASE(0)) = len;
}

static uint8_t standard_request(void)
{
    __code const uint8_t *desc;
    uint8_t reply[2] = { 0, 0 };

    switch (SETUP_REQUEST) {
    case REQ_GET_DESCRIPTOR:
        desc = usb_app_descriptor(SETUP_VALUE_H, SETUP_VALUE_L);
        if (!desc) {
            return 0;
        }
        usb_ep0_send_code(desc, usb_desc_len);
        return 1;
    case REQ_SET_ADDRESS:
        pending_address = SETUP_VALUE_L & 0x7F; /* applied after the status stage */
        usb_ep0_ack();
        return 1;
    case REQ_SET_CONFIGURATION:
        usb_configuration = SETUP_VALUE_L;
        usb_app_configured(usb_configuration);
        usb_ep0_ack();
        return 1;
    case REQ_GET_CONFIGURATION:
        reply[0] = usb_configuration;
        usb_ep0_send_ram(reply, 1);
        return 1;
    case REQ_GET_STATUS:
        if ((SETUP_REQTYPE & 0x1F) == 0) {
            reply[0] = 0x01;                    /* self-powered */
        }
        usb_ep0_send_ram(reply, 2);
        return 1;
    case REQ_GET_INTERFACE:
        reply[0] = usb_app_get_interface(SETUP_INDEX_L);
        usb_ep0_send_ram(reply, 1);
        return 1;
    case REQ_SET_INTERFACE:
        if (!usb_app_set_interface(SETUP_INDEX_L, SETUP_VALUE_L)) {
            return 0;
        }
        usb_ep0_ack();
        return 1;
    case REQ_CLEAR_FEATURE:
    case REQ_SET_FEATURE:
        usb_ep0_ack();                          /* accepted and ignored */
        return 1;
    default:
        return 0;
    }
}

static void handle_setup(void)
{
    uint8_t i;
    uint8_t ok;

    for (i = 0; i < 8; i++) {
        usb_setup[i] = SETUP_BUF[i];
    }

    EP_CNF(OEP_BASE(0)) &= (uint8_t)~EPCNF_STALL;
    EP_CNF(IEP_BASE(0)) &= (uint8_t)~EPCNF_STALL;
    in_active = 0;
    out_expected = 0;

    if ((SETUP_REQTYPE & 0x60) == 0x00) {
        ok = standard_request();
    } else if ((SETUP_REQTYPE & 0x80) || (usb_setup[7] == 0 && usb_setup[6] <= EP0_SIZE)) {
        ok = usb_app_setup();                   /* any control read, control writes of one data packet at most */
    } else {
        ok = 0;
    }
    if (!ok) {
        ep0_stall();
    }
    VECINT = 0;                                 /* clears USBSTA.SETUP: EP0 now answers */
}

static void handle_ep0_in(void)
{
    if (pending_address != NO_ADDRESS) {
        USBFADR = pending_address;
        pending_address = NO_ADDRESS;
    }
    if (!in_active) {
        return;
    }
    if (in_left) {
        ep0_load_next();
    } else if (in_zlp) {
        in_zlp = 0;
        EP_DCNTX(IEP_BASE(0)) = 0;
    } else {
        in_active = 0;
        ep0_status_out();
    }
}

static void handle_ep0_out(void)
{
    uint8_t n;
    if (!out_expected) {
        EP_DCNTX(OEP_BASE(0)) = 0;              /* status OUT received */
        return;
    }
    out_expected = 0;
    n = EP_DCNTX(OEP_BASE(0)) & EPDCNT_MASK;
    if (usb_app_ep0_out((const __xdata uint8_t *)EP0_OUT_BUF, n)) {
        usb_ep0_ack();
    } else {
        ep0_stall();
    }
    EP_DCNTX(OEP_BASE(0)) = 0;
}

void usb_poll(void)
{
    uint8_t vec = VECINT;

    switch (vec) {
    case VEC_NONE:
        return;
    case VEC_SETUP:
        handle_setup();                         /* acknowledges VECINT itself */
        return;
    case VEC_IN_EP(0):
        handle_ep0_in();
        break;
    case VEC_OUT_EP(0):
        handle_ep0_out();
        break;
    case VEC_RESET:
        usb_init();
        USBCTL |= USBCTL_FEN;                   /* FEN is cleared by a USB reset (DM 6.5.1.4) */
        usb_app_event(VEC_RESET);
        break;
    default:
        usb_app_event(vec);
        break;
    }
    VECINT = 0;
}
