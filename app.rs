//app.rs
use wgpu::{BackendOptions, Backends, CompositeAlphaMode, Device, DeviceDescriptor, ExperimentalFeatures, Features, Instance, 
    InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds, PowerPreference, PresentMode, Queue, RequestAdapterOptions, 
    Surface, SurfaceConfiguration, SurfaceTarget, TextureFormat, TextureUsages, MemoryHints, Trace, CurrentSurfaceTexture,
    TextureViewDescriptor,
};
use vello::{Renderer, RendererOptions, AaSupport, RenderParams, AaConfig};
use crate::editor::windowing::HostWindow;
use clap_sys::ext::gui::clap_window;
use wgpu::wgt::TextureDescriptor;
use vello::peniko::Color;
use crate::editor::Size;
use crate::logger::log;
use std::num::NonZero;
use std::time::Instant;


pub struct GpuState {
    pub device: Device,
    pub queue: Queue,
    pub renderer: Renderer,
    pub intermediate_texture: Option<wgpu::Texture>,
}

pub struct WindowState {
    pub surface: Surface<'static>,
    pub config: SurfaceConfiguration,
}

pub struct App {
    pub instance: Option<Instance>,
    pub gpu: Option<GpuState>,
    pub window_state: Option<WindowState>,
    pub window_handle: Option<*const clap_window>,
}

impl App {
    fn create_instance() -> Instance {
        Instance::new(InstanceDescriptor {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::empty(),
            memory_budget_thresholds: MemoryBudgetThresholds {
                for_resource_creation: Some(80),
                for_device_loss: Some(90),
            },
            backend_options: BackendOptions::default(),
            display: None,
        })
    }

    pub fn init(&mut self) { 
        if self.instance.is_none() {
            let t = Instant::now();
            self.instance = Some(Self::create_instance());
            log(&format!("app.rs: init - instance created {:?}", t.elapsed()));
        } else {
            log("app.rs: instance already exist, skipping initialization");
        }
    }

    pub fn get_instance(&mut self) -> &Instance {
        self.instance.get_or_insert_with(Self::create_instance)
    }

    pub fn create_window_state(&mut self, window: *const clap_window, size: Size) {    
        let instance = self.get_instance();
        
        let t = Instant::now();
        let target = HostWindow::from_clap(window)
        .expect("unsupported api");

        let surface = instance.create_surface(
            SurfaceTarget::from(target),
        ).expect("failed to create surface");
        log(&format!("app.rs: create_window_state - surface created {:?}", t.elapsed()));

        let t = Instant::now();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST,
            format: TextureFormat::Rgba8Unorm,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        log(&format!("app.rs: create_window_state - config created {:?}", t.elapsed()));

        self.window_state = Some( WindowState { surface, config } )
    }

    pub async fn setup_gpu(&mut self) {
        let instance = self.instance.as_ref().expect("no instance found");
        let surface = &self.window_state.as_ref().unwrap().surface;
        let t = Instant::now();
        let adapter = instance.request_adapter( &RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.expect("failed to find an appropriate adapter");
        log(&format!("app.rs: setup_gpu - adapter created {:?}", t.elapsed()));
        log(&format!("Adapter name: {:?}", adapter.get_info().name));
        log(&format!("Adapter backend: {:?}", adapter.get_info().backend));
        log(&format!("Adapter device type: {:?}", adapter.get_info().device_type));

        let caps = surface.get_capabilities(&adapter);
        if !caps.usages.contains(TextureUsages::STORAGE_BINDING) {
            log("Warning: Surface does not support STORAGE_BINDING. Vello may fail.");
        }

        let t = Instant::now();
        let limits = adapter.limits();
        log(&format!("app.rs: setup_gpu - getting limits {:?}", t.elapsed()));
        log(&format!("Limits: {:?}", limits));
        log(&format!("Features: {:?}", adapter.features()));

        let t = Instant::now();
        log("setup_gpu: requesting device...");
        let device_future = adapter.request_device(&DeviceDescriptor {
            label: Some("CLAP_Plugin_Device"),
            required_limits: limits,
            required_features: Features::empty(),
            experimental_features: ExperimentalFeatures::disabled(),
            memory_hints: MemoryHints::MemoryUsage,
            trace: Trace::Off,
        });
        log(&format!("setup_gpu: request_device future created {:?}", t.elapsed()));
        let (device, queue) = device_future.await.expect("failed to create device");
        log(&format!("setup_gpu: device ready {:?}", t.elapsed()));

        let t = Instant::now();
        let renderer = Renderer::new(&device, RendererOptions { 
            use_cpu: false, 
            antialiasing_support: AaSupport {
                area: true,
                msaa8: false,
                msaa16: false,
            }, 
            num_init_threads: NonZero::new(1), 
            pipeline_cache: None,
        }).expect("failed to create Vello renderer");
        log(&format!("Renderer::new took {:?}", t.elapsed()));

        let window_state = self.window_state.as_ref().unwrap();

        window_state.surface.configure(
            &device, 
            &window_state.config
        );

        self.gpu = Some(GpuState { 
            device, 
            queue, 
            renderer,
            intermediate_texture: None
        });
    }

    fn config_surface(&mut self){
        let gpu = self.gpu.as_mut().unwrap();
        let window_state = self.window_state.as_ref().unwrap();

        window_state.surface.configure(
            &gpu.device, 
            &window_state.config,
        );

        gpu.intermediate_texture = None;
    }

    pub fn render(&mut self) {
        let gpu = self.gpu.as_mut().unwrap();
        let window = self.window_state.as_mut().unwrap();

        let surface_texture = match window.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                self.config_surface();
                return self.render();
            }
            CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => {
                return;
            }
            CurrentSurfaceTexture::Validation => panic!("Validation error getting surface"),
            CurrentSurfaceTexture::Lost => panic!("Surface was lost")
        };

        let width = window.config.width;
        let height = window.config.height;

        if gpu.intermediate_texture.as_ref().map_or(true, |t| t.width() != width || t.height() != height) {
            log("Recreating intermediate texture...");
            gpu.intermediate_texture = Some(gpu.device.create_texture(&TextureDescriptor {
                label: Some("Vello Intermediate"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: window.config.format,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        };

        let intermediate = gpu.intermediate_texture.as_ref().unwrap();
        let texture = intermediate.create_view( &TextureViewDescriptor::default());

        let scene = crate::editor::scene::AppScene::build(Size { width, height });

        let recreate = if let Some(tex) = &gpu.intermediate_texture {
            tex.width() != width || tex.height() != height
        } else {
            true
        };

        if recreate {
            gpu.intermediate_texture = Some(gpu.device.create_texture(&TextureDescriptor {
                label: Some("Vello Intermediate"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: window.config.format,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                view_formats: &[],
            }))
        }

        let intermediate = gpu.intermediate_texture.as_ref().unwrap();

        gpu.renderer.render_to_texture(
            &gpu.device,
            &gpu.queue,
            &scene,
            &texture,
            &RenderParams { 
                base_color: Color::WHITE,
                width: window.config.width,
                height: window.config.height,
                antialiasing_method: AaConfig::Area,
            },
        ).expect("failed to render scene");

        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Copy Encoder"),
        });

        encoder.copy_texture_to_texture(wgpu::TexelCopyTextureInfo {
                texture: intermediate,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &surface_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }
}