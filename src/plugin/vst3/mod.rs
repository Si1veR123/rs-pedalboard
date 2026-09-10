//! A minimal, GUI-less VST3 host.
//!
//! This module loads a VST3 plugin, exposes its parameters as normalized
//! `0.0..=1.0` values (mirroring the VST2 pedal) and processes mono audio
//! through it. The plugin's own editor is not embedded yet; a no-op
//! [`handler::create_component_handler`] is passed to the controller so that
//! plugins which expect a host handler behave correctly.

pub mod handler;
pub mod host;
pub mod stream;

use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use libloading::Library;
use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, BusDirections_, BusInfo, IAudioProcessor,
    IAudioProcessorTrait, IComponent, IComponentTrait, IConnectionPoint, IConnectionPointTrait,
    IEditController, IEditControllerTrait, MediaTypes_, ParameterInfo, ParamID, ProcessData,
    ProcessModes_, ProcessSetup, String128, SymbolicSampleSizes_,
};
use vst3::Steinberg::{
    char8, kResultOk, FIDString, FUnknown, IBStreamTrait, IPluginBaseTrait, IPluginFactory,
    IPluginFactoryTrait, PClassInfo, TUID,
};
use vst3::Steinberg::Vst::IHostApplication;
use vst3::{ComPtr, ComRef, Interface};

#[cfg(target_os = "windows")]
pub const VST3_PLUGIN_PATH: &str = r"C:\Program Files\Common Files\VST3";
#[cfg(target_os = "linux")]
pub const VST3_PLUGIN_PATH: &str = "/usr/lib/vst3";
#[cfg(target_os = "macos")]
pub const VST3_PLUGIN_PATH: &str = "/Library/Audio/Plug-Ins/VST3";

/// VST3 plugins export their factory through this symbol.
type GetFactoryFn = unsafe extern "system" fn() -> *mut IPluginFactory;

/// The `PClassInfo::category` used by audio processing components.
const AUDIO_MODULE_CLASS: &str = "Audio Module Class";

#[derive(Clone, Debug)]
struct Vst3Parameter {
    id: ParamID,
    title: String,
    units: String,
}

fn read_c_string(bytes: &[char8]) -> String {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let utf8: Vec<u8> = bytes[..len].iter().map(|&b| b as u8).collect();
    String::from_utf8_lossy(&utf8).to_string()
}

fn read_string128(string: &String128) -> String {
    let len = string.iter().position(|&c| c == 0).unwrap_or(string.len());
    String::from_utf16_lossy(&string[..len])
}

/// Resolves a user selected path to the shared library that should be loaded.
///
/// VST3 plugins are distributed as bundles (directories ending in `.vst3`)
/// which contain the platform binary in an architecture specific
/// subdirectory. A path that already points at a file is used as-is.
pub fn resolve_plugin_binary(selected: &Path) -> Option<PathBuf> {
    if selected.is_file() {
        return Some(selected.to_path_buf());
    }

    if !selected.is_dir() {
        return None;
    }

    let contents = selected.join("Contents");
    let root = if contents.is_dir() {
        contents
    } else {
        selected.to_path_buf()
    };

    #[cfg(target_os = "windows")]
    let arch_dirs: &[&str] = &["x86_64-win", "x86-win", "x86_64-win7"];
    #[cfg(target_os = "linux")]
    let arch_dirs: &[&str] = &["x86_64-linux", "i386-linux", "aarch64-linux"];
    #[cfg(target_os = "macos")]
    let arch_dirs: &[&str] = &["MacOS"];

    for arch in arch_dirs {
        let dir = root.join(arch);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }

    None
}

/// A loaded VST3 plugin instance.
///
/// The instance is intentionally independent of any windowing or GUI code so
/// that it can run on the audio thread. Parameters are exposed as normalized
/// values in the range `0.0..=1.0`.
pub struct Vst3Instance {
    /// Kept alive for as long as any object created by the factory is in use.
    #[allow(dead_code)]
    factory: ComPtr<IPluginFactory>,
    component: ComPtr<IComponent>,
    controller: ComPtr<IEditController>,
    processor: ComPtr<IAudioProcessor>,
    /// Handed to the controller; kept alive for as long as the controller is.
    #[allow(dead_code)]
    component_handler: ComPtr<vst3::Steinberg::Vst::IComponentHandler>,
    /// Host context passed to the component and controller `initialize` calls.
    #[allow(dead_code)]
    host_application: ComPtr<IHostApplication>,
    parameters: Vec<Vst3Parameter>,
    plugin_name: String,
    /// The path selected by the user. Used for serialisation so that bundles
    /// round-trip rather than the resolved inner binary.
    selected_path: PathBuf,
    /// The actual shared library that was loaded.
    #[allow(dead_code)]
    binary_path: PathBuf,
    in_bus_buffers: Vec<Vec<Box<[f32]>>>,
    in_bus_ptrs: Vec<Vec<*mut f32>>,
    out_bus_buffers: Vec<Vec<Box<[f32]>>>,
    out_bus_ptrs: Vec<Vec<*mut f32>>,
    output_channel_count: usize,
    sample_rate: f32,
    buffer_size: usize,
    /// Kept alive until every plugin object has been released, so it must be
    /// the last field to be dropped.
    #[allow(dead_code)]
    library: Library,
}

