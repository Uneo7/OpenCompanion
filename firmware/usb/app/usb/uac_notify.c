/*
 * UAC2 AudioControl interrupt endpoint (EP 0x83 IN). Message (ADC-2 6.1, 6 bytes): FU 3 volume 00 01 00 02 00 03,
 * mute 00 01 00 01 00 03; the host answers with GET CUR. The endpoint must NAK when nothing is pending, so it is
 * armed only while a message is queued. One pending bit per control: at most one message per control in flight.
 */
#include "config.h"
#include "stream.h"
#include "descriptors.h"
#include "tas1020b.h"
#include "tick.h"
#include "uac_notify.h"
#include "xreg.h"

#define IEP  IEP_BASE(EP_AC_INT)
#define BUF  ((volatile __xdata uint8_t *)AC_INT_BUF)

static __data uint8_t notify_pending;
__data uint8_t usb_t[2];                        /* tick / 256: [0] last message armed, [1] last FU request (INFO) */

static __code const uint8_t notify_regs[] = {
    XR(EP_BBAX(IEP), EP_BUF_REG(AC_INT_BUF)),
    XR(EP_BSIZ(IEP), 1),                        /* 8 bytes */
    XR(EP_CNF(IEP), EPCNF_EN),                  /* TOGGLE = 0: DATA0; IE = 0 */
    XR(EP_DCNTX(IEP), EPDCNT_NACK),             /* nothing to send: NAK */
    XR_END
};

void uac_notify_init(void)
{
    xreg_load(notify_regs);
    notify_pending = 0;
}

void uac_notify_post(uint8_t what)
{
    notify_pending |= what;
}

void uac_notify_poll(void)
{
    uint8_t cs;

    if (!notify_pending || !(EP_DCNTX(IEP) & EPDCNT_NACK)) {
        return;                                 /* nothing queued, or the previous message is not collected */
    }
    cs = 1;                                     /* FU_MUTE_CONTROL */
    if (notify_pending & NOTIFY_VOLUME) {
        cs = 2;                                 /* FU_VOLUME_CONTROL first */
        notify_pending &= (uint8_t)~NOTIFY_VOLUME;
    } else {
        notify_pending = 0;
    }
    BUF[0] = 0x00;                              /* bInfo: class-specific, interface */
    BUF[1] = 0x01;                              /* bAttribute: CUR */
    BUF[2] = 0x00;                              /* CN 0 (master) */
    BUF[3] = cs;                                /* CS */
    BUF[4] = IF_AC;
    BUF[5] = UAC_ID_FU;
    EP_DCNTX(IEP) = AC_INT_MSG_LEN;
    usb_t[0] = (uint8_t)(tick_now() >> 8);
}
