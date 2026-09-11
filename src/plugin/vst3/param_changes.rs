//! Host implementations of [`IParameterChanges`] and [`IParamValueQueue`].
//!
//! VST3 separates the *edit controller*, which owns the parameter list and
//! their values, from the *audio processor*, which performs the DSP. Changing a
//! parameter on the controller is therefore not enough to make an effect
//! audible: the host has to forward the new value to the processor through the
//! [`ProcessData::inputParameterChanges`] queue on the next `process` call.
//!
//! This module provides the minimal COM objects required for that. Without
//! them, plugins that use separate processor and controller classes (the
//! common case) keep using their default parameter values and appear to do
//! nothing.
//!
//! [`ProcessData::inputParameterChanges`]: vst3::Steinberg::Vst::ProcessData
//! [`IParameterChanges`]: vst3::Steinberg::Vst::IParameterChanges
//! [`IParamValueQueue`]: vst3::Steinberg::Vst::IParamValueQueue

use std::ptr;
use std::sync::Mutex;

use vst3::Steinberg::Vst::{
    IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait, ParamID,
    ParamValue,
};
use vst3::Steinberg::{int32, kInvalidArgument, kResultOk, tresult};
use vst3::{Class, ComPtr, ComWrapper};

/// The automation points recorded for a single parameter.
///
/// The VST3 interface takes `&self` for every method, so the points are guarded
/// by a [`Mutex`] to allow `addPoint` to append while `getPoint` reads.
struct ParamValueQueue {
    id: ParamID,
    points: Mutex<Vec<(int32, ParamValue)>>,
}

impl Class for ParamValueQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParamValueQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id
    }

    unsafe fn getPointCount(&self) -> int32 {
        self.points.lock().unwrap().len() as int32
    }

    unsafe fn getPoint(
        &self,
        index: int32,
        sample_offset: *mut int32,
        value: *mut ParamValue,
    ) -> tresult {
        let points = self.points.lock().unwrap();
        match points.get(index as usize) {
            Some(&(offset, point_value)) => {
                if !sample_offset.is_null() {
                    *sample_offset = offset;
                }
                if !value.is_null() {
                    *value = point_value;
                }
                kResultOk
            }
            None => kInvalidArgument,
        }
    }

    unsafe fn addPoint(
        &self,
        sample_offset: int32,
        value: ParamValue,
        index: *mut int32,
    ) -> tresult {
        let mut points = self.points.lock().unwrap();
        points.push((sample_offset, value));

        if !index.is_null() {
            *index = (points.len() - 1) as int32;
        }

        kResultOk
    }
}

/// A collection of [`ParamValueQueue`]s, one for each parameter that changed.
///
/// The processor consumes this object during a single `process` call; it is not
/// meant to outlive the call.
pub struct ParameterChanges {
    /// Parameter id and matching queue. Each queue owns its automation points.
    queues: Mutex<Vec<(ParamID, ComPtr<IParamValueQueue>)>>,
}

impl Class for ParameterChanges {
    type Interfaces = (IParameterChanges,);
}

impl Default for ParameterChanges {
    fn default() -> Self {
        Self::new()
    }
}

impl ParameterChanges {
    pub fn new() -> Self {
        Self {
            queues: Mutex::new(Vec::new()),
        }
    }

    /// Returns the index of the queue for `id`, creating a new one when `id`
    /// has not been seen before.
    fn ensure_queue_index(
        queues: &mut Vec<(ParamID, ComPtr<IParamValueQueue>)>,
        id: ParamID,
    ) -> usize {
        match queues.iter().position(|(existing, _)| *existing == id) {
            Some(position) => position,
            None => {
                let queue = ComWrapper::new(ParamValueQueue {
                    id,
                    points: Mutex::new(Vec::new()),
                })
                .to_com_ptr::<IParamValueQueue>()
                .expect("ParamValueQueue implements IParamValueQueue");

                queues.push((id, queue));
                queues.len() - 1
            }
        }
    }

    /// Records a single automation point for `id` at `sample_offset`.
    ///
    /// This is the entry point used by the host when a parameter was changed on
    /// the edit controller and needs to be forwarded to the processor.
    pub fn add_change(&self, id: ParamID, sample_offset: int32, value: ParamValue) {
        let mut queues = self.queues.lock().unwrap();
        let position = Self::ensure_queue_index(&mut queues, id);

        let mut point_index: int32 = 0;
        // SAFETY: `position` refers to a queue owned by `self` that outlives
        // this call, and the underlying object implements `IParamValueQueue`.
        unsafe {
            queues[position]
                .1
                .addPoint(sample_offset, value, &mut point_index);
        }
    }
}

impl IParameterChangesTrait for ParameterChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        self.queues.lock().unwrap().len() as int32
    }

    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        self.queues
            .lock()
            .unwrap()
            .get(index as usize)
            // The returned pointer stays valid for as long as the queue is kept
            // alive by `self`, matching the VST3 borrowing rules.
            .map(|(_, queue)| queue.as_ptr())
            .unwrap_or(ptr::null_mut())
    }

    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut int32,
    ) -> *mut IParamValueQueue {
        if id.is_null() {
            return ptr::null_mut();
        }

        let mut queues = self.queues.lock().unwrap();
        let position = Self::ensure_queue_index(&mut queues, *id);

        if !index.is_null() {
            *index = position as int32;
        }

        queues[position].1.as_ptr()
    }
}
