//! A minimal [`IComponentHandler`] implementation.
//!
//! VST3 plugins use the component handler to notify the host about parameter
//! edits (for example when the user turns a knob in the plugin's own editor).
//! Since this host does not embed the plugin's native GUI yet, all callbacks
//! are simply acknowledged with [`kResultOk`]. Passing a valid handler (rather
//! than a null pointer) keeps plugins that assume a host handler is present
//! happy.

use vst3::Steinberg::Vst::{IComponentHandler, IComponentHandlerTrait, ParamID, ParamValue};
use vst3::Steinberg::{int32, kResultOk, tresult};
use vst3::{Class, ComPtr, ComWrapper};

/// A no-op [`IComponentHandler`].
struct HostComponentHandler;

impl Class for HostComponentHandler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for HostComponentHandler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, _id: ParamID, _value_normalized: ParamValue) -> tresult {
        kResultOk
    }

    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, _flags: int32) -> tresult {
        kResultOk
    }
}

/// Creates a component handler that can be handed to an [`IEditController`].
///
/// [`IEditController`]: vst3::Steinberg::Vst::IEditController
pub fn create_component_handler() -> ComPtr<IComponentHandler> {
    ComWrapper::new(HostComponentHandler)
        .to_com_ptr::<IComponentHandler>()
        .expect("HostComponentHandler implements IComponentHandler")
}
