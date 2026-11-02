use oc_protocol::oc_hid::{OcDevice, OcRelay};
use oc_protocol::transport::Companion;
use oc_protocol::Link;

pub enum Conn {
    Bose(Companion),
    /// OpenCompanion TAS firmware (05A7:1020 with bcdDevice >= 0x0500 since 0.5.0; 0.4.x: 1209:0001 / 0002).
    Oc(OcDevice),
}

impl Conn {
    /// `Ok(None)` when no vendor HID interface is present.
    pub fn open() -> anyhow::Result<Option<Self>> {
        match Companion::open() {
            Ok(c) => return Ok(Some(Self::Bose(c))),
            Err(oc_protocol::Error::NotFound) => {}
            Err(e) => return Err(e.into()),
        }
        match OcDevice::open() {
            Ok(d) => Ok(Some(Self::Oc(d))),
            Err(oc_protocol::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn open_required() -> anyhow::Result<Self> {
        Self::open()?.ok_or_else(|| {
            anyhow::anyhow!(
                "no speaker found on USB (05A7:1020, or 1209:0001 / 1209:0002 for OpenCompanion 0.4.x). On Linux, check the hidraw udev rule \
                 (oc-protocol docs); in ROM DFU mode run `oc install status`."
            )
        })
    }

    pub fn with_dsp<R>(&self, f: impl FnOnce(&dyn Link) -> R) -> R {
        match self {
            Self::Bose(c) => f(c),
            Self::Oc(d) => f(&OcRelay::new(d)),
        }
    }
}
