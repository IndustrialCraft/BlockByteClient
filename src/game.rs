use std::{collections::HashMap, ops::AddAssign, sync::Mutex};

use crate::{
    glwrappers::Vertex,
    util::{self, *},
};
use json::JsonValue;
use sdl2::keyboard::Keycode;
use ultraviolet::*;
use uuid::Uuid;

use crate::glwrappers::{self, ModelVertex};

#[derive(Debug, Clone, Copy)]
pub struct ClientPlayer {
    pub position: Vec3,
    velocity: Vec3,
    pub pitch_deg: f32,
    pub yaw_deg: f32,
    shifting: bool,
    shifting_animation: f32,
}
impl ClientPlayer {
    const UP: Vec3 = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub fn is_shifting(&self) -> bool {
        self.shifting
    }
    pub fn make_front(&self) -> Vec3 {
        let pitch_rad = f32::to_radians(self.pitch_deg);
        let yaw_rad = f32::to_radians(self.yaw_deg);
        Vec3 {
            x: yaw_rad.sin() * pitch_rad.cos(),
            y: pitch_rad.sin(),
            z: yaw_rad.cos() * pitch_rad.cos(),
        }
    }
    pub fn update_orientation(&mut self, d_pitch_deg: f32, d_yaw_deg: f32) {
        self.pitch_deg = (self.pitch_deg + d_pitch_deg).max(-89.0).min(89.0);
        self.yaw_deg = (self.yaw_deg + d_yaw_deg) % 360.0;
    }
    pub fn update_position(
        &mut self,
        keys: &std::collections::HashSet<Keycode>,
        delta_time: f32,
        world: &World,
    ) {
        let position = Position::new(self.position);
        if world.get_chunk(position.to_chunk_pos()).is_none() {
            return;
        }
        let mut forward = self.make_front();
        forward.y = 0.0;
        let cross_normalized = forward.cross(Self::UP).normalized();
        let mut move_vector = keys.iter().copied().fold(
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            |vec, key| match key {
                Keycode::W => vec + forward,
                Keycode::S => vec - forward,
                Keycode::A => vec - cross_normalized,
                Keycode::D => vec + cross_normalized,
                _ => vec,
            },
        );
        self.shifting = keys.contains(&Keycode::LShift);
        if keys.contains(&Keycode::Space)
            && ClientPlayer::collides_at(position.add(0., -0.1, 0.), world, self.shifting)
        {
            self.velocity.y = 0.8;
        }
        if !(move_vector.x == 0.0 && move_vector.y == 0.0 && move_vector.z == 0.0) {
            move_vector = move_vector.normalized();
        }
        move_vector /= 2.;
        if self.shifting {
            move_vector /= 2.;
        }
        let mut total_move = (move_vector + self.velocity) * (delta_time * 10f32);
        if ClientPlayer::collides_at(position.add(total_move.x, 0., 0.), world, self.shifting) {
            total_move.x = 0.;
            self.velocity.x = 0.;
        }
        if ClientPlayer::collides_at(
            position.add(total_move.x, total_move.y, 0.),
            world,
            self.shifting,
        ) {
            total_move.y = 0.;
            self.velocity.y = 0.;
        }
        if ClientPlayer::collides_at(
            position.add(total_move.x, total_move.y, total_move.z),
            world,
            self.shifting,
        ) {
            total_move.z = 0.;
            self.velocity.z = 0.;
        }
        if (total_move.x != 0.
            && self.shifting
            && ClientPlayer::collides_at(position.add(0., -0.1, 0.), world, self.shifting))
            && !ClientPlayer::collides_at(
                position.add(total_move.x, -0.1, 0.),
                world,
                self.shifting,
            )
        {
            total_move.x = 0.;
            self.velocity.x = 0.;
        }
        if (total_move.z != 0.
            && self.shifting
            && ClientPlayer::collides_at(
                position.add(total_move.x, -0.1, 0.),
                world,
                self.shifting,
            ))
            && !ClientPlayer::collides_at(
                position.add(total_move.x, -0.1, total_move.z),
                world,
                self.shifting,
            )
        {
            total_move.z = 0.;
            self.velocity.z = 0.;
        }

        self.position += total_move;
        self.velocity.y -= delta_time * 2f32;
        self.shifting_animation += (if self.shifting { 1. } else { -1. }) * delta_time * 4.;
        self.shifting_animation = self.shifting_animation.clamp(0., 0.5);
    }
    fn collides_at(position: util::Position, world: &World, shifting: bool) -> bool {
        let bounding_box = AABB {
            x: position.x - 0.3,
            y: position.y,
            z: position.z - 0.3,
            w: 0.6,
            h: 1.9 - if shifting { 0.5 } else { 0. },
            d: 0.6,
        };
        for block_pos in bounding_box.get_collisions_on_grid() {
            if world
                .get_block(block_pos)
                .map_or(true, |block| block != 0u32)
            {
                return true;
            }
        }
        return false;
    }
    pub const fn at_position(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::new(0., 0., 0.),
            pitch_deg: 0.0,
            yaw_deg: 0.0,
            shifting: false,
            shifting_animation: 0f32,
        }
    }
    fn eye_height_diff(&self) -> f32 {
        1.75 - self.shifting_animation
    }
    pub fn get_eye(&self) -> Position {
        Position::new(self.position).add(0., self.eye_height_diff(), 0.)
    }
    pub fn create_view_matrix(&self) -> ultraviolet::Mat4 {
        Mat4::look_at(
            self.position
                + Vec3 {
                    x: 0.,
                    y: self.eye_height_diff(),
                    z: 0.,
                },
            (self.position
                + Vec3 {
                    x: 0.,
                    y: self.eye_height_diff(),
                    z: 0.,
                })
                + self.make_front(),
            Self::UP,
        )
    }
}

