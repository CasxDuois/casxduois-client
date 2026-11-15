//! 5-pass wgpu rendering pipeline (Sky, Solid Terrain, Entity Outline, Translucent, Native UI).
//!
//! Pass structure:
//! - Pass 1: Sky Pass (Sky color and fog based on world time).
//! - Pass 2: Solid & Cutout Terrain Pass (Depth32Float, Back-face Culling, Frustum culling, Indirect Batching).
//! - Pass 3: Entity & Block Outline Pass (Wireframe outline for targeted block via 4.5m DDA Raycast).
//! - Pass 4: Translucent Terrain Pass (Alpha Blending, Depth Write = false for water and stained glass).
//! - Pass 5: Native UI Pass (HUD, Hotbar, Chat, Scoreboard, Launcher overlay).

use crate::frustum::Frustum;
use crate::indirect::IndirectBatchManager;
use crate::shader::VOXEL_SHADER_WGSL;
use crate::vertex::PackedVertex;
use crate::CameraUniform;
use casx_core::physics::Aabb;

/// Pass 1 configuration: Sky and fog parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyPassConfig {
    pub sky_color: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_start: f32,
    pub fog_end: f32,
}

impl Default for SkyPassConfig {
    fn default() -> Self {
        Self {
            sky_color: [0.47, 0.65, 1.0, 1.0], // Daytime blue
            fog_color: [0.75, 0.85, 1.0, 1.0],
            fog_start: 48.0,
            fog_end: 128.0,
        }
    }
}

impl SkyPassConfig {
    /// Computes sky and fog color according to server world time (0..24000 ticks)
    pub fn update_from_world_time(&mut self, world_time: i64) {
        let day_time = (world_time % 24000) as f32;

        if (1000.0..11000.0).contains(&day_time) {
            // Daytime
            self.sky_color = [0.47, 0.65, 1.0, 1.0];
            self.fog_color = [0.75, 0.85, 1.0, 1.0];
        } else if (11000.0..13000.0).contains(&day_time) {
            // Sunset / Sunrise
            self.sky_color = [0.85, 0.45, 0.2, 1.0];
            self.fog_color = [0.65, 0.35, 0.25, 1.0];
        } else {
            // Nighttime
            self.sky_color = [0.03, 0.03, 0.08, 1.0];
            self.fog_color = [0.05, 0.05, 0.1, 1.0];
        }
    }
}

/// Pass 2 configuration: Solid and cutout terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolidTerrainPassConfig {
    pub enable_depth_test: bool,
    pub enable_depth_write: bool,
    pub enable_backface_culling: bool,
}

impl Default for SolidTerrainPassConfig {
    fn default() -> Self {
        Self {
            enable_depth_test: true,
            enable_depth_write: true,
            enable_backface_culling: true,
        }
    }
}

/// Pass 3 configuration: Targeted block outline and entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityOutlinePassConfig {
    pub wireframe_color: [f32; 4],
    pub reach_distance: f32,
    pub active_outline_box: Option<Aabb>,
}

impl Default for EntityOutlinePassConfig {
    fn default() -> Self {
        Self {
            wireframe_color: [0.0, 0.0, 0.0, 0.6],
            reach_distance: 4.5,
            active_outline_box: None,
        }
    }
}

/// Pass 4 configuration: Translucent terrain (Water, stained glass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslucentPassConfig {
    pub enable_alpha_blending: bool,
    pub enable_depth_write: bool,
}

impl Default for TranslucentPassConfig {
    fn default() -> Self {
        Self {
            enable_alpha_blending: true,
            enable_depth_write: false,
        }
    }
}

/// Pass 5 configuration: 2D Native UI overlay pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiPassConfig {
    pub render_hud: bool,
    pub render_crosshair: bool,
}

impl Default for NativeUiPassConfig {
    fn default() -> Self {
        Self {
            render_hud: true,
            render_crosshair: true,
        }
    }
}

/// 5-pass rendering pipeline manager.
#[derive(Debug, Default, Clone)]
pub struct RenderPipeline5Pass {
    pub pass1_sky: SkyPassConfig,
    pub pass2_solid: SolidTerrainPassConfig,
    pub pass3_outline: EntityOutlinePassConfig,
    pub pass4_translucent: TranslucentPassConfig,
    pub pass5_ui: NativeUiPassConfig,
    pub indirect_batch: IndirectBatchManager,
}

