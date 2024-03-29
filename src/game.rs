use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{BTreeSet, HashMap},
    hash::Hash,
    ops::AddAssign,
    os,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{
    glwrappers::{Vertex, VertexArray},
    model::{self, Model},
    util::{self, *},
    TextureAtlas,
};
use alto::{Alto, OutputDevice, Source};
use hashbrown::HashSet;
use indexmap::IndexMap;
use json::JsonValue;
use ogl33::GL_CULL_FACE;
use rustc_hash::{FxHashMap, FxHashSet};
use sdl2::keyboard::Keycode;
use ultraviolet::*;
use uuid::Uuid;

use crate::glwrappers::{self, ModelVertex};

#[derive(Clone, Copy)]
pub struct ClientPlayer<'a> {
    pub position: Vec3,
    pub velocity: Vec3,
    pub pitch_deg: f32,
    pub yaw_deg: f32,
    shifting: bool,
    shifting_animation: f32,
    block_registry: &'a BlockRegistry,
    pub last_moved: bool,
    pub speed: f32,
    pub movement_type: MovementType,
}
impl<'a> ClientPlayer<'a> {
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
    pub fn knockback(&mut self, x: f32, y: f32, z: f32, set: bool) {
        if set {
            self.velocity = Vec3::zero();
        }
        self.velocity += Vec3::new(x, y, z);
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
        forward.y = 0.;
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

        if !(move_vector.x == 0.0 && move_vector.y == 0.0 && move_vector.z == 0.0) {
            move_vector = move_vector.normalized();
        }
        if self.shifting {
            move_vector /= 2.;
        }
        if self.movement_type == MovementType::Normal {
            if keys.contains(&Keycode::Space) {
                let block = world.get_block(position.to_block_pos()).unwrap();
                let block = self.block_registry.get_block(block);
                if block.fluid {
                    move_vector.y += 1.;
                    self.velocity.y = 0.;
                } else {
                    if ClientPlayer::collides_at(
                        self.movement_type,
                        self.block_registry,
                        position.add(0., -0.2, 0.),
                        world,
                        self.shifting,
                    ) {
                        self.velocity.y = 5.5;
                    }
                }
            }
        } else {
            if keys.contains(&Keycode::Space) {
                move_vector.y += 1.;
            }
            if keys.contains(&Keycode::LShift) {
                move_vector.y -= 1.;
            }
        }
        move_vector *= self.speed;
        move_vector *= 5.;

        let mut total_move = (move_vector + self.velocity) * delta_time;

        self.last_moved = move_vector.mag() > 0.;

        if (total_move.x != 0.
            && self.shifting
            && ClientPlayer::collides_at(
                self.movement_type,
                self.block_registry,
                position.add(0., -0.1, 0.),
                world,
                self.shifting,
            ))
            && !ClientPlayer::collides_at(
                self.movement_type,
                self.block_registry,
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
                self.movement_type,
                self.block_registry,
                position.add(total_move.x, -0.1, 0.),
                world,
                self.shifting,
            ))
            && !ClientPlayer::collides_at(
                self.movement_type,
                self.block_registry,
                position.add(total_move.x, -0.1, total_move.z),
                world,
                self.shifting,
            )
        {
            total_move.z = 0.;
            self.velocity.z = 0.;
        }

