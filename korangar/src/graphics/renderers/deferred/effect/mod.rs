use std::sync::Arc;

use bytemuck::{cast_slice, Pod, Zeroable};
use cgmath::{ElementWise, Matrix2, Vector2};
use wgpu::{
    include_wgsl, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource,
    BindingType, ColorTargetState, ColorWrites, Device, FragmentState, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PushConstantRange, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, ShaderModule,
    ShaderModuleDescriptor, ShaderStages, TextureFormat, TextureSampleType, TextureViewDimension, VertexState,
};

use super::{DeferredRenderer, DeferredSubRenderer};
use crate::graphics::renderers::sampler::{create_new_sampler, SamplerType};
use crate::graphics::{Color, Renderer, Texture, EFFECT_ATTACHMENT_BLEND};
use crate::interface::layout::ScreenSize;

const SHADER: ShaderModuleDescriptor = include_wgsl!("effect.wgsl");

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Constants {
    top_left: [f32; 2],
    bottom_left: [f32; 2],
    top_right: [f32; 2],
    bottom_right: [f32; 2],
    texture_top_left: [f32; 2],
    texture_bottom_left: [f32; 2],
    texture_top_right: [f32; 2],
    texture_bottom_right: [f32; 2],
    // Needs to be stored in two arrays,
    // or else we get alignment problems.
    color0: [f32; 2],
    color1: [f32; 2],
}

pub struct EffectRenderer {
    device: Arc<Device>,
    shader_module: ShaderModule,
    linear_sampler: Sampler,
    bind_group_layout: BindGroupLayout,
    pipeline: RenderPipeline,
}

impl EffectRenderer {
    pub fn new(device: Arc<Device>, surface_format: TextureFormat) -> Self {
        let shader_module = device.create_shader_module(SHADER);
        let linear_sampler = create_new_sampler(&device, "effect linear", SamplerType::Linear);

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("effect"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline = Self::create_pipeline(&device, &shader_module, &bind_group_layout, surface_format);

        Self {
            device,
            shader_module,
            linear_sampler,
            bind_group_layout,
            pipeline,
        }
    }

    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    pub fn recreate_pipeline(&mut self, surface_texture: TextureFormat) {
        self.pipeline = Self::create_pipeline(&self.device, &self.shader_module, &self.bind_group_layout, surface_texture);
    }

    fn create_pipeline(
        device: &Device,
        shader_module: &ShaderModule,
        bind_group_layout: &BindGroupLayout,
        surface_format: TextureFormat,
    ) -> RenderPipeline {
        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("effect"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[PushConstantRange {
                stages: ShaderStages::VERTEX_FRAGMENT,
                range: 0..size_of::<Constants>() as _,
            }],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("effect"),
            layout: Some(&layout),
            vertex: VertexState {
                module: shader_module,
                entry_point: "vs_main",
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(FragmentState {
                module: shader_module,
                entry_point: "fs_main",
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(EFFECT_ATTACHMENT_BLEND),
                    write_mask: ColorWrites::default(),
                })],
            }),
            multiview: None,
            cache: None,
        })
    }

    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    fn bind_pipeline(&self, render_pass: &mut RenderPass) {
        render_pass.set_pipeline(&self.pipeline);
    }

    #[cfg_attr(feature = "debug", korangar_debug::profile("render effect"))]
    pub fn render(
        &self,
        render_target: &mut <DeferredRenderer as Renderer>::Target,
        render_pass: &mut RenderPass,
        texture: &Texture,
        window_size: ScreenSize,
        corner_screen_position: [Vector2<f32>; 4],
        texture_coordinates: [Vector2<f32>; 4],
        screen_space_position: Vector2<f32>,
        offset: Vector2<f32>,
        angle: f32,
        color: Color,
    ) {
        if render_target.bound_sub_renderer(DeferredSubRenderer::Effect) {
            self.bind_pipeline(render_pass);
        }

        let half_screen = Vector2::new(window_size.width / 2.0, window_size.height / 2.0);
        // TODO: move this calculation to the loading
        let rotation_matrix = Matrix2::from_angle(cgmath::Deg(angle / (1024.0 / 360.0)));

        let clip_positions = corner_screen_position.map(|position| {
            // TODO: NHA Black magic calculation of the "corner_position". Later this should
            //       not render in screen space, but properly in world space.
            const EFFECT_ORIGIN: Vector2<f32> = Vector2::new(-1600.0, -700.0);
            let corner_position = ((rotation_matrix * position) + offset - EFFECT_ORIGIN - half_screen).div_element_wise(half_screen);

            // This converts the screen space coordinate to the clip space.
            let position = (screen_space_position * 2.0) + corner_position;
            [position.x - 1.0, -position.y + 1.0]
        });

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("effect"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(texture.get_texture_view()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        });

        let color = color.components_linear();

        // TODO: apply angle
        let push_constants = Constants {
            top_left: clip_positions[0],
            bottom_left: clip_positions[2],
            top_right: clip_positions[1],
            bottom_right: clip_positions[3],
            texture_top_left: texture_coordinates[2].into(),
            texture_bottom_left: texture_coordinates[3].into(),
            texture_top_right: texture_coordinates[1].into(),
            texture_bottom_right: texture_coordinates[0].into(),
            color0: [color[0], color[1]],
            color1: [color[2], color[3]],
        };

        render_pass.set_push_constants(ShaderStages::VERTEX_FRAGMENT, 0, cast_slice(&[push_constants]));
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}
