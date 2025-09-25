pub mod handler;

use eframe::WindowAttributes;
use libloading::Library;
use vst3::Interface;
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window;
use std::ffi::OsStr;
use std::os::raw::c_void;
use std::ptr::{self, null_mut, NonNull};
use std::path::{Path, PathBuf};

// Import raw VST3 interfaces
use vst3::Steinberg::{char8, int32, kResultOk, kResultTrue, FUnknown, IPlugView, IPluginBase, IPluginFactory, PClassInfo, TBool, TUID};
use vst3::Steinberg::Vst::{AudioBusBuffers, AudioBusBuffers__type0, IAudioProcessor, IComponent, IComponentHandler, IEditController, IHostApplication, ProcessData, ProcessModes, ProcessModes_, ProcessSetup, SymbolicSampleSizes, SymbolicSampleSizes_};

// GetPluginFactory symbol type
type GetFactoryFn = unsafe extern "system" fn() -> *mut FUnknown;

const AUDIO_MODULE_CLASS: [char8; 32] = {
    let mut desired_category_bytes: [char8; 32] = [0; 32];
    let desired_category_string = "Audio Module Class".as_bytes();

    let mut i = 0;
    loop {
        if i < desired_category_string.len() {
            desired_category_bytes[i] = desired_category_string[i] as char8;
        }
        if i >= 31 {
            break;
        }
        i += 1;
    }

    desired_category_bytes
};

enum Vst3Command {
    OpenGui,
    CloseGui
}

pub struct Vst3ProcessHandle {
    audio_processor: NonNull<IAudioProcessor>,
    kill_channel: crossbeam::channel::Receiver<bool>,
    command_sender: crossbeam::channel::Sender<Vst3Command>,
}

// This is safe to send across threads, but the VST3 SDK specifies that only the audio thread should call process methods.
unsafe impl Send for Vst3ProcessHandle {}

impl Vst3ProcessHandle {
    pub fn plugin_is_active(&self) -> bool {
        self.kill_channel.is_empty()
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        if self.kill_channel.len() > 0 {
            return;
        }

        if buffer.is_empty() {
            return;
        }

        let mut bus = AudioBusBuffers {
            numChannels: 1,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: &mut buffer.as_mut_ptr()
            }
        };

        let mut process_data = ProcessData {
            processMode: ProcessModes_::kRealtime,
            symbolicSampleSize: kSample32,
            numSamples: buffer.len() as int32,
            inputs: &mut bus,
            outputs: &mut bus,
            inputEvents: null_mut(),
            outputEvents: null_mut(),
            processContext: null_mut(),
            numInputs: 1,
            numOutputs: 1,
            inputParameterChanges: null_mut(),
            outputParameterChanges: null_mut(),
            
        };
        
        let res = unsafe {
            ((*(*self.audio_processor.as_ptr()).vtbl).process)(self.audio_processor.as_ptr(), &mut process_data)
        };
        if res != kResultOk {
            tracing::error!("Failed to process audio: {}", res);
            return;
        }
    }

    pub fn open_gui_window(&mut self) -> Result<(), String> {
        if self.kill_channel.len() > 0 {
            return Err("Plugin is not active".into());
        }

        // Send command to open GUI
        self.command_sender.send(Vst3Command::OpenGui)
            .map_err(|e| format!("Failed to send OpenGui command: {}", e))?;

        Ok(())
    }

    pub fn close_gui_window(&mut self) -> Result<(), String> {
        if self.kill_channel.len() > 0 {
            return Err("Plugin is not active".into());
        }

        // Send command to close GUI
        self.command_sender.send(Vst3Command::CloseGui)
            .map_err(|e| format!("Failed to send CloseGui command: {}", e))?;

        Ok(())
    }
}

impl Drop for Vst3ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            (((*(*self.audio_processor.as_ptr()).vtbl).base.release))(self.audio_processor.as_ptr() as *mut FUnknown);
        }
    }
}

pub struct RawVst3Plugin {
    lib: Library,
    factory: NonNull<IPluginFactory>,
    component: NonNull<IComponent>,
    kill_channel: crossbeam::channel::Sender<bool>,
    command_receiver: crossbeam::channel::Receiver<Vst3Command>,
    plug_view: *mut IPlugView,
    window: Option<window::Window>,
}

