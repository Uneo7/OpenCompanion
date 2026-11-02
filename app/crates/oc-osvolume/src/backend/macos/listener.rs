use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::controls::Controls;
use super::ffi::*;
use super::props;
use crate::{Callback, Result};

struct Context {
    device: AudioObjectID,
    controls: Controls,
    cb: Callback,
}

unsafe extern "C" fn on_change(
    _object: AudioObjectID,
    _count: u32,
    _addresses: *const AudioObjectPropertyAddress,
    data: *mut c_void,
) -> OSStatus {
    // SAFETY: `data` is the `Context` owned by a live `Subscription`; it is
    // freed only after every listener has been removed.
    let ctx = &*(data as *const Context);
    // Never unwind into the HAL.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Ok(state) = ctx.controls.read(ctx.device) {
            (ctx.cb)(state);
        }
    }));
    0
}

pub struct Subscription {
    ctx: *mut Context,
    addresses: Vec<AudioObjectPropertyAddress>,
}

// SAFETY: `ctx` is only shared with the HAL listener thread, and `Context`
// holds a `Send + Sync` callback and plain data.
unsafe impl Send for Subscription {}
unsafe impl Sync for Subscription {}

impl Subscription {
    pub(super) fn register(
        device: AudioObjectID,
        controls: Controls,
        cb: Callback,
    ) -> Result<Self> {
        let addresses = controls.watched();
        let ctx = Box::into_raw(Box::new(Context {
            device,
            controls,
            cb,
        }));
        let mut sub = Self {
            ctx,
            addresses: Vec::with_capacity(addresses.len()),
        };
        for a in addresses {
            // On error, `sub` drops: registered listeners are removed and the
            // context freed.
            props::add_listener(device, &a, on_change, ctx.cast())?;
            sub.addresses.push(a);
        }
        Ok(sub)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // SAFETY: `ctx` came from Box::into_raw and is still owned here.
        let device = unsafe { (*self.ctx).device };
        for a in &self.addresses {
            props::remove_listener(device, a, on_change, self.ctx.cast());
        }
        // In practice the HAL serializes removal with its notification
        // dispatch; Apple does not document it explicitly. If a crash ever
        // shows up here, switch to AudioObjectAddPropertyListenerBlock on a
        // private serial queue and free the context on that queue.
        drop(unsafe { Box::from_raw(self.ctx) });
    }
}