impl Vst3Instance {
    /// Loads a VST3 plugin from `path`, which may be either a `.vst3` bundle
    /// directory or the platform shared library itself.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        let selected_path = path.as_ref().to_path_buf();
        let binary_path = resolve_plugin_binary(&selected_path).ok_or_else(|| {
            tracing::error!(
                "Could not find a VST3 binary within {:?}",
                selected_path
            );
        })?;

        // SAFETY: Loading an untrusted plugin is inherently unsafe. We pass a
        // valid, absolute path and keep the library alive for the plugin
        // objects' lifetime.
        let library = unsafe { Library::new(&binary_path) }.map_err(|e| {
            tracing::error!("Failed to load VST3 library {:?}: {}", binary_path, e);
        })?;

        let factory: ComPtr<IPluginFactory> = {
            // SAFETY: `GetPluginFactory` is part of the VST3 ABI. The symbol is
            // only borrowed for the duration of this block, after which the
            // library ownership is moved into the instance.
            let get_factory: libloading::Symbol<GetFactoryFn> =
                unsafe { library.get(b"GetPluginFactory\0") }.map_err(|e| {
                    tracing::error!("GetPluginFactory missing in {:?}: {}", binary_path, e);
                })?;

            let factory_ptr = unsafe { get_factory() };

            // The returned reference is owned by the plugin, so increment the
            // reference count before taking ownership of it.
            let factory_ref = unsafe { ComRef::from_raw(factory_ptr) }.ok_or_else(|| {
                tracing::error!("GetPluginFactory returned null for {:?}", binary_path);
            })?;

            factory_ref.to_com_ptr()
        };

        let (component, plugin_name) = Self::create_component(&factory)?;

        let host_application = host::create_host_application();
        let host_context = host_application.clone().upcast::<FUnknown>();
        let host_context_ptr = host_context.as_ptr();

        // SAFETY: `host_context_ptr` points to a valid host application that
        // outlives both the component and controller.
        if unsafe { component.initialize(host_context_ptr) } != kResultOk {
            tracing::error!("Failed to initialize VST3 component {:?}", binary_path);
            return Err(());
        }

        let controller = Self::create_controller(&factory, &component, host_context_ptr)?;

        let component_handler = handler::create_component_handler();
        // SAFETY: `component_handler` outlives the controller.
        unsafe {
            controller.setComponentHandler(component_handler.as_ptr());
        }

        // Plugins that expose a separate component and controller class only
        // report their parameter list once the two have been wired together
        // through their connection points.
        if let (Some(component_point), Some(controller_point)) = (
            component.cast::<IConnectionPoint>(),
            controller.cast::<IConnectionPoint>(),
        ) {
            // SAFETY: Both connection points are valid and outlive the calls.
            unsafe {
                component_point.connect(controller_point.as_ptr());
                controller_point.connect(component_point.as_ptr());
            }
        }

        // Propagate the component state to the controller up front.
        let state_stream = stream::create_memory_stream();
        // SAFETY: `state_stream` outlives all calls and is a valid `IBStream`.
        unsafe {
            component.getState(state_stream.as_ptr());
            let mut position: i64 = 0;
            state_stream.seek(0, 0, &mut position);
            controller.setComponentState(state_stream.as_ptr());
        }

        // SAFETY: The audio processor is a required interface of a VST3
        // component, but we still handle a missing interface gracefully.
        let processor = component.cast::<IAudioProcessor>().ok_or_else(|| {
            tracing::error!("VST3 component does not implement IAudioProcessor");
        })?;

        let parameters = Self::read_parameters(&controller);

