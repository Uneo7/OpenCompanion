pub mod meters;
pub mod op;
pub mod options;
pub mod text;

use oc_protocol::dsp::DspConsole;
use oc_protocol::Link;

pub use meters::Meters;
pub use op::{BassSource, BassState, OpInfo, Route21, SlotEntry, PROTO_OPTIONS, PROTO_SUB_DEVICE, PROTO_SUB_LEVEL, SLOTS};
pub use options::{DeviceOptions, StandbyDelay};
pub use text::{BootStatus, PodStatus, ProtectStatus, SysStatus, Versions};

pub struct Dsp<'a> {
    link: &'a dyn Link,
}

impl<'a> Dsp<'a> {
    pub fn new(link: &'a dyn Link) -> Self {
        Self { link }
    }

    fn console(&self) -> DspConsole<'a, dyn Link + 'a> {
        DspConsole::new(self.link)
    }

    /// Text line only: binary commands cannot go through it.
    pub fn line(&self, cmd: &str) -> crate::Result<String> {
        self.text(cmd)
    }
}
