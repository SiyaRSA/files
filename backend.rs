use wgpu::{CompositeAlphaMode, Device, ExperimentalFeatures, Features, Instance, Limits, Operations, PresentMode, 
    Queue, RenderPassColorAttachment, RenderPassDescriptor, Surface, SurfaceConfiguration, TextureUsages};
use egui_wgpu::{Renderer, RendererOptions};
use clap_sys::ext::gui::clap_window;
use egui::Context;


pub struct GpuState {
    pub renderer: Renderer,
    pub surface: Surface<'static>,
    pub device: Device,
    pub config: SurfaceConfiguration,
    pub ctx: Context,
    pub queue: Queue,
}

impl GpuState {

    pub fn create_instance(window: *const clap_window, width: u32, height: u32) -> Box<GpuState> {
        pollster::block_on(async {
            let instance = Instance::default();

            #[cfg(target_os = "windows")]
            let handle = unsafe { (*window).specific.win32 };
            #[cfg(target_os = "macos")]
            let handle = unsafe { (*window).specific.cocoa };
            #[cfg(target_os = "linux")]
            let handle = unsafe { (*window).specific.x11 };

            let surface = unsafe {
                instance.create_surface_unsafe(
                    wgpu::SurfaceTargetUnsafe::SurfaceHandle(handle)
                )
            }.expect("Failed to create surface");

            let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Faild to find adapter");

            let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::defaults(),
                experimental_features: ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to create device");

            let renderer = Renderer::new(
                &device, 
                wgpu::TextureFormat::Bgra8UnormSrgb, 
                RendererOptions{
                    msaa_samples: 1,
                    depth_stencil_format: Some(wgpu::TextureFormat::Depth24PlusStencil8),
                    dithering: true,
                    predictable_texture_filtering: true,
                }
            );

            let config = SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: surface.get_capabilities(&adapter).formats[0],
                width: width,
                height: height,
                present_mode: PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: CompositeAlphaMode::Auto,
                view_formats: vec![],
            };

            surface.configure(&device, &config);

            Box::new(GpuState { 
                renderer,
                surface,  
                device, 
                queue,
                config,
                ctx: Context::default(),
            })
        })
    }

    pub fn frame(&mut self, width: u32, height: u32) {
        let raw_input = egui::RawInput::default();
        
        let output = self.ctx.run_ui(raw_input, |ui| {
            crate::editor::gui::draw_ui(ui);
        });

        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(&self.device, &self.queue, *id, delta);
        }

        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        let paint_jobs = self.ctx.tessellate(
            output.shapes, 
            output.pixels_per_point
        );

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: output.pixels_per_point,
        };

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => { frame }

            wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }

            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }

            wgpu::CurrentSurfaceTexture::Validation => { return; }
        };

        let view = frame
        .texture
        .create_view(&wgpu::wgt::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        self.renderer.update_buffers(
            &self.device, 
            &self.queue, 
            &mut encoder, 
            &paint_jobs, 
            &screen_descriptor
        );

        {
            let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            
            self.renderer.render(
                &mut render_pass.forget_lifetime(), 
                &paint_jobs, 
                &screen_descriptor
            );
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}