impl RawVst3Plugin {
    fn get_factory(lib: &Library) -> Result<*mut IPluginFactory, String> {
        // Get the GetPluginFactory symbol
        let get_factory: libloading::Symbol<GetFactoryFn> =
            unsafe { lib.get(b"GetPluginFactory\0") }
            .map_err(|e| format!("GetPluginFactory missing: {}", e))?;

        // Call the function to get the factory pointer
        let factory_ptr = unsafe { get_factory() };
        if factory_ptr.is_null() {
            return Err("GetPluginFactory returned null".into());
        }

        Ok(factory_ptr as *mut IPluginFactory)
    }

    unsafe fn get_audio_module_class(factory: *mut IPluginFactory) -> Result<*mut IComponent, String> {
        // Find the first Audio Module Class
        let class_count = ((*(*factory).vtbl).countClasses)(factory);

        let mut i = 0;
        let processor_component = loop {
            let mut class_info = PClassInfo {
                cid: TUID::default(),
                cardinality: int32::default(),
                category: [char8::default(); 32],
                name: [char8::default(); 64],
            };
            let res = ((*(*factory).vtbl).getClassInfo)(factory, i, &mut class_info);
            if res != kResultOk {
                return Err("Failed to get class info".into());
            }

            if class_info.category == AUDIO_MODULE_CLASS {
                let mut processor_component: *mut IComponent = null_mut();
                let res = ((*(*factory).vtbl).createInstance)(factory, class_info.cid.as_ptr(), IComponent::IID.as_ptr() as *const i8, &mut processor_component as *mut *mut IComponent as *mut *mut c_void);

                if !processor_component.is_null() && res == kResultOk {
                    break processor_component;
                } else {
                    return Err("Failed to create IComponent instance".into());
                }
            }

            i += 1;

            if i >= class_count {
                return Err("No Audio Module Class found".into());
            }
        };

        let res = ((*(*processor_component).vtbl).base.initialize)(processor_component as *mut IPluginBase, null_mut() as *mut FUnknown);
        if res != kResultOk {
            return Err("Failed to initialize IComponent".into());
        }

        Ok(processor_component)
    }

    unsafe fn initialize_controller(factory: *mut IPluginFactory, processor_component: *mut IComponent, component_handler: *mut IComponentHandler) -> Result<*mut IEditController, String> {
        // Attempt to get the IEditController directly from IComponent
        let mut edit_controller: *mut IEditController = null_mut();
        let query_result = ((*(*processor_component).vtbl).base.base.queryInterface)(
            processor_component as *mut FUnknown,
            IEditController::IID.as_ptr() as *const i8 as *const [i8; 16],
            &mut edit_controller as *mut *mut IEditController as *mut *mut c_void,
        );

        if query_result != kResultTrue || edit_controller.is_null() {
            // Try to get controller class ID
            let mut controller_cid = [0; 16];
            let has_controller = (((*(*processor_component).vtbl).getControllerClassId))(processor_component, &mut controller_cid);
            
            if has_controller == kResultTrue {
                let create_result = ((*(*factory).vtbl).createInstance)(
                    factory,
                    controller_cid.as_ptr(),
                    IEditController::IID.as_ptr() as *const i8,
                    &mut edit_controller as *mut *mut IEditController as *mut *mut c_void,
                );

                if create_result != kResultOk || edit_controller.is_null() {
                    return Err("Failed to create IEditController from factory".into());
                }
            } else {
                return Err("Plugin has no valid controller class ID".into());
            }
        }

        // Now edit_controller is valid
        let init_res = ((*(*edit_controller).vtbl).base.initialize)(
            edit_controller as *mut IPluginBase,
            component_handler as *mut FUnknown,
        );

        if init_res != kResultOk {
            return Err("Failed to initialize IEditController".into());
        }

        Ok(edit_controller)
    }