struct AABB {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
    h: f32,
    d: f32,
}
impl AABB {
    pub fn get_collisions_on_grid(&self) -> Vec<BlockPosition> {
        let mut output = Vec::new();
        let first = Position {
            x: self.x,
            y: self.y,
            z: self.z,
        }
        .to_block_pos();
        let second = Position {
            x: self.x + self.w,
            y: self.y + self.h,
            z: self.z + self.d,
        }
        .to_block_pos();
        for x in first.x..=second.x {
            for y in first.y..=second.y {
                for z in first.z..=second.z {
                    output.push(BlockPosition { x, y, z });
                }
            }
        }
        output
    }
}

pub struct Chunk<'a> {
    blocks: [[[u32; 16]; 16]; 16],
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    vertex_count: u32,
    position: ChunkPosition,
    block_registry: &'a BlockRegistry,
    modified: bool,
    loaded: bool,
}
impl<'a> Chunk<'a> {
    pub fn new(position: ChunkPosition, block_registry: &'a BlockRegistry) -> Self {
        let vao = glwrappers::VertexArray::new().expect("couldnt create vao for chunk");
        vao.bind();
        let vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::Vertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::Vertex>()
                    .try_into()
                    .unwrap(),
                12 as *const _,
            );
            ogl33::glVertexAttribIPointer(
                2,
                1,
                ogl33::GL_BYTE,
                std::mem::size_of::<glwrappers::Vertex>()
                    .try_into()
                    .unwrap(),
                20 as *const _,
            );
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        Chunk {
            blocks: [[[0; 16]; 16]; 16],
            vao,
            vbo,
            vertex_count: 0,
            position,
            block_registry,
            modified: false,
            loaded: false,
        }
    }
    pub fn upload_vertices(&mut self, vertices: Vec<glwrappers::Vertex>) {
        self.vertex_count = vertices.len() as u32;
        self.vbo
            .upload_data(bytemuck::cast_slice(&vertices), ogl33::GL_STATIC_DRAW);
    }
    pub fn set_blocks_no_update(&mut self, blocks: [[[u32; 16]; 16]; 16]) {
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    self.blocks[x][y][z] = blocks[x][y][z];
                }
            }
        }
        self.loaded = true;
    }
    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block_type: u32) {
        self.blocks[x as usize][y as usize][z as usize] = block_type;
        self.modified = true;
    }
    pub fn get_block(&self, x: u8, y: u8, z: u8) -> u32 {
        return self.blocks[x as usize][y as usize][z as usize];
    }
    fn rebuild_chunk_mesh(&mut self) {
        let mut vertices: Vec<glwrappers::Vertex> = Vec::new();
        for bx in 0..16i32 {
            let x = bx as f32;
            for by in 0..16i32 {
                let y = by as f32;
                for bz in 0..16i32 {
                    let z = bz as f32;
                    let block_id = self.blocks[bx as usize][by as usize][bz as usize];
                    let block = self.block_registry.get_block(block_id);
                    let position = BlockPosition {
                        x: bx,
                        y: by,
                        z: bz,
                    };
                    match &block.render_type {
                        BlockRenderType::Air => {}
                        BlockRenderType::Cube(north, south, right, left, up, down) => {
                            for face in [
                                Face::Front,
                                Face::Back,
                                Face::Up,
                                Face::Down,
                                Face::Left,
                                Face::Right,
                            ] {
                                let face_offset = face.get_offset();
                                let neighbor_pos = position + face_offset;
                                let neighbor_side_full = if neighbor_pos.is_inside_origin_chunk() {
                                    self.block_registry
                                        .get_block(
                                            self.blocks[neighbor_pos.x as usize]
                                                [neighbor_pos.y as usize]
                                                [neighbor_pos.z as usize],
                                        )
                                        .is_face_full(&face.opposite())
                                } else {
                                    false
                                };
                                let texture = match face {
                                    Face::Front => north,
                                    Face::Back => south,
                                    Face::Right => right,
                                    Face::Left => left,
                                    Face::Up => up,
                                    Face::Down => down,
                                };
                                if !neighbor_side_full {
                                    let face_vertices = face.get_vertices();
                                    let uv = texture.get_coords();
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[0].x + x,
                                        y: face_vertices[0].y + y,
                                        z: face_vertices[0].z + z,
                                        u: uv.0,
                                        v: uv.1,
                                        render_data: block.render_data,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[1].x + x,
                                        y: face_vertices[1].y + y,
                                        z: face_vertices[1].z + z,
                                        u: uv.2,
                                        v: uv.1,
                                        render_data: block.render_data,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[2].x + x,
                                        y: face_vertices[2].y + y,
                                        z: face_vertices[2].z + z,
                                        u: uv.2,
                                        v: uv.3,
                                        render_data: block.render_data,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[2].x + x,
                                        y: face_vertices[2].y + y,
                                        z: face_vertices[2].z + z,
                                        u: uv.2,
                                        v: uv.3,
                                        render_data: block.render_data,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[3].x + x,
                                        y: face_vertices[3].y + y,
                                        z: face_vertices[3].z + z,
                                        u: uv.0,
                                        v: uv.3,
                                        render_data: block.render_data,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[0].x + x,
                                        y: face_vertices[0].y + y,
                                        z: face_vertices[0].z + z,
                                        u: uv.0,
                                        v: uv.1,
                                        render_data: block.render_data,
                                    });
                                }
                            }
                        }
                        BlockRenderType::StaticModel(model, _, _, _, _, _, _) => {
                            model.add_to_chunk_mesh(
                                &mut vertices,
                                block.render_data,
                                BlockPosition {
                                    x: bx,
                                    y: by,
                                    z: bz,
                                },
                            );
                        }
                    }
                }
            }
        }
        self.vertex_count = vertices.len() as u32;
        self.vbo
            .upload_data(bytemuck::cast_slice(&vertices), ogl33::GL_STATIC_DRAW);
    }
    pub fn render(&mut self, shader: &glwrappers::Shader, time: f32) {
        if self.modified {
            self.rebuild_chunk_mesh();
            self.modified = false;
        }
        if self.vertex_count == 0 {
            return;
        }
        self.vao.bind();
        //self.vbo.bind();
        shader.set_uniform_matrix(
            shader
                .get_uniform_location("model\0")
                .expect("uniform model not found"),
            Mat4::from_translation(Vec3 {
                x: (self.position.x * 16) as f32,
                y: (self.position.y * 16) as f32,
                z: (self.position.z * 16) as f32,
            }),
        );
        shader.set_uniform_float(shader.get_uniform_location("time\0").unwrap(), time);
        unsafe {
            ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, self.vertex_count as i32);
        }
    }
}
#[derive(Clone)]
pub enum BlockRenderType {
    Air,
    Cube(
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
    ),
    StaticModel(StaticBlockModel, bool, bool, bool, bool, bool, bool),
}
#[derive(Clone)]
pub struct Block {
    pub render_type: BlockRenderType,
    pub render_data: u8,
}
impl Block {
    pub fn new_air() -> Self {
        Block {
            render_data: 0,
            render_type: BlockRenderType::Air,
        }
    }
    pub fn new_full(texture: AtlassedTexture, render_data: u8) -> Self {
        Block {
            render_type: BlockRenderType::Cube(
                texture, texture, texture, texture, texture, texture,
            ),
            render_data,
        }
    }
    pub fn is_face_full(&self, face: &Face) -> bool {
        match self.render_type {
            BlockRenderType::Air => false,
            BlockRenderType::Cube(_, _, _, _, _, _) => true,
            BlockRenderType::StaticModel(_, north, south, right, left, up, down) => match face {
                Face::Front => north,
                Face::Back => south,
                Face::Left => left,
                Face::Right => right,
                Face::Up => up,
                Face::Down => down,
            },
        }
    }
}
#[derive(Clone)]
pub struct BlockRegistry {
    pub blocks: HashMap<u32, Block>,
}
impl BlockRegistry {
    pub fn get_block(&self, id: u32) -> &Block {
        &self.blocks[&id]
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AtlassedTexture {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub atlas_w: u32,
    pub atlas_h: u32,
}
impl AtlassedTexture {
    pub fn get_coords(&self) -> (f32, f32, f32, f32) {
        (
            (self.x as f32) / (self.atlas_w as f32),
            (self.y as f32) / (self.atlas_h as f32),
            ((self.x + self.w) as f32) / (self.atlas_w as f32),
            ((self.y + self.h) as f32) / (self.atlas_h as f32),
        )
    }
    pub fn map(&self, uv: (f32, f32)) -> (f32, f32) {
        (
            ((self.x as f32) + uv.0) / (self.atlas_w as f32),
            ((self.y as f32) + uv.1) / (self.atlas_h as f32),
        )
    }
    pub fn map_uv(&self, uv: (f32, f32)) -> (f32, f32) {
        (
            ((self.x as f32) + (uv.0 * self.w as f32)) / (self.atlas_w as f32),
            ((self.y as f32) + (uv.1 * self.h as f32)) / (self.atlas_h as f32),
        )
    }
}

pub struct World<'a> {
    chunks: std::collections::HashMap<ChunkPosition, Chunk<'a>>,
    pub blocks_with_items: HashMap<BlockPosition, HashMap<u32, (f32, f32, f32, u32)>>,
    block_registry: &'a BlockRegistry,
}
impl<'a> World<'a> {
    pub fn new(block_registry: &'a BlockRegistry) -> Self {
        World {
            chunks: std::collections::HashMap::new(),
            block_registry,
            blocks_with_items: HashMap::new(),
        }
    }
    pub fn load_chunk(&mut self, position: ChunkPosition) -> &mut Chunk<'a> {
        if !self.chunks.contains_key(&position) {
            self.chunks
                .insert(position, Chunk::new(position, self.block_registry));
        }
        self.chunks.get_mut(&position).unwrap()
    }
    pub fn unload_chunk(&mut self, position: ChunkPosition) {
        self.chunks.remove(&position);
        self.blocks_with_items
            .drain_filter(|pos, _| pos.to_chunk_pos() == position);
    }
    pub fn get_chunk(&self, position: ChunkPosition) -> Option<&Chunk> {
        self.chunks.get(&position)
    }
    pub fn get_mut_chunk(&mut self, position: ChunkPosition) -> Option<&mut Chunk<'a>> {
        self.chunks.get_mut(&position)
    }
    pub fn set_block(&mut self, position: BlockPosition, id: u32) -> Result<(), ()> {
        self.blocks_with_items.remove(&position);
        match self.get_mut_chunk(position.to_chunk_pos()) {
            Some(chunk) => {
                let offset = position.chunk_offset();
                chunk.set_block(offset.0, offset.1, offset.2, id);
                Ok(())
            }
            None => Err(()),
        }
    }
    pub fn get_block(&self, position: BlockPosition) -> Option<u32> {
        self.get_chunk(position.to_chunk_pos())
            .map_or(None, |chunk| {
                if chunk.loaded {
                    let offset = position.chunk_offset();
                    Some(chunk.get_block(offset.0, offset.1, offset.2))
                } else {
                    None
                }
            })
    }
    pub fn render(&mut self, shader: &glwrappers::Shader, time: f32) {
        for chunk in self.chunks.values_mut() {
            chunk.render(shader, time);
        }
    }
}
pub struct Entity {
    pub entity_type: u32,
    pub position: Position,
    pub rotation: f32,
    pub items: HashMap<u32, u32>,
}
#[derive(Clone)]
pub struct BlockModelCube {
    pub from: Position,
    pub to: Position,
    pub north_uv: (f32, f32, f32, f32),
    pub south_uv: (f32, f32, f32, f32),
    pub right_uv: (f32, f32, f32, f32),
    pub left_uv: (f32, f32, f32, f32),
    pub up_uv: (f32, f32, f32, f32),
    pub down_uv: (f32, f32, f32, f32),
}
#[derive(Clone)]
pub struct StaticBlockModel {
    pub cubes: Vec<BlockModelCube>,
}
impl StaticBlockModel {
    pub fn new(json: &JsonValue, texture: &AtlassedTexture) -> Self {
        let mut cubes = Vec::new();
        for element in json["elements"].members() {
            assert_eq!(element["type"], "cube");
            let from = EntityModel::parse_array_into_position(&element["from"]);
            let to = EntityModel::parse_array_into_position(&element["to"]);
            let faces = &element["faces"];
            cubes.push(BlockModelCube {
                from,
                to,
                north_uv: EntityModel::parse_uv(&faces["north"], texture),
                south_uv: EntityModel::parse_uv(&faces["south"], texture),
                right_uv: EntityModel::parse_uv(&faces["east"], texture),
                left_uv: EntityModel::parse_uv(&faces["west"], texture),
                up_uv: EntityModel::parse_uv(&faces["up"], texture),
                down_uv: EntityModel::parse_uv(&faces["down"], texture),
            });
        }
        StaticBlockModel { cubes }
    }
    pub fn add_to_chunk_mesh(
        &self,
        vertices: &mut Vec<Vertex>,
        render_data: u8,
        position: BlockPosition,
    ) {
        for cube in &self.cubes {
            StaticBlockModel::create_cube(
                vertices,
                cube.from,
                cube.to,
                cube.north_uv,
                cube.south_uv,
                cube.up_uv,
                cube.down_uv,
                cube.left_uv,
                cube.right_uv,
                render_data,
                position,
            );
        }
    }
    fn create_cube(
        vertices: &mut Vec<Vertex>,
        from: Position,
        to: Position,
        north: (f32, f32, f32, f32),
        south: (f32, f32, f32, f32),
        up: (f32, f32, f32, f32),
        down: (f32, f32, f32, f32),
        west: (f32, f32, f32, f32),
        east: (f32, f32, f32, f32),
        render_data: u8,
        position: BlockPosition,
    ) {
        let from = Position {
            x: (from.x) + position.x as f32,
            y: (from.y) + position.y as f32,
            z: (from.z) + position.z as f32,
        };
        let to = Position {
            x: (to.x) + position.x as f32,
            y: (to.y) + position.y as f32,
            z: (to.z) + position.z as f32,
        };
        let size = Position {
            x: to.x - from.x,
            y: to.y - from.y,
            z: to.z - from.z,
        };
        StaticBlockModel::create_face(
            vertices,
            from,
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y,
                z: from.z + size.z,
            },
            down,
            render_data,
        );
        StaticBlockModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            up,
            render_data,
        );
        StaticBlockModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z,
            },
            west,
            render_data,
        );
        StaticBlockModel::create_face(
            vertices,
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z,
            },
            east,
            render_data,
        );
        StaticBlockModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z,
            },
            north,
            render_data,
        );
        StaticBlockModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            south,
            render_data,
        );
    }
    fn create_face(
        vertices: &mut Vec<Vertex>,
        p1: Position,
        p2: Position,
        p3: Position,
        p4: Position,
        uv: (f32, f32, f32, f32),
        render_data: u8,
    ) {
        let v1 = Vertex {
            x: p1.x,
            y: p1.y,
            z: p1.z,
            u: uv.0,
            v: uv.1,
            render_data,
        };
        let v2 = Vertex {
            x: p2.x,
            y: p2.y,
            z: p2.z,
            u: uv.2,
            v: uv.1,
            render_data,
        };
        let v3 = Vertex {
            x: p3.x,
            y: p3.y,
            z: p3.z,
            u: uv.2,
            v: uv.3,
            render_data,
        };
        let v4 = Vertex {
            x: p4.x,
            y: p4.y,
            z: p4.z,
            u: uv.0,
            v: uv.3,
            render_data,
        };
        vertices.push(v1);
        vertices.push(v2);
        vertices.push(v3);
        vertices.push(v3);
        vertices.push(v4);
        vertices.push(v1);
    }
}
pub struct EntityModel {
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    vertex_count: u32,
    bones: HashMap<Uuid, ModelBone>,
    pub render_data: EntityRenderData,
}
struct ModelBone {
    id: Uuid,
    render_id: u16,
    parent: Option<Uuid>,
    children: Vec<Uuid>,
}
impl ModelBone {
    pub fn load(json: &json::JsonValue) -> (HashMap<Uuid, ModelBone>, HashMap<Uuid, Uuid>) {
        let mut id_generator = 0u16;
        let mut map = HashMap::new();
        let mut render_map = HashMap::new();
        ModelBone::load_children(&mut map, &mut render_map, json, None, &mut id_generator);
        (map, render_map)
    }
    pub fn get_render_id(&self) -> u16 {
        self.render_id
    }
    fn load_children(
        map: &mut HashMap<Uuid, ModelBone>,
        render_map: &mut HashMap<Uuid, Uuid>,
        json: &json::JsonValue,
        parent: Option<Uuid>,
        id_generator: &mut u16,
    ) -> Vec<Uuid> {
        let mut children = Vec::new();
        for bone in json.members() {
            if bone.is_string() {
                render_map.insert(
                    Uuid::try_parse(bone.as_str().unwrap()).unwrap(),
                    parent.expect("root node can only have other nodes"),
                );
            } else {
                let uuid = Uuid::try_parse(bone["uuid"].as_str().unwrap()).unwrap();
                let id = *id_generator;
                id_generator.add_assign(1);
                let bone_children = ModelBone::load_children(
                    map,
                    render_map,
                    &bone["children"],
                    Some(uuid),
                    id_generator,
                );
                map.insert(
                    uuid,
                    ModelBone {
                        id: uuid,
                        render_id: id,
                        parent,
                        children: bone_children,
                    },
                );
                children.push(uuid);
            }
        }
        children
    }
}
impl EntityModel {
    pub fn new(
        json: json::JsonValue,
        texture_atlas: &AtlassedTexture,
        render_data: EntityRenderData,
    ) -> Self {
        let vao = glwrappers::VertexArray::new().expect("couldnt create vao for entity renderer");
        vao.bind();
        let mut vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::ModelVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::ModelVertex>()
                    .try_into()
                    .unwrap(),
                12 as *const _,
            );
            ogl33::glVertexAttribIPointer(
                2,
                1,
                ogl33::GL_SHORT,
                std::mem::size_of::<glwrappers::ModelVertex>()
                    .try_into()
                    .unwrap(),
                20 as *const _,
            );
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        let mut vertices: Vec<ModelVertex> = Vec::new();
        let (bones, bones_render_data) = ModelBone::load(&json["outliner"]);
        for element in json["elements"].members() {
            assert_eq!(element["type"], "cube");
            let uuid = Uuid::try_parse(element["uuid"].as_str().unwrap()).unwrap();
            let from = EntityModel::parse_array_into_position(&element["from"]);
            let to = EntityModel::parse_array_into_position(&element["to"]);
            let faces = &element["faces"];
            EntityModel::create_cube(
                &mut vertices,
                from,
                to,
                EntityModel::parse_uv(&faces["north"], texture_atlas),
                EntityModel::parse_uv(&faces["south"], texture_atlas),
                EntityModel::parse_uv(&faces["up"], texture_atlas),
                EntityModel::parse_uv(&faces["down"], texture_atlas),
                EntityModel::parse_uv(&faces["west"], texture_atlas),
                EntityModel::parse_uv(&faces["east"], texture_atlas),
                bones[&bones_render_data[&uuid]].get_render_id(),
            );
        }
        vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STATIC_DRAW,
        );
        EntityModel {
            vao,
            vbo,
            vertex_count: vertices.len() as u32,
            bones,
            render_data,
        }
    }
    pub fn render(&self, position: Position, rotation: f32, shader: &glwrappers::Shader) {
        if self.vertex_count == 0 {
            return;
        }
        self.vao.bind();
        self.vbo.bind();
        shader.set_uniform_matrix(
            shader
                .get_uniform_location("model\0")
                .expect("uniform model not found"),
            ultraviolet::Mat4::from_translation(ultraviolet::Vec3 {
                x: position.x,
                y: position.y,
                z: position.z,
            }) * ultraviolet::Mat4::from_rotation_y(rotation),
        );
        let mut bone_matrices = Vec::new();
        //todo
        for _bone in &self.bones {
            bone_matrices.push(ultraviolet::Mat4::from_translation(ultraviolet::Vec3 {
                x: 0.,
                y: 0.,
                z: 0., /*i as f32*/
            }));
        }
        shader.set_uniform_matrices(
            shader.get_uniform_location("bones\0").unwrap(),
            bone_matrices,
        );
        unsafe {
            ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, self.vertex_count as i32);
        }
    }
    fn parse_uv(json: &json::JsonValue, texture: &AtlassedTexture) -> (f32, f32, f32, f32) {
        let json = &json["uv"];
        assert_eq!(json.len(), 4);
        let uv1 = texture.map((json[0].as_f32().unwrap(), json[1].as_f32().unwrap()));
        let uv2 = texture.map((json[2].as_f32().unwrap(), json[3].as_f32().unwrap()));
        (uv2.0, uv2.1, uv1.0, uv1.1)
    }
    fn parse_array_into_position(json: &json::JsonValue) -> Position {
        assert_eq!(json.len(), 3);
        Position {
            x: (json[0].as_f32().unwrap() / 16.) + 0.5,
            y: json[1].as_f32().unwrap() / 16.,
            z: (json[2].as_f32().unwrap() / 16.) + 0.5,
        }
    }
    fn create_cube(
        vertices: &mut Vec<ModelVertex>,
        from: Position,
        to: Position,
        north: (f32, f32, f32, f32),
        south: (f32, f32, f32, f32),
        up: (f32, f32, f32, f32),
        down: (f32, f32, f32, f32),
        west: (f32, f32, f32, f32),
        east: (f32, f32, f32, f32),
        bone_id: u16,
    ) {
        let size = Position {
            x: to.x - from.x,
            y: to.y - from.y,
            z: to.z - from.z,
        };
        EntityModel::create_face(
            vertices,
            from,
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y,
                z: from.z + size.z,
            },
            down,
            bone_id,
        );
        EntityModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            up,
            bone_id,
        );
        EntityModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z,
            },
            west,
            bone_id,
        );
        EntityModel::create_face(
            vertices,
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z,
            },
            east,
            bone_id,
        );
        EntityModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z,
            },
            north,
            bone_id,
        );
        EntityModel::create_face(
            vertices,
            Position {
                x: from.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x + size.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            Position {
                x: from.x,
                y: from.y + size.y,
                z: from.z + size.z,
            },
            south,
            bone_id,
        );
    }
    fn create_face(
        vertices: &mut Vec<ModelVertex>,
        p1: Position,
        p2: Position,
        p3: Position,
        p4: Position,
        uv: (f32, f32, f32, f32),
        bone_id: u16,
    ) {
        let v1 = ModelVertex {
            x: p1.x,
            y: p1.y,
            z: p1.z,
            u: uv.0,
            v: uv.1,
            render_data: bone_id,
        };
        let v2 = ModelVertex {
            x: p2.x,
            y: p2.y,
            z: p2.z,
            u: uv.2,
            v: uv.1,
            render_data: bone_id,
        };
        let v3 = ModelVertex {
            x: p3.x,
            y: p3.y,
            z: p3.z,
            u: uv.2,
            v: uv.3,
            render_data: bone_id,
        };
        let v4 = ModelVertex {
            x: p4.x,
            y: p4.y,
            z: p4.z,
            u: uv.0,
            v: uv.3,
            render_data: bone_id,
        };
        vertices.push(v1);
        vertices.push(v2);
        vertices.push(v3);
        vertices.push(v3);
        vertices.push(v4);
        vertices.push(v1);
    }
}