        if ClientPlayer::collides_at(
            self.movement_type,
            self.block_registry,
            position.add(total_move.x, 0., 0.),
            world,
            self.shifting,
        ) {
            total_move.x = 0.;
            self.velocity.x = 0.;
        }
        if ClientPlayer::collides_at(
            self.movement_type,
            self.block_registry,
            position.add(total_move.x, total_move.y, 0.),
            world,
            self.shifting,
        ) {
            total_move.y = 0.;
            self.velocity.y = 0.;
        }
        if ClientPlayer::collides_at(
            self.movement_type,
            self.block_registry,
            position.add(total_move.x, total_move.y, total_move.z),
            world,
            self.shifting,
        ) {
            total_move.z = 0.;
            self.velocity.z = 0.;
        }
        let drag_coefficient = 0.025;
        let drag = self.velocity
            * self.velocity
            * Vec3 {
                x: 1f32.copysign(self.velocity.x),
                y: 1f32.copysign(self.velocity.y),
                z: 1f32.copysign(self.velocity.z),
            }
            * drag_coefficient;
        self.velocity -= drag * delta_time;
        self.position += total_move;
        if self.movement_type == MovementType::Normal {
            self.velocity.y -= delta_time * 15f32;
        }
        self.shifting_animation += (if self.shifting { 1. } else { -1. }) * delta_time * 4.;
        self.shifting_animation = self.shifting_animation.clamp(0., 0.5);
    }
    fn collides_at(
        movement_type: MovementType,
        block_registry: &BlockRegistry,
        position: util::Position,
        world: &World,
        shifting: bool,
    ) -> bool {
        if movement_type == MovementType::NoClip {
            return false;
        }
        let bounding_box = AABB {
            x: position.x - 0.3,
            y: position.y,
            z: position.z - 0.3,
            w: 0.6,
            h: 1.9 - if shifting { 0.5 } else { 0. },
            d: 0.6,
        };
        for block_pos in bounding_box.get_collisions_on_grid() {
            if world.get_block(block_pos).map_or(true, |block| {
                let block = block_registry.get_block(block);
                !block.fluid && !block.no_collision
            }) {
                return true;
            }
        }
        return false;
    }
    pub const fn at_position(position: Vec3, block_registry: &'a BlockRegistry) -> Self {
        Self {
            position,
            velocity: Vec3::new(0., 0., 0.),
            pitch_deg: 0.0,
            yaw_deg: 0.0,
            shifting: false,
            shifting_animation: 0f32,
            block_registry,
            last_moved: false,
            speed: 1.,
            movement_type: MovementType::Normal,
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
    pub fn create_view_matrix_no_pos(&self) -> ultraviolet::Mat4 {
        Mat4::look_at(
            Vec3 {
                x: 0.,
                y: 0.,
                z: 0.,
            },
            self.make_front(),
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
pub struct DynamicBlockData {
    pub id: u32,
    pub animation: Option<(u32, f32)>,
    pub items: HashMap<u32, ItemSlot>,
}

pub struct Chunk<'a> {
    blocks: [[[u32; 16]; 16]; 16],
    light: [[[u16; 16]; 16]; 16],
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    vertex_count: u32,
    transparent_vao: glwrappers::VertexArray,
    transparent_vbo: glwrappers::Buffer,
    transparent_vertex_count: u32,
    foliage_vao: glwrappers::VertexArray,
    foliage_vbo: glwrappers::Buffer,
    foliage_vertex_count: u32,
    position: ChunkPosition,
    block_registry: &'a BlockRegistry,
    front: Option<Rc<RefCell<Chunk<'a>>>>,
    back: Option<Rc<RefCell<Chunk<'a>>>>,
    left: Option<Rc<RefCell<Chunk<'a>>>>,
    right: Option<Rc<RefCell<Chunk<'a>>>>,
    up: Option<Rc<RefCell<Chunk<'a>>>>,
    down: Option<Rc<RefCell<Chunk<'a>>>>,
    pub dynamic_blocks: HashMap<BlockPosition, DynamicBlockData>,
}
impl<'a> Chunk<'a> {
    fn set_neighbor_by_face(&mut self, face: &Face, chunk: Option<Rc<RefCell<Chunk<'a>>>>) {
        match face {
            Face::Front => self.front = chunk,
            Face::Back => self.back = chunk,
            Face::Left => self.left = chunk,
            Face::Right => self.right = chunk,
            Face::Up => self.up = chunk,
            Face::Down => self.down = chunk,
        }
    }
    pub fn new(
        position: ChunkPosition,
        block_registry: &'a BlockRegistry,
        blocks: [[[u32; 16]; 16]; 16],
        world: &mut World,
    ) -> Self {
        let mut dynamic_blocks = HashMap::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    let id = blocks[x as usize][y as usize][z as usize];
                    let block = block_registry.get_block(id);
                    let block_position = BlockPosition {
                        x: position.x * 16 + x,
                        y: position.y * 16 + y,
                        z: position.z * 16 + z,
                    };
                    if block.is_light_emmiting() {
                        world.light_updates.insert(block_position);
                    }
                    if block.dynamic.is_some() {
                        dynamic_blocks.insert(
                            block_position,
                            DynamicBlockData {
                                id,
                                animation: None,
                                items: HashMap::new(),
                            },
                        );
                    }
                }
            }
        }
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
            ogl33::glVertexAttribIPointer(
                3,
                1,
                ogl33::GL_SHORT,
                std::mem::size_of::<glwrappers::Vertex>()
                    .try_into()
                    .unwrap(),
                21 as *const _,
            );
            ogl33::glEnableVertexAttribArray(3);
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        let transparent_vao = glwrappers::VertexArray::new().expect("couldnt create vao for chunk");
        transparent_vao.bind();
        let transparent_vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        transparent_vbo.bind();
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
            ogl33::glVertexAttribIPointer(
                3,
                1,
                ogl33::GL_SHORT,
                std::mem::size_of::<glwrappers::Vertex>()
                    .try_into()
                    .unwrap(),
                21 as *const _,
            );
            ogl33::glEnableVertexAttribArray(3);
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        let foliage_vao = glwrappers::VertexArray::new().expect("couldnt create vao for chunk");
        foliage_vao.bind();
        let foliage_vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        foliage_vbo.bind();
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
            ogl33::glVertexAttribIPointer(
                3,
                1,
                ogl33::GL_SHORT,
                std::mem::size_of::<glwrappers::Vertex>()
                    .try_into()
                    .unwrap(),
                21 as *const _,
            );
            ogl33::glEnableVertexAttribArray(3);
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        Chunk {
            blocks,
            light: [[[15 << 12; 16]; 16]; 16],
            vao,
            vbo,
            vertex_count: 0,
            position,
            block_registry,
            transparent_vbo,
            transparent_vao,
            transparent_vertex_count: 0,
            foliage_vbo,
            foliage_vao,
            foliage_vertex_count: 0,
            front: None,
            back: None,
            left: None,
            right: None,
            up: None,
            down: None,
            dynamic_blocks,
        }
    }
    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block_type: u32, world: &mut World) {
        let position = BlockPosition {
            x: (self.position.x * 16) + x as i32,
            y: (self.position.y * 16) + y as i32,
            z: (self.position.z * 16) + z as i32,
        };
        self.dynamic_blocks.remove(&position);
        self.blocks[x as usize][y as usize][z as usize] = block_type;
        world.chunk_mesh_updates.insert(self.position);
        if self.block_registry.get_block(block_type).dynamic.is_some() {
            self.dynamic_blocks.insert(
                position,
                DynamicBlockData {
                    id: block_type,
                    animation: None,
                    items: HashMap::new(),
                },
            );
        }
    }
    pub fn schedule_mesh_rebuild(&self, world: &mut World) {
        world.chunk_mesh_updates.insert(self.position);
    }
    pub fn get_block(&self, x: u8, y: u8, z: u8) -> u32 {
        return self.blocks[x as usize][y as usize][z as usize];
    }
    pub fn get_light(&self, x: u8, y: u8, z: u8) -> (u8, u8, u8) {
        let light = self.light[x as usize][y as usize][z as usize];
        (
            (light & 15) as u8,
            ((light >> 4) & 15) as u8,
            ((light >> 8) & 15) as u8,
            //((light >> 12) & 15) as u8,
        )
    }
    fn rebuild_chunk_mesh(&mut self, this: Rc<RefCell<Chunk<'a>>>, world: &mut World<'a>) {
        if self.front.is_none()
            || self.back.is_none()
            || self.left.is_none()
            || self.right.is_none()
            || self.up.is_none()
            || self.down.is_none()
        {
            return;
        }
        let front_chunk = self.front.as_deref().unwrap().borrow();
        let back_chunk = self.back.as_deref().unwrap().borrow();
        let left_chunk = self.left.as_deref().unwrap().borrow();
        let right_chunk = self.right.as_deref().unwrap().borrow();
        let up_chunk = self.up.as_deref().unwrap().borrow();
        let down_chunk = self.down.as_deref().unwrap().borrow();

        let mut vertices: Vec<glwrappers::Vertex> = Vec::new();
        let mut transparent_vertices: Vec<glwrappers::Vertex> = Vec::new();
        let mut foliage_vertices: Vec<glwrappers::Vertex> = Vec::new();
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
                        BlockRenderType::Cube(transparent, north, south, right, left, up, down) => {
                            for face in Face::all() {
                                let face_offset = face.get_offset();
                                let neighbor_pos = position + face_offset;
                                let original_offset_in_chunk = position.chunk_offset();
                                let offset_in_chunk = neighbor_pos.chunk_offset();
                                let (neighbor_block, light) = match (BlockPosition {
                                    x: original_offset_in_chunk.0 as i32 + face_offset.x,
                                    y: original_offset_in_chunk.1 as i32 + face_offset.y,
                                    z: original_offset_in_chunk.2 as i32 + face_offset.z,
                                }
                                .offset_from_origin_chunk())
                                {
                                    Some(Face::Front) => (
                                        front_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        front_chunk.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                    Some(Face::Back) => (
                                        back_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        back_chunk.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                    Some(Face::Left) => (
                                        left_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        left_chunk.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                    Some(Face::Right) => (
                                        right_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        right_chunk.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                    Some(Face::Up) => (
                                        up_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        up_chunk.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                    Some(Face::Down) => (
                                        down_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        down_chunk.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                    None => (
                                        self.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                        self.light[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize],
                                    ),
                                };
                                let neighbor_block = self.block_registry.get_block(neighbor_block);
                                let neighbor_side_full = neighbor_block
                                    .is_face_full(&face.opposite())
                                    && !(neighbor_block.is_transparent()
                                        && !block.is_transparent());
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
                                    let vertices = if *transparent {
                                        &mut transparent_vertices
                                    } else {
                                        &mut vertices
                                    };
                                    let uv0 = face_vertices[0].1.map(uv);
                                    let uv1 = face_vertices[1].1.map(uv);
                                    let uv2 = face_vertices[2].1.map(uv);
                                    let uv3 = face_vertices[3].1.map(uv);

                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[0].0.x + x,
                                        y: face_vertices[0].0.y + y,
                                        z: face_vertices[0].0.z + z,
                                        u: uv0.0,
                                        v: uv0.1,
                                        render_data: block.render_data,
                                        light,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[1].0.x + x,
                                        y: face_vertices[1].0.y + y,
                                        z: face_vertices[1].0.z + z,
                                        u: uv1.0,
                                        v: uv1.1,
                                        render_data: block.render_data,
                                        light,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[2].0.x + x,
                                        y: face_vertices[2].0.y + y,
                                        z: face_vertices[2].0.z + z,
                                        u: uv2.0,
                                        v: uv2.1,
                                        render_data: block.render_data,
                                        light,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[2].0.x + x,
                                        y: face_vertices[2].0.y + y,
                                        z: face_vertices[2].0.z + z,
                                        u: uv2.0,
                                        v: uv2.1,
                                        render_data: block.render_data,
                                        light,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[3].0.x + x,
                                        y: face_vertices[3].0.y + y,
                                        z: face_vertices[3].0.z + z,
                                        u: uv3.0,
                                        v: uv3.1,
                                        render_data: block.render_data,
                                        light,
                                    });
                                    vertices.push(glwrappers::Vertex {
                                        x: face_vertices[0].0.x + x,
                                        y: face_vertices[0].0.y + y,
                                        z: face_vertices[0].0.z + z,
                                        u: uv0.0,
                                        v: uv0.1,
                                        render_data: block.render_data,
                                        light,
                                    });
                                }
                            }
                        }
                        BlockRenderType::StaticModel(
                            transparent,
                            model,
                            _,
                            _,
                            _,
                            _,
                            _,
                            _,
                            connections,
                            foliage,
                        ) => {
                            let original_offset_in_chunk = position.chunk_offset();
                            let light = self.light[original_offset_in_chunk.0 as usize]
                                [original_offset_in_chunk.1 as usize]
                                [original_offset_in_chunk.2 as usize];
                            let vertices = if *transparent {
                                &mut transparent_vertices
                            } else if *foliage {
                                &mut foliage_vertices
                            } else {
                                &mut vertices
                            };
                            for face in Face::all() {
                                let face_offset = face.get_offset();
                                let neighbor_pos = position + face_offset;
                                let offset_in_chunk = neighbor_pos.chunk_offset();
                                let neighbor_block = match (BlockPosition {
                                    x: original_offset_in_chunk.0 as i32 + face_offset.x,
                                    y: original_offset_in_chunk.1 as i32 + face_offset.y,
                                    z: original_offset_in_chunk.2 as i32 + face_offset.z,
                                }
                                .offset_from_origin_chunk())
                                {
                                    Some(Face::Front) => {
                                        front_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                    Some(Face::Back) => {
                                        back_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                    Some(Face::Left) => {
                                        left_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                    Some(Face::Right) => {
                                        right_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                    Some(Face::Up) => {
                                        up_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                    Some(Face::Down) => {
                                        down_chunk.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                    None => {
                                        self.blocks[offset_in_chunk.0 as usize]
                                            [offset_in_chunk.1 as usize]
                                            [offset_in_chunk.2 as usize]
                                    }
                                };
                                if let Some(connection) =
                                    connections.by_face(face).get(&neighbor_block)
                                {
                                    connection.add_vertices_simple(
                                        &mut |pos, u, v| {
                                            vertices.push(Vertex {
                                                x: pos.x + 0.5,
                                                y: pos.y,
                                                z: pos.z + 0.5,
                                                u,
                                                v,
                                                render_data: block.render_data,
                                                light,
                                            });
                                            self.vertex_count += 1;
                                        },
                                        None,
                                        Vec3 {
                                            x: bx as f32,
                                            y: by as f32,
                                            z: bz as f32,
                                        },
                                        None,
                                    );
                                }
                            }
                            model.add_vertices_simple(
                                &mut |pos, u, v| {
                                    vertices.push(Vertex {
                                        x: pos.x + 0.5,
                                        y: pos.y,
                                        z: pos.z + 0.5,
                                        u,
                                        v,
                                        render_data: block.render_data,
                                        light,
                                    });
                                    self.vertex_count += 1;
                                },
                                None,
                                Vec3 {
                                    x: bx as f32,
                                    y: by as f32,
                                    z: bz as f32,
                                },
                                None,
                            );
                        }
                        BlockRenderType::Foliage(texture1, texture2, texture3, texture4) => {
                            let original_offset_in_chunk = position.chunk_offset();
                            let light = self.light[original_offset_in_chunk.0 as usize]
                                [original_offset_in_chunk.1 as usize]
                                [original_offset_in_chunk.2 as usize];
                            let mut face_creator =
                                |p1: Position,
                                 p2: Position,
                                 p3: Position,
                                 p4: Position,
                                 uv: (f32, f32, f32, f32),
                                 render_data: u8,
                                 light: u16| {
                                    let v1 = Vertex {
                                        x: p1.x,
                                        y: p1.y,
                                        z: p1.z,
                                        u: uv.0,
                                        v: uv.1,
                                        render_data,
                                        light,
                                    };
                                    let v2 = Vertex {
                                        x: p2.x,
                                        y: p2.y,
                                        z: p2.z,
                                        u: uv.2,
                                        v: uv.1,
                                        render_data,
                                        light,
                                    };
                                    let v3 = Vertex {
                                        x: p3.x,
                                        y: p3.y,
                                        z: p3.z,
                                        u: uv.2,
                                        v: uv.3,
                                        render_data,
                                        light,
                                    };
                                    let v4 = Vertex {
                                        x: p4.x,
                                        y: p4.y,
                                        z: p4.z,
                                        u: uv.0,
                                        v: uv.3,
                                        render_data,
                                        light,
                                    };
                                    foliage_vertices.push(v1);
                                    foliage_vertices.push(v2);
                                    foliage_vertices.push(v3);
                                    foliage_vertices.push(v3);
                                    foliage_vertices.push(v4);
                                    foliage_vertices.push(v1);
                                };
                            if let Some(texture1) = texture1 {
                                face_creator.call_mut((
                                    Position {
                                        x: x + 0.01,
                                        y: y + 0.99,
                                        z,
                                    },
                                    Position {
                                        x: x + 0.99,
                                        y: y + 0.99,
                                        z: z + 0.99,
                                    },
                                    Position {
                                        x: x + 0.99,
                                        y,
                                        z: z + 0.99,
                                    },
                                    Position { x: x + 0.01, y, z },
                                    texture1.get_coords(),
                                    block.render_data,
                                    light,
                                ));
                            }
                            if let Some(texture2) = texture2 {
                                face_creator.call_mut((
                                    Position {
                                        x: x + 0.99,
                                        y: y + 0.99,
                                        z,
                                    },
                                    Position {
                                        x: x + 0.01,
                                        y: y + 0.99,
                                        z: z + 0.99,
                                    },
                                    Position {
                                        x: x + 0.01,
                                        y,
                                        z: z + 1.,
                                    },
                                    Position { x: x + 0.99, y, z },
                                    texture2.get_coords(),
                                    block.render_data,
                                    light,
                                ));
                            }
                            if let Some(texture3) = texture3 {
                                face_creator.call_mut((
                                    Position {
                                        x: x + 0.5,
                                        y: y + 0.99,
                                        z,
                                    },
                                    Position {
                                        x: x + 0.5,
                                        y: y + 0.99,
                                        z: z + 0.99,
                                    },
                                    Position {
                                        x: x + 0.5,
                                        y,
                                        z: z + 0.99,
                                    },
                                    Position { x: x + 0.5, y, z },
                                    texture3.get_coords(),
                                    block.render_data,
                                    light,
                                ));
                            }
                            if let Some(texture4) = texture4 {
                                face_creator.call_mut((
                                    Position {
                                        x: x + 0.1,
                                        y: y + 0.99,
                                        z: z + 0.5,
                                    },
                                    Position {
                                        x: x + 0.99,
                                        y: y + 0.99,
                                        z: z + 0.5,
                                    },
                                    Position {
                                        x: x + 0.99,
                                        y,
                                        z: z + 0.5,
                                    },
                                    Position {
                                        x: x + 0.01,
                                        y,
                                        z: z + 0.5,
                                    },
                                    texture4.get_coords(),
                                    block.render_data,
                                    light,
                                ));
                            }
                        }
                    }
                }
            }
        }
        self.vertex_count = vertices.len() as u32;
        self.vbo
            .upload_data(bytemuck::cast_slice(&vertices), ogl33::GL_STATIC_DRAW);
        self.transparent_vertex_count = transparent_vertices.len() as u32;
        self.transparent_vbo.upload_data(
            bytemuck::cast_slice(&transparent_vertices),
            ogl33::GL_STATIC_DRAW,
        );
        self.foliage_vertex_count = foliage_vertices.len() as u32;
        self.foliage_vbo.upload_data(
            bytemuck::cast_slice(&foliage_vertices),
            ogl33::GL_STATIC_DRAW,
        );

        if self.vertex_count > 0 || self.foliage_vertex_count > 0 {
            world.solid_chunks.insert(self.position, this.clone());
        } else {
            world.solid_chunks.remove(&self.position);
        }
        if self.transparent_vertex_count > 0 {
            world.transparent_chunks.insert(self.position, this.clone());
        } else {
            world.transparent_chunks.remove(&self.position);
        }
    }
    pub fn render(
        &mut self,
        shader: &glwrappers::Shader,
        render_foliage: bool,
        rendered_chunks_stat: &mut (i32, i32, i32, i32, i32),
    ) {
        /*if self.modified {
            self.modified = !self.rebuild_chunk_mesh();
        }*/
        if true {
            /* !self.modified*/
            if self.vertex_count != 0 || (self.foliage_vertex_count != 0 && render_foliage) {
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
            }
            if self.vertex_count != 0 {
                rendered_chunks_stat.0 += 1;
                self.vao.bind();
                unsafe {
                    ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, self.vertex_count as i32);
                }
            }
            if self.foliage_vertex_count != 0 && render_foliage {
                rendered_chunks_stat.2 += 1;
                self.foliage_vao.bind();
                unsafe {
                    ogl33::glDisable(GL_CULL_FACE);

                    ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, self.foliage_vertex_count as i32);
                    ogl33::glEnable(GL_CULL_FACE);
                }
            }
        }
    }
    pub fn render_transparent(
        &self,
        shader: &glwrappers::Shader,
        rendered_chunks_stat: &mut (i32, i32, i32, i32, i32),
    ) {
        if self.transparent_vertex_count != 0 {
            rendered_chunks_stat.1 += 1;
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
            self.transparent_vao.bind();
            unsafe {
                ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, self.transparent_vertex_count as i32);
            }
        }
    }
}
#[derive(Clone)]
pub struct StaticBlockModelConnections {
    pub front: HashMap<u32, model::Model>,
    pub back: HashMap<u32, model::Model>,
    pub left: HashMap<u32, model::Model>,
    pub right: HashMap<u32, model::Model>,
    pub up: HashMap<u32, model::Model>,
    pub down: HashMap<u32, model::Model>,
}
impl StaticBlockModelConnections {
    pub fn by_face_mut(&mut self, face: &Face) -> &mut HashMap<u32, model::Model> {
        match face {
            Face::Front => &mut self.front,
            Face::Back => &mut self.back,
            Face::Left => &mut self.left,
            Face::Right => &mut self.right,
            Face::Up => &mut self.up,
            Face::Down => &mut self.down,
        }
    }
    pub fn by_face(&self, face: &Face) -> &HashMap<u32, model::Model> {
        match face {
            Face::Front => &self.front,
            Face::Back => &self.back,
            Face::Left => &self.left,
            Face::Right => &self.right,
            Face::Up => &self.up,
            Face::Down => &self.down,
        }
    }
}
#[derive(Clone)]
pub enum BlockRenderType {
    Air,
    Cube(
        bool,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
        AtlassedTexture,
    ),
    StaticModel(
        bool,
        model::Model,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        StaticBlockModelConnections,
        bool,
    ),
    Foliage(
        Option<AtlassedTexture>,
        Option<AtlassedTexture>,
        Option<AtlassedTexture>,
        Option<AtlassedTexture>,
    ),
}
impl BlockRenderType {
    pub fn add_item_quads(
        &self,
        size: f32,
        f: &mut dyn FnMut(f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32),
    ) {
        match self {
            Self::Air
            | Self::StaticModel(_, _, _, _, _, _, _, _, _, _)
            | Self::Foliage(_, _, _, _) => {}
            Self::Cube(_, north, _, right, _, up, _) => {
                let top_texture = up.get_coords();
                let front_texture = north.get_coords();
                let right_texture = right.get_coords();
                let middle_x = size * 13. / 26.;
                let middle_y = size * 11. / 26.;
                f.call_mut((
                    0.,
                    (size / 6. * 5.),
                    middle_x,
                    size,
                    size,
                    (size / 6. * 5.),
                    middle_x,
                    middle_y,
                    top_texture.0,
                    top_texture.1,
                    top_texture.2,
                    top_texture.3,
                ));
                /*quads.push(GUIQuad {
                    x1: x,
                    y1: y + (size / 6. * 5.),
                    x2: x + middle_x,
                    y2: y + size,
                    x3: x + size,
                    y3: y + (size / 6. * 5.),
                    x4: x + middle_x,
                    y4: y + middle_y,
                    color: Color {
                        r: 1.,
                        g: 1.,
                        b: 1.,
                        a: 1.,
                    },
                    u1: top_texture.0,
                    v1: top_texture.1,
                    u2: top_texture.2,
                    v2: top_texture.3,
                });*/
                /*quads.push(GUIQuad {
                    x1: x + middle_x,
                    y1: y + middle_y,
                    x2: x + size,
                    y2: y + (size * 5. / 6.),
                    x3: x + (size * 23. / 25.),
                    y3: y + (size * 7.5 / 25.),
                    x4: x + middle_x,
                    y4: y,
                    color: Color {
                        r: 1.,
                        g: 1.,
                        b: 1.,
                        a: 1.,
                    },
                    u1: front_texture.0,
                    v1: front_texture.3,
                    u2: front_texture.2,
                    v2: front_texture.1,
                });
                quads.push(GUIQuad {
                    x1: x,
                    y1: y + (size * 5. / 6.),
                    x2: x + middle_x,
                    y2: y + middle_y,
                    x3: x + middle_x,
                    y3: y,
                    x4: x + (size * 2. / 25.),
                    y4: y + (size * 7.5 / 25.),
                    color: Color {
                        r: 1.,
                        g: 1.,
                        b: 1.,
                        a: 1.,
                    },
                    u1: right_texture.0,
                    v1: right_texture.3,
                    u2: right_texture.2,
                    v2: right_texture.1,
                });*/
            }
        }
    }
}
#[derive(Clone)]
pub struct Block {
    pub render_type: BlockRenderType,
    pub dynamic: Option<Model>,
    pub render_data: u8,
    pub fluid: bool,
    pub no_collision: bool,
    pub selectable: bool,
    pub light: (u8, u8, u8),
}
impl Block {
    pub fn new_air() -> Self {
        Block {
            render_data: 0,
            render_type: BlockRenderType::Air,
            dynamic: None,
            fluid: false,
            no_collision: true,
            light: (0, 0, 0),
            selectable: false,
        }
    }
    pub fn is_light_emmiting(&self) -> bool {
        self.light.0 > 0 || self.light.1 > 0 || self.light.2 > 0
    }
    pub fn is_light_blocking(&self) -> bool {
        match self.render_type {
            BlockRenderType::Air => false,
            BlockRenderType::Cube(_, _, _, _, _, _, _) => !self.is_transparent(),
            BlockRenderType::StaticModel(_, _, _, _, _, _, _, _, _, _) => false,
            BlockRenderType::Foliage(_, _, _, _) => false,
        }
    }
    pub fn is_face_full(&self, face: &Face) -> bool {
        match self.render_type {
            BlockRenderType::Air => false,
            BlockRenderType::Cube(_, _, _, _, _, _, _) => true,
            BlockRenderType::StaticModel(_, _, north, south, right, left, up, down, _, _) => {
                match face {
                    Face::Front => north,
                    Face::Back => south,
                    Face::Left => left,
                    Face::Right => right,
                    Face::Up => up,
                    Face::Down => down,
                }
            }
            BlockRenderType::Foliage(_, _, _, _) => false,
        }
    }
    pub fn is_transparent(&self) -> bool {
        match self.render_type {
            BlockRenderType::Air => false,
            BlockRenderType::Cube(transparent, _, _, _, _, _, _) => transparent,
            BlockRenderType::StaticModel(transparent, _, _, _, _, _, _, _, _, _) => transparent,
            BlockRenderType::Foliage(_, _, _, _) => false,
        }
    }
}
#[derive(Clone)]
pub struct BlockRegistry {
    pub blocks: Vec<Block>,
}
impl BlockRegistry {
    #[inline(always)]
    pub fn get_block(&self, id: u32) -> &Block {
        &self.blocks[id as usize]
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
    pub fn empty() -> AtlassedTexture {
        AtlassedTexture {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            atlas_w: 1,
            atlas_h: 1,
        }
    }
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
    pub chunks: IndexMap<ChunkPosition, Rc<RefCell<Chunk<'a>>>>,
    block_registry: &'a BlockRegistry,
    pub light_updates: BTreeSet<BlockPosition>,
    pub chunk_mesh_updates: FxHashSet<ChunkPosition>,
    pub solid_chunks: FxHashMap<ChunkPosition, Rc<RefCell<Chunk<'a>>>>,
    pub transparent_chunks: FxHashMap<ChunkPosition, Rc<RefCell<Chunk<'a>>>>,
}
impl<'a> World<'a> {
    pub fn new(block_registry: &'a BlockRegistry) -> Self {
        World {
            chunks: IndexMap::default(),
            block_registry,
            light_updates: BTreeSet::new(),
            chunk_mesh_updates: FxHashSet::default(),
            solid_chunks: FxHashMap::default(),
            transparent_chunks: FxHashMap::default(),
        }
    }
    pub fn load_chunk(
        &mut self,
        position: ChunkPosition,
        blocks: [[[u32; 16]; 16]; 16],
    ) -> RefMut<'_, Chunk<'a>> {
        if !self.chunks.contains_key(&position) {
            let chunk = Rc::new(RefCell::new(Chunk::new(
                position,
                self.block_registry,
                blocks,
                self,
            )));
            self.chunks.insert(position, chunk.clone());
            self.chunk_mesh_updates.insert(position);
            for face in Face::all() {
                let offset = face.get_offset();
                let neighbor = self
                    .chunks
                    .get(&position.add(offset.x, offset.y, offset.z))
                    .map(|chunk| chunk.clone());
                {
                    chunk
                        .borrow_mut()
                        .set_neighbor_by_face(&face, neighbor.clone());
                }
                {
                    if let Some(neighbor) = neighbor {
                        let mut neighbor = neighbor.borrow_mut();
                        neighbor.set_neighbor_by_face(&face.opposite(), Some(chunk.clone()));
                        neighbor.schedule_mesh_rebuild(self);
                    }
                }
            }
            self.light_updates.insert(BlockPosition {
                x: position.x * 16 + 8,
                y: position.y * 16 + 15,
                z: position.z * 16 + 8,
            });
        }
        self.chunks.get_mut(&position).unwrap().borrow_mut()
    }
    pub fn unload_chunk(&mut self, position: ChunkPosition) {
        for face in Face::all() {
            let offset = face.get_offset();
            let neighbor = self.chunks.get(&position.add(offset.x, offset.y, offset.z));
            {
                if let Some(neighbor) = neighbor {
                    neighbor
                        .borrow_mut()
                        .set_neighbor_by_face(&face.opposite(), None);
                }
            }
        }
        self.chunks.remove(&position);
        self.solid_chunks.remove(&position);
        self.transparent_chunks.remove(&position);
    }
    pub fn get_chunk(&self, position: ChunkPosition) -> Option<Ref<'_, Chunk<'a>>> {
        match self.chunks.get(&position) {
            Some(chunk) => Some(chunk.borrow()),
            None => None,
        }
    }
    pub fn get_chunk_clone(&self, position: ChunkPosition) -> Option<Rc<RefCell<Chunk<'a>>>> {
        match self.chunks.get(&position) {
            Some(chunk) => Some(chunk.clone()),
            None => None,
        }
    }
    pub fn get_mut_chunk(&mut self, position: ChunkPosition) -> Option<RefMut<'_, Chunk<'a>>> {
        self.chunks
            .get_mut(&position)
            .map(|chunk| chunk.borrow_mut())
    }
    pub fn set_block(&mut self, position: BlockPosition, id: u32) -> Result<(), ()> {
        let chunk_position = position.to_chunk_pos();
        let offset = position.chunk_offset();
        if offset.0 == 0 {
            if let Some(chunk) = self.get_chunk_clone(chunk_position.add(-1, 0, 0)) {
                chunk.borrow().schedule_mesh_rebuild(self);
            }
        }
        if offset.0 == 15 {
            if let Some(chunk) = self.get_chunk_clone(chunk_position.add(1, 0, 0)) {
                chunk.borrow().schedule_mesh_rebuild(self);
            }
        }
        if offset.1 == 0 {
            if let Some(chunk) = self.get_chunk_clone(chunk_position.add(0, -1, 0)) {
                chunk.borrow().schedule_mesh_rebuild(self);
            }
        }
        if offset.1 == 15 {
            if let Some(chunk) = self.get_chunk_clone(chunk_position.add(0, 1, 0)) {
                chunk.borrow().schedule_mesh_rebuild(self);
            }
        }
        if offset.2 == 0 {
            if let Some(chunk) = self.get_chunk_clone(chunk_position.add(0, 0, -1)) {
                chunk.borrow().schedule_mesh_rebuild(self);
            }
        }
        if offset.2 == 15 {
            if let Some(chunk) = self.get_chunk_clone(chunk_position.add(0, 0, 1)) {
                chunk.borrow().schedule_mesh_rebuild(self);
            }
        }
        self.light_updates.insert(position);
        match self.get_chunk_clone(chunk_position) {
            Some(chunk) => {
                chunk
                    .borrow_mut()
                    .set_block(offset.0, offset.1, offset.2, id, self);
                Ok(())
            }
            None => Err(()),
        }
    }
    pub fn get_block(&self, position: BlockPosition) -> Option<u32> {
        self.get_chunk(position.to_chunk_pos())
            .map_or(None, |chunk| {
                let offset = position.chunk_offset();
                Some(chunk.get_block(offset.0, offset.1, offset.2))
            })
    }
    pub fn get_light(&self, position: BlockPosition) -> Option<(u8, u8, u8)> {
        self.get_chunk(position.to_chunk_pos())
            .map_or(None, |chunk| {
                let offset = position.chunk_offset();
                Some(chunk.get_light(offset.0, offset.1, offset.2))
            })
    }
    pub fn set_light(&mut self, position: BlockPosition, light: (u8, u8, u8)) {
        if let Some(mut chunk) = self.get_mut_chunk(position.to_chunk_pos()) {
            let offset = position.chunk_offset();
            let light = light.0 as u16 | ((light.1 as u16) << 4) | ((light.2 as u16) << 8);
            //| ((light.3 as u16) << 12);
            chunk.light[offset.0 as usize][offset.1 as usize][offset.2 as usize] = light;
            //chunk.modified = true;
            //todo
        }
    }
    pub fn loaded(&self, position: ChunkPosition) -> bool {
        self.get_chunk(position).is_some()
    }
    pub fn update_lights(&mut self) -> u32 {
        self.light_updates.clear();
        let updates = self.light_updates.len() as u32;
        while let Some(light_update) = self.light_updates.pop_last() {
            if self.get_block(light_update).map_or(false, |b| {
                !self.block_registry.get_block(b).is_light_blocking()
            }) {
                let current_light = self.get_light(light_update).unwrap();
                let mut biggest_light: (u8, u8, u8) = (
                    current_light.0 + 1,
                    current_light.1 + 1,
                    current_light.2 + 1,
                );
                for face in Face::all() {
                    let face_offset = face.get_offset();
                    let block_pos = BlockPosition {
                        x: light_update.x + face_offset.x,
                        y: light_update.y + face_offset.y,
                        z: light_update.z + face_offset.z,
                    };
                    let block_light = self
                        .get_block(block_pos)
                        .map_or((0, 0, 0), |b| self.block_registry.get_block(b).light);
                    let light = self.get_light(block_pos).unwrap_or((0, 0, 0));

                    biggest_light.0 = biggest_light.0.max(light.0).max(block_light.0);
                    biggest_light.1 = biggest_light.1.max(light.1).max(block_light.1);
                    biggest_light.2 = biggest_light.2.max(light.2).max(block_light.2);
                }
                let new_light = (
                    (biggest_light.0 - 1).max(0),
                    (biggest_light.1 - 1).max(0),
                    (biggest_light.2 - 1).max(0),
                );
                if new_light != current_light {
                    self.set_light(light_update, new_light);
                    for face in Face::all() {
                        let face_offset = face.get_offset();
                        let block_pos = BlockPosition {
                            x: light_update.x + face_offset.x,
                            y: light_update.y + face_offset.y,
                            z: light_update.z + face_offset.z,
                        };
                        self.light_updates.insert(block_pos);
                    }
                }
            }
        }
        updates
    }
    pub fn render(
        &mut self,
        shader: &glwrappers::Shader,
        time: f32,
        player_position: ChunkPosition,
    ) -> (i32, i32, i32, i32, i32) {
        let mesh_updates: Vec<_> = self
            .chunk_mesh_updates
            .extract_if(|_| true)
            .take(200)
            .collect(); //todo: optimize
        for mesh_update in mesh_updates {
            if let Some(chunk) = self.get_chunk_clone(mesh_update) {
                let cloned_chunk = chunk.clone();
                chunk.borrow_mut().rebuild_chunk_mesh(cloned_chunk, self);
            }
        }
        unsafe {
            ogl33::glEnable(GL_CULL_FACE);
        }
        let light_updates = self.update_lights() as i32;
        let mut rendered_chunks_stat = (0, 0, 0, self.chunks.len() as i32, light_updates);
        shader.set_uniform_float(shader.get_uniform_location("time\0").unwrap(), time);
        for chunk in self.solid_chunks.values() {
            chunk
                .borrow_mut()
                .render(shader, true, &mut rendered_chunks_stat);
        }
        /*for chunk in self.chunks.values() {
            let mut borrowed_chunk = chunk.borrow_mut();
            let pos = borrowed_chunk.position.clone();
            borrowed_chunk.render(
                shader,
                player_position.distance_squared(&pos) < 64,
                &mut rendered_chunks_stat,
            );
        }*/
        /*rendered_chunks.sort_by(|a, b| {
            let a = a.borrow();
            let b = b.borrow();
            a.position
                .distance_squared(&player_position)
                .cmp(&b.position.distance_squared(&player_position))
        });*/
        rendered_chunks_stat
    }
    pub fn render_transparent(
        &self,
        shader: &glwrappers::Shader,
        rendered_chunk_stats: &mut (i32, i32, i32, i32, i32),
    ) {
        unsafe {
            ogl33::glBlendFunc(ogl33::GL_SRC_ALPHA, ogl33::GL_ONE_MINUS_SRC_ALPHA);
            ogl33::glEnable(ogl33::GL_BLEND);
            ogl33::glDisable(ogl33::GL_CULL_FACE);
        }
        for chunk in self.transparent_chunks.values() {
            chunk
                .borrow()
                .render_transparent(shader, rendered_chunk_stats);
        }
        unsafe {
            ogl33::glDisable(ogl33::GL_BLEND);
        }
    }
}
pub struct Entity {
    pub entity_type: u32,
    pub position: Position,
    pub rotation: f32,
    pub items: HashMap<u32, ItemSlot>,
    pub animation: Option<(u32, f32)>,
}
pub struct ParticleManager {
    renderer: ParticleRenderer,
    particles: Vec<Particle>,
}
impl ParticleManager {
    pub fn new() -> Self {
        let particles = Vec::new();
        ParticleManager {
            particles,
            renderer: ParticleRenderer::new(),
        }
    }
    pub fn render(&mut self, projection: &Mat4, view: &Mat4) {
        self.renderer.render(&self.particles, projection, view);
    }
    pub fn tick(&mut self, delta_time: f32, world: &World, block_registry: &BlockRegistry) {
        for particle in &mut self.particles {
            let new_pos = Position {
                x: particle.position.x + (particle.velocity.0 * delta_time),
                y: particle.position.y + (particle.velocity.1 * delta_time),
                z: particle.position.z + (particle.velocity.2 * delta_time),
            };
            let no_collide = world
                .get_block(new_pos.to_block_pos())
                .map_or(true, |b| block_registry.get_block(b).no_collision);
            if no_collide {
                particle.position = new_pos;
            } else {
                if particle.destroy_on_collision {
                    if particle.lifetime < 0. {
                        particle.lifetime *= -1.;
                        particle.destroy_on_collision = false;
                    } else {
                        particle.destroyed = true;
                    }
                }
                particle.velocity = (0., 0., 0.);
            }
            particle.velocity.1 -= particle.gravity * delta_time;
            if particle.lifetime > 0. {
                particle.lifetime -= delta_time;
                if particle.lifetime <= 0. {
                    particle.destroyed = true;
                }
            }
        }
        self.particles.extract_if(|p| p.destroyed).count();
    }
}
pub struct ParticleRenderer {
    shader: glwrappers::Shader,
    vao: glwrappers::VertexArray,
    vertex_vbo: glwrappers::Buffer,
    particle_vbo: glwrappers::Buffer,
}
impl ParticleRenderer {
    pub fn new() -> Self {
        let shader = glwrappers::Shader::new(
            include_str!("shaders/particle.vert").to_string(),
            include_str!("shaders/particle.frag").to_string(),
        );
        let vao = glwrappers::VertexArray::new().unwrap();
        vao.bind();
        let mut vertex_vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array).unwrap();
        let mut vertices: Vec<[f32; 2]> = Vec::new();
        vertices.push([0., 0.]);
        vertices.push([1., 0.]);
        vertices.push([0., 1.]);
        vertices.push([1., 1.]);
        vertex_vbo.upload_data(bytemuck::cast_slice(&vertices), ogl33::GL_STATIC_DRAW);
        let particle_vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array).unwrap();
        unsafe {
            vertex_vbo.bind();
            ogl33::glVertexAttribPointer(0, 2, ogl33::GL_FLOAT, ogl33::GL_FALSE, 8, 0 as *const _);
            ogl33::glEnableVertexAttribArray(0);
            particle_vbo.bind();
            ogl33::glVertexAttribPointer(1, 3, ogl33::GL_FLOAT, ogl33::GL_FALSE, 32, 0 as *const _);
            ogl33::glVertexAttribDivisor(1, 1);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glVertexAttribPointer(
                2,
                4,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                32,
                12 as *const _,
            );
            ogl33::glVertexAttribDivisor(2, 1);
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glVertexAttribPointer(
                3,
                1,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                32,
                28 as *const _,
            );
            ogl33::glVertexAttribDivisor(3, 1);
            ogl33::glEnableVertexAttribArray(3);
        }
        VertexArray::unbind();
        ParticleRenderer {
            shader,
            vao,
            vertex_vbo,
            particle_vbo,
        }
    }
    pub fn render(&mut self, particles: &Vec<Particle>, projection: &Mat4, view: &Mat4) {
        let mut data = Vec::new();
        for particle in particles {
            data.push([
                particle.position.x,
                particle.position.y,
                particle.position.z,
                particle.color.0,
                particle.color.1,
                particle.color.2,
                if particle.lifetime > 0.
                    && particle.blendout_lifetime > 0.
                    && particle.lifetime < particle.blendout_lifetime
                {
                    particle.lifetime / particle.blendout_lifetime
                } else {
                    1.
                },
                particle.size,
            ]);
        }
        self.shader.use_program();
        self.shader.set_uniform_matrix(
            self.shader.get_uniform_location("projection\0").unwrap(),
            projection.clone(),
        );
        self.shader.set_uniform_matrix(
            self.shader.get_uniform_location("view\0").unwrap(),
            view.clone(),
        );
        self.vao.bind();
        self.particle_vbo
            .upload_data(bytemuck::cast_slice(&data), ogl33::GL_STREAM_DRAW);
        unsafe {
            //ogl33::glDisable(ogl33::GL_DEPTH_TEST);
            //ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, 4);
            ogl33::glBlendFunc(ogl33::GL_SRC_ALPHA, ogl33::GL_ONE_MINUS_SRC_ALPHA);
            ogl33::glEnable(ogl33::GL_BLEND);
            ogl33::glDrawArraysInstanced(ogl33::GL_TRIANGLE_STRIP, 0, 4, particles.len() as i32);
            ogl33::glDisable(ogl33::GL_BLEND);
            //ogl33::glEnable(ogl33::GL_DEPTH_TEST);
        }
        VertexArray::unbind();
    }
}
pub struct Particle {
    position: Position,
    color: (f32, f32, f32),
    velocity: (f32, f32, f32),
    size: f32,
    gravity: f32,
    lifetime: f32,
    blendout_lifetime: f32,
    destroy_on_collision: bool,
    destroyed: bool,
}

