//! A minimal [`IHostApplication`] implementation.
//!
//! VST3 components and edit controllers receive a host context when they are
//! initialized. Some plugins query [`IHostApplication`] during initialization
//! and only fully set themselves up (including their parameter list) when a
//! valid context is available.

use std::os::raw::c_void;

use vst3::Steinberg::Vst::{IHostApplication, IHostApplicationTrait, String128};
use vst3::Steinberg::{kNotImplemented, kResultOk, tresult, TUID};
use vst3::{Class, ComPtr, ComWrapper};

const HOST_NAME: &str = "Pedalboard VST Host";

struct HostApplication;

impl Class for HostApplication {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for HostApplication {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kNotImplemented;
        }

        let encoded: Vec<u16> = HOST_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let destination = &mut *name;
        let length = encoded.len().min(destination.len());
        destination[..length].copy_from_slice(&encoded[..length]);

        kResultOk
    }

    unsafe fn createInstance(
        &self,
        _cid: *mut TUID,
        _iid: *mut TUID,
        _obj: *mut *mut c_void,
    ) -> tresult {
        kNotImplemented
    }
}

/// Creates a host application context that can be passed to `initialize`.
pub fn create_host_application() -> ComPtr<IHostApplication> {
    ComWrapper::new(HostApplication)
        .to_com_ptr::<IHostApplication>()
        .expect("HostApplication implements IHostApplication")
}