impl RenderPipeline5Pass {
    pub fn new() -> Self {
        Self {
            pass1_sky: SkyPassConfig::default(),
            pass2_solid: SolidTerrainPassConfig::default(),
            pass3_outline: EntityOutlinePassConfig::default(),
            pass4_translucent: TranslucentPassConfig::default(),
            pass5_ui: NativeUiPassConfig::default(),
            indirect_batch: IndirectBatchManager::new(),
        }
    }

    /// Sets the bounding box for the targeted block.
    pub fn set_targeted_block_outline(&mut self, aabb: Option<Aabb>) {
        self.pass3_outline.active_outline_box = aabb;
    }

    /// Updates time of day for sky and fog rendering.
    pub fn update_time_of_day(&mut self, time_ticks: i64) {
        self.pass1_sky.update_from_world_time(time_ticks);
    }

    /// Checks if a chunk section is visible within the frustum for Pass 2.
    pub fn is_section_visible(&self, frustum: &Frustum, section_aabb: &Aabb) -> bool {
        frustum.intersects_aabb(section_aabb)
    }
}

/// GPU rendering pipeline and resource bindings for 3D voxel terrain.
pub struct TerrainGpuPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub texture_bind_group: wgpu::BindGroup,
}

impl TerrainGpuPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        _atlas: &casx_assets::TextureAtlas,
        atlas_pixels: &[u8],
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VoxelShader"),
            source: wgpu::ShaderSource::Wgsl(VOXEL_SHADER_WGSL.into()),
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("CameraBindGroupLayout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("TextureBindGroupLayout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VoxelPipelineLayout"),
            bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("VoxelRenderPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[PackedVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CameraUniformBuffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_camera = CameraUniform::default();
        queue.write_buffer(
            &camera_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(&initial_camera)),
        );

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CameraBindGroup"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let layer_count = 32u32;
        let tile_size = 16u32;
        let mut layers_data = vec![0u8; (tile_size * tile_size * 4 * layer_count) as usize];

        for t in 0..layer_count {
            let col = (t % 32) * tile_size;
            let row = (t / 32) * tile_size;
            let dst_layer_offset = (t * tile_size * tile_size * 4) as usize;

            for y in 0..tile_size {
                let src_y = row + y;
                let src_offset = ((src_y * 512 + col) * 4) as usize;
                let dst_offset = dst_layer_offset + (y * tile_size * 4) as usize;
                if src_offset + 64 <= atlas_pixels.len() {
                    layers_data[dst_offset..dst_offset + 64]
                        .copy_from_slice(&atlas_pixels[src_offset..src_offset + 64]);
                }
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("VoxelTextureArray"),
            size: wgpu::Extent3d {
                width: tile_size,
                height: tile_size,
                depth_or_array_layers: layer_count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &layers_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(tile_size * 4),
                rows_per_image: Some(tile_size),
            },
            wgpu::Extent3d {
                width: tile_size,
                height: tile_size,
                depth_or_array_layers: layer_count,
            },
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("VoxelTextureArrayView"),
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(layer_count),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("VoxelTextureSampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TextureBindGroup"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            render_pipeline,
            camera_buffer,
            camera_bind_group,
            texture_bind_group,
        }
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &CameraUniform) {
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(camera)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    #[test]
    fn test_render_pipeline_5_pass_configuration_and_time_update() {
        let mut pipeline = RenderPipeline5Pass::new();

        pipeline.update_time_of_day(6000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.47);

        pipeline.update_time_of_day(18000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.03);

        pipeline.update_time_of_day(12000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.85);

        assert!(!pipeline.pass4_translucent.enable_depth_write);
        assert!(pipeline.pass4_translucent.enable_alpha_blending);

        let box_target = Aabb::new(DVec3::new(1.0, 64.0, 2.0), DVec3::new(2.0, 65.0, 3.0));
        pipeline.set_targeted_block_outline(Some(box_target));
        assert!(pipeline.pass3_outline.active_outline_box.is_some());
    }
}
