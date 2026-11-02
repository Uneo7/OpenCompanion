#include "descriptors.h"
#include "tas1020b.h"
#include "test_usb.h"
#include "usb_core.h"

#define HID_SET_IDLE     0x0A
#define HID_DESC_OFFSET  18     /* HID descriptor inside the configuration descriptor */

static void ep1_init(void)
{
    EP_BBAX(IEP_BASE(1)) = EP_BUF_REG(EP1_IN_BUF);
    EP_BSIZ(IEP_BASE(1)) = EP1_SIZE / 8;
    EP_CNF(IEP_BASE(1)) = EPCNF_EN | EPCNF_IE;  /* TOGGLE = 0: DATA0 */
    EP_DCNTX(IEP_BASE(1)) = EPDCNT_NACK;

    EP_BBAX(OEP_BASE(1)) = EP_BUF_REG(EP1_OUT_BUF);
    EP_BSIZ(OEP_BASE(1)) = EP1_SIZE / 8;
    EP_CNF(OEP_BASE(1)) = EPCNF_EN | EPCNF_IE;
    EP_DCNTX(OEP_BASE(1)) = 0;
}

__code const uint8_t *usb_app_descriptor(uint8_t type, uint8_t index)
{
    __code const uint8_t *s;
    switch (type) {
    case DESC_DEVICE:
        usb_desc_len = sizeof(device_descriptor);
        return device_descriptor;
    case DESC_CONFIG:
        usb_desc_len = sizeof(config_descriptor);
        return config_descriptor;
    case DESC_STRING:
        s = string_descriptor(index);
        if (s) {
            usb_desc_len = s[0];
        }
        return s;
    case DESC_HID:
        usb_desc_len = 9;
        return config_descriptor + HID_DESC_OFFSET;
    case DESC_HID_REPORT:
        usb_desc_len = sizeof(hid_report_descriptor);
        return hid_report_descriptor;
    default:
        return 0;
    }
}

uint8_t usb_app_setup(void)
{
    if (SETUP_REQTYPE == 0x21 && SETUP_REQUEST == HID_SET_IDLE) {
        usb_ep0_ack();
        return 1;
    }
    return 0;                                   /* GET_REPORT / SET_REPORT: use the interrupt endpoints */
}

uint8_t usb_app_ep0_out(const __xdata uint8_t *buf, uint8_t len)
{
    (void)buf;
    (void)len;
    return 0;
}

void usb_app_configured(uint8_t configuration)
{
    if (configuration) {
        ep1_init();
    }
}

uint8_t usb_app_set_interface(uint8_t ifc, uint8_t alt)
{
    (void)ifc;
    (void)alt;
    return 1;
}

uint8_t usb_app_get_interface(uint8_t ifc)
{
    (void)ifc;
    return 0;
}

static void handle_ep1_out(void)
{
    __xdata uint8_t *rx = (__xdata uint8_t *)EP1_OUT_BUF;
    __xdata uint8_t *tx = (__xdata uint8_t *)EP1_IN_BUF;
    uint8_t n = EP_DCNTX(OEP_BASE(1)) & EPDCNT_MASK;
    uint16_t guard = 0;

    /* The IN buffer is free once the UBM set its NACK bit (previous report collected by the host). */
    while (!(EP_DCNTX(IEP_BASE(1)) & EPDCNT_NACK) && ++guard) {
    }
    n = command_handle(rx, n, tx);
    if (n) {
        /* Input reports are fixed at 64 bytes (report descriptor): pad the reply. */
        while (n < EP1_SIZE) {
            tx[n++] = 0;
        }
        EP_DCNTX(IEP_BASE(1)) = EP1_SIZE;
    }
    EP_DCNTX(OEP_BASE(1)) = 0;                  /* accept the next OUT report */
}

void usb_app_event(uint8_t vec)
{
    if (vec == VEC_OUT_EP(1)) {
        handle_ep1_out();
    }
}
