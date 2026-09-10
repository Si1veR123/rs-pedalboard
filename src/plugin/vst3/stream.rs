//! A minimal in-memory [`IBStream`] implementation.
//!
//! Used to transfer state between a VST3 component and its edit controller
//! (for example when the plugin exposes separate component and controller
//! classes).

use std::os::raw::c_void;
use std::sync::Mutex;

use vst3::Steinberg::{
    int32, int64, kInvalidArgument, kResultOk, tresult, IBStream, IBStreamTrait,
};
use vst3::{Class, ComPtr, ComWrapper};

// Seek modes from the VST3 SDK's `IStreamSeekMode` enum.
const IB_SEEK_SET: int32 = 0;
const IB_SEEK_CUR: int32 = 1;
const IB_SEEK_END: int32 = 2;

struct MemoryStream {
    data: Mutex<Vec<u8>>,
    position: Mutex<usize>,
}

impl Class for MemoryStream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for MemoryStream {
    unsafe fn read(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_bytes_read: *mut int32,
    ) -> tresult {
        let data = self.data.lock().unwrap();
        let mut position = self.position.lock().unwrap();

        let available = data.len().saturating_sub(*position);
        let to_read = (num_bytes.max(0) as usize).min(available);
        if to_read > 0 {
            std::ptr::copy_nonoverlapping(
                data[*position..].as_ptr(),
                buffer as *mut u8,
                to_read,
            );
            *position += to_read;
        }

        if !num_bytes_read.is_null() {
            *num_bytes_read = to_read as int32;
        }

        kResultOk
    }

    unsafe fn write(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_bytes_written: *mut int32,
    ) -> tresult {
        let mut data = self.data.lock().unwrap();
        let mut position = self.position.lock().unwrap();

        let num_bytes = num_bytes.max(0) as usize;
        let end = *position + num_bytes;
        if end > data.len() {
            data.resize(end, 0);
        }

        if num_bytes > 0 {
            std::ptr::copy_nonoverlapping(
                buffer as *const u8,
                data[*position..].as_mut_ptr(),
                num_bytes,
            );
        }
        *position = end;

        if !num_bytes_written.is_null() {
            *num_bytes_written = num_bytes as int32;
        }

        kResultOk
    }

    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let length = self.data.lock().unwrap().len() as int64;
        let mut position = self.position.lock().unwrap();

        let new_position = match mode {
            IB_SEEK_SET => pos,
            IB_SEEK_CUR => *position as int64 + pos,
            IB_SEEK_END => length + pos,
            _ => return kInvalidArgument,
        };

        if new_position < 0 {
            return kInvalidArgument;
        }

        *position = new_position as usize;
        if !result.is_null() {
            *result = new_position;
        }

        kResultOk
    }

    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        *pos = *self.position.lock().unwrap() as int64;
        kResultOk
    }
}

/// Creates a new, empty in-memory stream.
pub fn create_memory_stream() -> ComPtr<IBStream> {
    ComWrapper::new(MemoryStream {
        data: Mutex::new(Vec::new()),
        position: Mutex::new(0),
    })
    .to_com_ptr::<IBStream>()
    .expect("MemoryStream implements IBStream")
}
