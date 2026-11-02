/*
 * Vendor HID interrupt endpoints (DM 2.2.7.2). The OUT report is only flagged here and processed by the main loop,
 * so that long DSP / EEPROM transactions never run inside usb_poll().
 */
#include "config.h"
#include "hid.h"
#include "tas1020b.h"
#include "xreg.h"

#define IEP IEP_BASE(EP_HID_VENDOR)
#define OEP OEP_BASE(EP_HID_VENDOR)

__bit hid_rx_pending;

static __code const uint8_t hid_regs[] = {
    XR(EP_BBAX(IEP), EP_BUF_REG(EP2_IN_BUF)),
    XR(EP_BSIZ(IEP), HID_REPORT_SIZE / 8),
    XR(EP_CNF(IEP), EPCNF_EN | EPCNF_IE),       /* TOGGLE = 0: DATA0 */
    XR(EP_DCNTX(IEP), EPDCNT_NACK),
    XR(EP_BBAX(OEP), EP_BUF_REG(EP2_OUT_BUF)),
    XR(EP_BSIZ(OEP), HID_REPORT_SIZE / 8),
    XR(EP_CNF(OEP), EPCNF_EN | EPCNF_IE),
    XR(EP_DCNTX(OEP), 0),
    XR_END
};

void hid_init(void)
{
    xreg_load(hid_regs);
    hid_rx_pending = 0;
}

void hid_rx_done(void)
{
    hid_rx_pending = 0;
    EP_DCNTX(OEP) = 0;
}

uint8_t hid_tx_free(void)
{
    return EP_DCNTX(IEP) & EPDCNT_NACK;         /* non-zero = free */
}

void hid_tx_send(void)
{
    EP_DCNTX(IEP) = HID_REPORT_SIZE;
}