        Ok(Vst3Instance {
            factory,
            component,
            controller,
            processor,
            component_handler,
            host_application,
            parameters,
            plugin_name,
            selected_path,
            binary_path,
            in_bus_buffers: Vec::new(),
            in_bus_ptrs: Vec::new(),
            out_bus_buffers: Vec::new(),
            out_bus_ptrs: Vec::new(),
            output_channel_count: 0,
            sample_rate: 0.0,
            buffer_size: 0,
            library,
        })
    }

    /// Finds the first "Audio Module Class" exposed by the factory and
    /// instantiates it as an [`IComponent`].
    fn create_component(
        factory: &ComPtr<IPluginFactory>,
    ) -> Result<(ComPtr<IComponent>, String), ()> {
        // SAFETY: `factory` is a valid plugin factory.
        let class_count = unsafe { factory.countClasses() };

        for index in 0..class_count {
            let mut class_info = PClassInfo {
                cid: [0; 16],
                cardinality: 0,
                category: [0; 32],
                name: [0; 64],
            };

            // SAFETY: `class_info` is a valid, initialised `PClassInfo`.
            let res = unsafe { factory.getClassInfo(index, &mut class_info) };
            if res != kResultOk || read_c_string(&class_info.category) != AUDIO_MODULE_CLASS {
                continue;
            }

            let mut obj: *mut std::ffi::c_void = null_mut();
            // SAFETY: The class id comes from the factory and `obj` is a valid
            // out pointer.
            let res = unsafe {
                factory.createInstance(
                    class_info.cid.as_ptr(),
                    IComponent::IID.as_ptr() as FIDString,
                    &mut obj,
                )
            };

            if res != kResultOk || obj.is_null() {
                continue;
            }

            // SAFETY: `createInstance` returns an owning reference on success.
            if let Some(component) = unsafe { ComPtr::from_raw(obj as *mut IComponent) } {
                return Ok((component, read_c_string(&class_info.name)));
            }
        }

        tracing::error!("No Audio Module Class found in VST3 plugin");
        Err(())
    }

    /// Returns the plugin's edit controller, creating a separate one when the
    /// component does not implement [`IEditController`] itself.
    fn create_controller(
        factory: &ComPtr<IPluginFactory>,
        component: &ComPtr<IComponent>,
        context: *mut FUnknown,
    ) -> Result<ComPtr<IEditController>, ()> {
        // Many plugins implement the controller on the same object as the
        // component, in which case it is already initialised.
        if let Some(controller) = component.cast::<IEditController>() {
            return Ok(controller);
        }

        let mut controller_cid: TUID = [0; 16];
        // SAFETY: `controller_cid` is a valid out pointer.
        let res = unsafe { component.getControllerClassId(&mut controller_cid) };
        if res != kResultOk {
            tracing::error!("VST3 plugin has no controller class id");
            return Err(());
        }

        let mut obj: *mut std::ffi::c_void = null_mut();
        // SAFETY: The controller class id comes from the component.
        let res = unsafe {
            factory.createInstance(
                controller_cid.as_ptr(),
                IEditController::IID.as_ptr() as FIDString,
                &mut obj,
            )
        };
        if res != kResultOk || obj.is_null() {
            tracing::error!("Failed to create VST3 edit controller");
            return Err(());
        }

        // SAFETY: `createInstance` returns an owning reference on success.
        let controller = unsafe { ComPtr::from_raw(obj as *mut IEditController) }.ok_or(())?;

        // SAFETY: A freshly created, separate controller needs initialising with
        // the same host context as the component.
        if unsafe { controller.initialize(context) } != kResultOk {
            tracing::error!("Failed to initialize VST3 edit controller");
            return Err(());
        }

        Ok(controller)
    }

    /// Reads the plugin's parameter list.
    fn read_parameters(controller: &ComPtr<IEditController>) -> Vec<Vst3Parameter> {
        // SAFETY: `controller` is a valid, initialised edit controller.
        let parameter_count = unsafe { controller.getParameterCount() };
        let mut parameters = Vec::with_capacity(parameter_count.max(0) as usize);

        for index in 0..parameter_count {
            let mut info = ParameterInfo {
                id: 0,
                title: [0; 128],
                shortTitle: [0; 128],
                units: [0; 128],
                stepCount: 0,
                defaultNormalizedValue: 0.0,
                unitId: 0,
                flags: 0,
            };

            // SAFETY: `info` is a valid, initialised `ParameterInfo`.
            if unsafe { controller.getParameterInfo(index, &mut info) } != kResultOk {
                continue;
            }

            parameters.push(Vst3Parameter {
                id: info.id,
                title: read_string128(&info.title),
                units: read_string128(&info.units),
            });
        }

        parameters
    }

    /// Configures the plugin for the given buffer size and sample rate and
    /// activates it for processing.
    pub fn set_config(&mut self, buffer_size: usize, sample_rate: u32) {
        if self.is_configured() {
            // SAFETY: Deactivating a configured plugin is always allowed.
            unsafe {
                self.processor.setProcessing(0);
                self.component.setActive(0);
            }
        }

        self.buffer_size = buffer_size;
        self.sample_rate = sample_rate as f32;

        let input_channel_counts = self.audio_bus_channel_counts(BusDirections_::kInput as i32);
        let output_channel_counts = self.audio_bus_channel_counts(BusDirections_::kOutput as i32);

        self.output_channel_count = output_channel_counts
            .iter()
            .map(|&count| count.max(0) as usize)
            .sum();

        let (in_buffers, in_pointers) = self.allocate_direction(
            BusDirections_::kInput as i32,
            &input_channel_counts,
            buffer_size,
        );
        let (out_buffers, out_pointers) = self.allocate_direction(
            BusDirections_::kOutput as i32,
            &output_channel_counts,
            buffer_size,
        );

        self.in_bus_buffers = in_buffers;
        self.in_bus_ptrs = in_pointers;
        self.out_bus_buffers = out_buffers;
        self.out_bus_ptrs = out_pointers;

        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            maxSamplesPerBlock: buffer_size as i32,
            sampleRate: sample_rate as f64,
        };

        // SAFETY: `setup` is valid for the duration of the call.
        if unsafe { self.processor.setupProcessing(&mut setup) } != kResultOk {
            tracing::error!("Failed to set up VST3 processing");
            self.buffer_size = 0;
            self.sample_rate = 0.0;
            return;
        }

        // SAFETY: The component and processor are now fully configured.
        unsafe {
            self.component.setActive(1);
            self.processor.setProcessing(1);
        }
    }

    /// Returns the channel count of each audio bus in the given direction.
    fn audio_bus_channel_counts(&self, direction: i32) -> Vec<i32> {
        // SAFETY: `direction` is a valid bus direction.
        let bus_count =
            unsafe { self.component.getBusCount(MediaTypes_::kAudio as i32, direction) };

        let mut counts = Vec::new();
        for index in 0..bus_count {
            // SAFETY: `BusInfo` is a plain C struct, so zero initialisation is valid.
            let mut info: BusInfo = unsafe { std::mem::zeroed() };
            // SAFETY: `info` is a valid out pointer for this bus.
            let res = unsafe {
                self.component
                    .getBusInfo(MediaTypes_::kAudio as i32, direction, index, &mut info)
            };
            if res == kResultOk {
                counts.push(info.channelCount);
            }
        }

        counts
    }

    /// Allocates per-bus audio buffers for the given direction and activates
    /// each bus.
    fn allocate_direction(
        &self,
        direction: i32,
        channel_counts: &[i32],
        buffer_size: usize,
    ) -> (Vec<Vec<Box<[f32]>>>, Vec<Vec<*mut f32>>) {
        let mut buffers = Vec::with_capacity(channel_counts.len());
        let mut pointers = Vec::with_capacity(channel_counts.len());

        for (bus_index, &channel_count) in channel_counts.iter().enumerate() {
            // SAFETY: `bus_index` refers to a bus reported by the component.
            unsafe {
                self.component.activateBus(
                    MediaTypes_::kAudio as i32,
                    direction,
                    bus_index as i32,
                    1,
                );
            }

            let channel_count = channel_count.max(1) as usize;
            let mut bus_buffers: Vec<Box<[f32]>> = (0..channel_count)
                .map(|_| vec![0.0; buffer_size].into_boxed_slice())
                .collect();
            let bus_pointers: Vec<*mut f32> =
                bus_buffers.iter_mut().map(|buf| buf.as_mut_ptr()).collect();

            buffers.push(bus_buffers);
            pointers.push(bus_pointers);
        }

        (buffers, pointers)
    }

    /// Processes a mono buffer through the plugin.
    ///
    /// The mono input is copied to every input channel and the plugin's output
    /// channels are averaged back into `output`.
    ///
    /// Ensure that [`Vst3Instance::set_config`] has been called first.
    pub fn process(&mut self, input: &mut [f32], output: &mut [f32]) {
        assert_eq!(
            input.len(),
            output.len(),
            "Input and output buffers must have the same length"
        );
        assert!(
            self.is_configured(),
            "VST3 instance must be configured with sample rate and buffer size before processing"
        );
        assert!(
            input.len() <= self.buffer_size,
            "Input buffer length must not exceed configured buffer size"
        );

        let num_samples = input.len();
        output.fill(0.0);

        for bus in &mut self.out_bus_buffers {
            for channel in bus.iter_mut() {
                channel[..num_samples].fill(0.0);
            }
        }

        for bus in &mut self.in_bus_buffers {
            for channel in bus.iter_mut() {
                channel[..num_samples].copy_from_slice(input);
            }
        }

        let mut in_buses: Vec<AudioBusBuffers> = self
            .in_bus_ptrs
            .iter_mut()
            .map(|pointers| AudioBusBuffers {
                numChannels: pointers.len() as i32,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: pointers.as_mut_ptr(),
                },
            })
            .collect();

        let mut out_buses: Vec<AudioBusBuffers> = self
            .out_bus_ptrs
            .iter_mut()
            .map(|pointers| AudioBusBuffers {
                numChannels: pointers.len() as i32,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: pointers.as_mut_ptr(),
                },
            })
            .collect();

        let mut process_data = ProcessData {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            numSamples: num_samples as i32,
            numInputs: in_buses.len() as i32,
            numOutputs: out_buses.len() as i32,
            inputs: in_buses.as_mut_ptr(),
            outputs: out_buses.as_mut_ptr(),
            inputParameterChanges: null_mut(),
            outputParameterChanges: null_mut(),
            inputEvents: null_mut(),
            outputEvents: null_mut(),
            processContext: null_mut(),
        };

        // SAFETY: The bus descriptors and channel buffers are valid for
        // `num_samples` samples and outlive the call.
        if unsafe { self.processor.process(&mut process_data) } != kResultOk {
            tracing::error!("VST3 plugin failed to process audio");
            return;
        }

        // Average the plugin's output channels into the mono output buffer.
        let divisor = self.output_channel_count.max(1) as f32;
        for bus in &self.out_bus_buffers {
            for channel in bus.iter() {
                for (output_sample, &plugin_sample) in output.iter_mut().zip(channel.iter()) {
                    *output_sample += plugin_sample / divisor;
                }
            }
        }
    }

    pub fn is_configured(&self) -> bool {
        self.sample_rate > 0.0 && self.buffer_size > 0
    }

    pub fn dll_path(&self) -> &Path {
        self.selected_path.as_path()
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn plugin_name(&self) -> String {
        self.plugin_name.clone()
    }

    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    pub fn parameter_name(&self, index: usize) -> String {
        self.parameters
            .get(index)
            .map(|parameter| parameter.title.clone())
            .unwrap_or_else(|| "Invalid Parameter".to_string())
    }

    pub fn parameter_value(&self, index: usize) -> f32 {
        match self.parameters.get(index) {
            // SAFETY: `parameter.id` was obtained from the same controller.
            Some(parameter) => unsafe { self.controller.getParamNormalized(parameter.id) as f32 },
            None => -1.0,
        }
    }

    pub fn parameter_label(&self, index: usize) -> String {
        self.parameters
            .get(index)
            .map(|parameter| parameter.units.clone())
            .unwrap_or_default()
    }

    pub fn set_parameter_value(&mut self, index: usize, value: f32) {
        let Some(parameter) = self.parameters.get(index) else {
            return;
        };

        // SAFETY: `parameter.id` was obtained from the same controller.
        unsafe {
            self.controller
                .setParamNormalized(parameter.id, value.clamp(0.0, 1.0) as f64);
        }
    }
}

impl Clone for Vst3Instance {
    fn clone(&self) -> Self {
        // Reload the plugin so that the clone has independent internal state.
        let mut instance = Self::load(&self.selected_path)
            .expect("Plugin has previously been loaded, so cloning should succeed");
        instance.set_config(self.buffer_size, self.sample_rate as u32);
        instance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_vst3_plugin() {
        let plugin_path = PathBuf::from(r"C:\Program Files\Common Files\VST3\NA Black.vst3");
        if !plugin_path.exists() {
            eprintln!("Skipping VST3 test: {:?} does not exist", plugin_path);
            return;
        }

        let mut instance = Vst3Instance::load(plugin_path).expect("Failed to load VST3 plugin");
        instance.set_config(512, 48000);

        println!("Plugin: {}", instance.plugin_name());
        println!("Parameters: {}", instance.parameter_count());

        let mut input = vec![0.1; 512];
        let mut output = vec![0.0; 512];
        instance.process(&mut input, &mut output);
        assert_eq!(output.len(), input.len());
        println!("Output: {:?}", &output[..20]);
    }
}
