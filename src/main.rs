slint::include_modules!();

type DynResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

use slint::wgpu_24::WGPUConfiguration;
use slint::wgpu_24::wgpu;
use slint::{GraphicsAPI, Image, RenderingState};

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    displayed_texture: Option<wgpu::Texture>,
    next_texture: Option<wgpu::Texture>,
}

impl Renderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&Default::default());

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            displayed_texture: None,
            next_texture: None,
        }
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn render(&mut self, width: u32, height: u32) -> wgpu::Texture {
        if self.next_texture.is_none() {
            self.next_texture = Some(Self::create_texture(&self.device, width, height));
        };

        let Some(next_texture) = self.next_texture.as_mut() else { todo!() };

        if next_texture.size().width != width || next_texture.size().height != height {
            let mut new_texture = Self::create_texture(&self.device, width, height);
            std::mem::swap(next_texture, &mut new_texture);
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &next_texture.create_view(&Default::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        let result_texture = next_texture.clone();

        std::mem::swap(&mut self.next_texture, &mut self.displayed_texture);

        result_texture
    }
}

fn main() -> DynResult<()> {
    slint::BackendSelector::new()
        .require_wgpu_24(WGPUConfiguration::Automatic(Default::default()))
        .select()?;

    let app = App::new()?;
    let app_weak = app.as_weak();

    let mut underlay = None;

    app.window()
        .set_rendering_notifier(move |state, graphics_api| match state {
            RenderingState::RenderingSetup => {
                let GraphicsAPI::WGPU24 { device, queue, .. } = graphics_api else { todo!() };
                underlay = Some(Renderer::new(device, queue));
            }
            RenderingState::BeforeRendering => {
                if let (Some(underlay), Some(app)) = (underlay.as_mut(), app_weak.upgrade()) {
                    let texture = underlay.render(
                        app.get_requested_texture_width() as u32,
                        app.get_requested_texture_height() as u32,
                    );
                    app.set_texture(Image::try_from(texture).unwrap());
                    app.window().request_redraw();
                }
            }
            RenderingState::AfterRendering => {}
            RenderingState::RenderingTeardown => drop(underlay.take()),
            _ => todo!(),
        })?;

    Ok(app.run()?)
}
