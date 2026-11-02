//! U6002 write order (same rule as `firmware/usb/app/proto/eeprom.c`):
//! 1. header DataType := 3: from here an interruption followed by a power cycle makes the TAS ROM boot its
//!    DFU, never a half-written application;
//! 2. code and data area page by page, each page read back;
//! 3. final header: bytes 0..13, then 15..17, then DataType = 1 alone, last;
//! 4. whole EEPROM read back and compared with the target.

mod data;
mod install;
mod plan;
mod port;
mod state;
mod target;

pub use data::{data_plan, write_data_area, DataOnly, DataReport};
pub use install::{install, TasOptions, TasReport};
pub use plan::{plan, TasPlan};
pub use port::{OcPort, StockPort, TasPort};
pub use state::{classify, TasContent};
pub use target::{updating_header, TasTarget, CODE_START, DATA_AREA, EE_SIZE, HEADER_LEN, OC_CODE_END, PAGE};
