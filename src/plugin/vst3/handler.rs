// Completely trusted ChatGPT on this one...

use vst3::Interface;
use vst3::Steinberg::Vst::{IComponentHandler, ParamID, ParamValue};
use vst3::Steinberg::{FUnknown, FIDString, tresult, kResultOk, kNotImplemented};
use std::ffi::c_void;
use std::ptr;

// Compare 2 pointers to 16 byte i8 strings
unsafe fn compare_iid(a: FIDString, b: FIDString) -> bool {
    let a_ptr = a as *const i8;
    let b_ptr = b as *const i8;
    for i in 0..16 {
        if *a_ptr.add(i) != *b_ptr.add(i) {
            return false;
        }
    }
    true
}

#[repr(C)]
pub struct MyComponentHandler {
    vtable: *const IComponentHandlerVTable,
    refcount: std::cell::Cell<u32>,
}

#[repr(C)]
pub struct IComponentHandlerVTable {
    // FUnknown methods
    pub query_interface: unsafe extern "system" fn(*mut c_void, FIDString, *mut *mut c_void) -> tresult,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IComponentHandler methods
    pub begin_edit: unsafe extern "system" fn(*mut c_void, ParamID) -> tresult,
    pub perform_edit: unsafe extern "system" fn(*mut c_void, ParamID, ParamValue) -> tresult,
    pub end_edit: unsafe extern "system" fn(*mut c_void, ParamID) -> tresult,
    pub restart_component: unsafe extern "system" fn(*mut c_void, i32) -> tresult,
}

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: FIDString,
    obj: *mut *mut c_void,
) -> tresult {
    if compare_iid(iid, IComponentHandler::IID.as_ptr() as *const i8) || compare_iid(iid, FUnknown::IID.as_ptr() as *const i8) {
        *obj = this;
        add_ref(this);
        kResultOk
    } else {
        *obj = ptr::null_mut();
        kNotImplemented
    }
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let handler = &*(this as *mut MyComponentHandler);
    let count = handler.refcount.get() + 1;
    handler.refcount.set(count);
    count
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let handler = &*(this as *mut MyComponentHandler);
    let count = handler.refcount.get() - 1;
    handler.refcount.set(count);
    if count == 0 {
        drop(Box::from_raw(this as *mut MyComponentHandler));
    }
    count
}

unsafe extern "system" fn begin_edit(_: *mut c_void, _: ParamID) -> tresult {
    kResultOk
}
unsafe extern "system" fn perform_edit(_: *mut c_void, _: ParamID, _: ParamValue) -> tresult {
    kResultOk
}
unsafe extern "system" fn end_edit(_: *mut c_void, _: ParamID) -> tresult {
    kResultOk
}
unsafe extern "system" fn restart_component(_: *mut c_void, _: i32) -> tresult {
    kResultOk
}

static VTABLE: IComponentHandlerVTable = IComponentHandlerVTable {
    query_interface,
    add_ref,
    release,
    begin_edit,
    perform_edit,
    end_edit,
    restart_component,
};

pub fn create_component_handler() -> *mut IComponentHandler {
    let handler = Box::new(MyComponentHandler {
        vtable: &VTABLE,
        refcount: std::cell::Cell::new(1),
    });
    Box::into_raw(handler) as *mut _
}