    unsafe fn initialize_audio_processor(processor_component: *mut IComponent, max_samples: usize) -> Result<*mut IAudioProcessor, String> {
        // Get the IAudioProcessor interface from the component and set it up
        let res = ((*(*processor_component).vtbl).setActive)(processor_component, 0);
        if res != kResultOk {
            return Err("Failed to set component to inactive".into());
        }

        let mut process_setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32,
            maxSamplesPerBlock: max_samples as int32,
            sampleRate: 48000.0,
        };

        let mut audio_processor: *mut IAudioProcessor = null_mut();
        let res = ((*(*processor_component).vtbl).base.base.queryInterface)(
            processor_component as *mut FUnknown, 
            IAudioProcessor::IID.as_ptr() as *const i8 as *const [i8; 16],
            &mut audio_processor as *mut *mut IAudioProcessor as *mut *mut c_void,
        );

        if res != kResultTrue || audio_processor.is_null() {
            return Err("Failed to get IAudioProcessor interface".into());
        }

        let res = ((*(*audio_processor).vtbl).setupProcessing)(
            audio_processor,
            &mut process_setup as *mut ProcessSetup,
        );
        if res != kResultOk {
            return Err("Failed to setup processing".into());
        }

        let res = ((*(*processor_component).vtbl).setActive)(processor_component, 1);
        if res != kResultOk {
            return Err("Failed to set component to active".into());
        }

        Ok(audio_processor)
    }

    unsafe fn get_plugin_view(controller: *mut IEditController) -> Result<*mut IPlugView, String> {
        let plug_view = (((*(*controller).vtbl).createView))(controller, b"editor\0".as_ptr() as *const i8);
        Ok(plug_view)
    }

    fn should_gui_be_open(&mut self) -> Option<bool> {
        match self.command_receiver.try_recv() {
            Ok(command) => {
                match command {
                    Vst3Command::OpenGui => Some(true),
                    Vst3Command::CloseGui => Some(false),
                }
            },
            Err(_) => None,
        }
    }

    pub fn load(path: PathBuf, max_samples: usize) -> Result<Vst3ProcessHandle, String> {
        let (s, r) = crossbeam::channel::bounded(1);

        std::thread::spawn(move || {
            let lib = unsafe { Library::new(path) }
                .map_err(|e| format!("Failed to load plugin: {}", e));

            let lib = match lib {
                Ok(lib) => lib,
                Err(e) => {
                    s.send(Err(e)).expect("Failed to send error");
                    return;
                }
            };

            let factory = Self::get_factory(&lib);
            let factory = match factory {
                Ok(factory) => factory,
                Err(e) => {
                    s.send(Err(e)).expect("Failed to send error");
                    return;
                }
            };

            let processor_component = unsafe { Self::get_audio_module_class(factory) };
            let processor_component = match processor_component {
                Ok(component) => component,
                Err(e) => {
                    s.send(Err(e)).expect("Failed to send error");
                    return;
                }
            };

            //let host_handler = handler::create_component_handler();
//
            //let edit_controller = unsafe { Self::initialize_controller(factory, processor_component, host_handler) };
            //let edit_controller = match edit_controller {
            //    Ok(controller) => controller,
            //    Err(e) => {
            //        s.send(Err(e)).expect("Failed to send error");
            //        return;
            //    }
            //};

            let audio_processor = unsafe { Self::initialize_audio_processor(processor_component, max_samples) };
            let audio_processor = match audio_processor {
                Ok(processor) => processor,
                Err(e) => {
                    s.send(Err(e)).expect("Failed to send error");
                    return;
                }
            };


            //let plug_view = unsafe { Self::get_plugin_view(edit_controller) };
            //let plug_view = match plug_view {
            //    Ok(view) => view,
            //    Err(e) => {
            //        s.send(Err(e)).expect("Failed to send error");
            //        return;
            //    }
            //};

            let (kill_s, kill_r) = crossbeam::channel::bounded(1);
            let (command_s, command_r) = crossbeam::channel::unbounded();

            let mut plugin = RawVst3Plugin {
                lib,
                factory: NonNull::new(factory).unwrap(),
                component: NonNull::new(processor_component).unwrap(),
                kill_channel: kill_s,
                command_receiver: command_r,
                plug_view: null_mut(),
                window: None,
            };

            let process_handle = Vst3ProcessHandle {
                audio_processor: NonNull::new(audio_processor).unwrap(),
                kill_channel: kill_r,
                command_sender: command_s,
            };

            s.send(Ok(process_handle)).expect("Failed to send process handle");

            loop {
                if plugin.should_gui_be_open() == Some(true) {
                    if plugin.plug_view.is_null() {
                        tracing::error!("Plugin view is null, cannot open GUI");
                    } else {
                        tracing::info!("Opening GUI window for plugin");
                        let event_loop = EventLoop::new().unwrap();
                        event_loop.set_control_flow(ControlFlow::Poll);
                        event_loop.run_app(&mut plugin).expect("Failed to run event loop");
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

        });

        match r.recv() {
            Ok(result) => {
                // If we successfully received a process handle, set the audio processor to processing
                // This should be done on the audio thread
                match result {
                    Ok(handle) => {
                        let res = unsafe { ((*(*handle.audio_processor.as_ptr()).vtbl).setProcessing)(handle.audio_processor.as_ptr(), 1) };
                        if res != kResultOk {
                            Err("Failed to set audio processor to processing".into())
                        } else {
                            Ok(handle)
                        }
                    },
                    _ => result
                }
            },
            Err(e) => Err(format!("Failed to receive process handle: {}", e)),
        }
    }
}

/// self.plug_view can be assumed to be NonNull since we would not launch a window otherwise
impl ApplicationHandler for RawVst3Plugin {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop.create_window(WindowAttributes::default()).unwrap();
        
        // Attach the plugin view to the window
        let raw_handle = window.window_handle().unwrap().as_raw();
        let attach_res = match raw_handle {
            RawWindowHandle::Win32(handle) => {
                unsafe { ((*(*self.plug_view).vtbl).attached)(self.plug_view, handle.hwnd.get() as *mut std::ffi::c_void, "HWND\0".as_ptr() as *const i8) }
            }
            RawWindowHandle::AppKit(handle) => {
                unsafe { ((*(*self.plug_view).vtbl).attached)(self.plug_view, handle.ns_view.as_ptr(), "NSView\0".as_ptr() as *const i8) }
            }
            RawWindowHandle::Xlib(handle) => {
                unsafe { ((*(*self.plug_view).vtbl).attached)(self.plug_view, handle.window as *mut std::ffi::c_void, "X11EmbedWindowID\0".as_ptr() as *const i8) }
            },
            _ => {
                tracing::error!("Unsupported platform window handle");
                return;
            }
        };

        if attach_res != kResultOk {
            tracing::error!("Failed to attach plugin view to window");
            return;
        }

        self.window = Some(window);
    }

    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = self.window.take() {
            // Detach the plugin view from the window
            unsafe {
                ((*(*self.plug_view).vtbl).removed)(self.plug_view);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(b) = self.should_gui_be_open() {
                    if !b {
                        event_loop.exit();
                    }
                }
            }

            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::Resized(size) => {
                
            },
            _ => {}
        }
    }
}