pub struct SoundManager {
    alto: Alto,
    device: alto::OutputDevice,
    context: alto::Context,
    buffers: HashMap<String, Arc<alto::Buffer>>,
    sounds: Vec<alto::StaticSource>,
}
impl SoundManager {
    pub fn new() -> Self {
        let alto = Alto::load_default().unwrap();
        let device = alto.open(None).unwrap();
        let context = device.new_context(None).unwrap();
        context.set_distance_model(alto::DistanceModel::InverseClamped); //todo: check what is best
        SoundManager {
            alto,
            device,
            context,
            buffers: HashMap::new(),
            sounds: Vec::new(),
        }
    }
    pub fn load(&mut self, name: String, data: Vec<u8>) {
        let reader = hound::WavReader::new(data.as_slice()).unwrap();
        let frequency = reader.spec().sample_rate as i32;
        let samples: Vec<i16> = reader.into_samples().map(|s| s.unwrap()).collect();
        let buffer = self
            .context
            .new_buffer::<alto::Mono<i16>, &[i16]>(samples.as_slice(), frequency)
            .unwrap();
        self.buffers.insert(name, Arc::new(buffer));
    }
    pub fn play_sound(
        &mut self,
        name: String,
        position: Position,
        gain: f32,
        pitch: f32,
        relative: bool,
    ) {
        let mut source = self.context.new_static_source().unwrap();
        source
            .set_buffer(self.buffers.get(&name).unwrap().clone())
            .unwrap();
        source
            .set_position([position.x, position.y, position.z])
            .unwrap();
        source.set_gain(gain).unwrap();
        source.set_pitch(pitch).unwrap();
        source.set_looping(false);
        source.set_relative(relative);
        source.play();
        self.sounds.push(source);
    }
    pub fn tick(&mut self, position: Vec3, forward: Vec3) {
        self.context
            .set_position([position.x, position.y, position.z])
            .unwrap();
        self.context
            .set_orientation((
                [ClientPlayer::UP.x, ClientPlayer::UP.y, ClientPlayer::UP.z],
                [-forward.x, -forward.y, -forward.z],
            ))
            .unwrap();
        //todo: set velocity
        self.sounds
            .extract_if(|source| source.state() != alto::SourceState::Playing)
            .count();
    }
}