impl Drop for RawVst3Plugin {
    fn drop(&mut self) {
        self.kill_channel.send(true).expect("Failed to send kill signal");

        unsafe {
            //((*(*self.controller.as_ptr()).vtbl).base.base.release)(self.controller.as_ptr() as *mut FUnknown);
            ((*(*self.component.as_ptr()).vtbl).base.base.release)(self.component.as_ptr() as *mut FUnknown);
            ((*(*self.factory.as_ptr()).vtbl).base.release)(self.factory.as_ptr() as *mut FUnknown);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_plugin() {
        let plugin_path = PathBuf::from(r"C:\Program Files\Common Files\VST3\NA Black.vst3\Contents\x86_64-win\NA Black.vst3");
        let max_samples = 1024;

        let result = RawVst3Plugin::load(plugin_path, max_samples);
        assert!(result.is_ok(), "Failed to load plugin: {:?}", result.err());
        
        let mut plugin = result.unwrap();
        println!("Plugin loaded successfully");
        //plugin.open_gui_window().expect("Failed to open GUI window");
        
        let sample_buffer = &mut vec![0.5; max_samples];
        plugin.process_buffer(sample_buffer);
        println!("Processed {} samples", sample_buffer.len());
        println!(" with output: {:?}", &sample_buffer);
    }
}
