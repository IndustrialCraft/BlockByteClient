use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ops::AddAssign,
    rc::Rc,
    sync::Mutex,
};

use crate::{
    glwrappers::{Vertex, VertexArray},
    util::{self, *},
    TextureAtlas,
};
use json::JsonValue;
use rustc_hash::FxHashMap;
use sdl2::keyboard::Keycode;
use ultraviolet::*;
use uuid::Uuid;

use crate::glwrappers::{self, ModelVertex};

#[derive(Clone, Copy)]
pub struct ClientPlayer<'a> {
    pub position: Vec3,
    velocity: Vec3,
    pub pitch_deg: f32,
    pub yaw_deg: f32,
    shifting: bool,
    shifting_animation: f32,
    block_registry: &'a BlockRegistry,
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

        if !(move_vector.x == 0.0 && move_vector.y == 0.0 && move_vector.z == 0.0) {
            move_vector = move_vector.normalized();
        }
        move_vector /= 2.;
        if self.shifting {
            move_vector /= 2.;
        }
        if keys.contains(&Keycode::Space) {
            let block = world.get_block(position.to_block_pos()).unwrap();
            let block = self.block_registry.get_block(block);
            if block.fluid {
                move_vector.y += 0.5;
                self.velocity.y = 0.;
            } else {
                if ClientPlayer::collides_at(
                    self.block_registry,
                    position.add(0., -0.2, 0.),
                    world,
                    self.shifting,
                ) {
                    self.velocity.y = 0.8;
                }
            }
        }
        let mut total_move = (move_vector + self.velocity) * (delta_time * 10f32);
        if ClientPlayer::collides_at(
            self.block_registry,
            position.add(total_move.x, 0., 0.),
            world,
            self.shifting,
        ) {
            total_move.x = 0.;
            self.velocity.x = 0.;
        }
        if ClientPlayer::collides_at(
            self.block_registry,
            position.add(total_move.x, total_move.y, 0.),
            world,
            self.shifting,
        ) {
            total_move.y = 0.;
            self.velocity.y = 0.;
        }
        if ClientPlayer::collides_at(
            self.block_registry,
            position.add(total_move.x, total_move.y, total_move.z),
            world,
            self.shifting,
        ) {
            total_move.z = 0.;
            self.velocity.z = 0.;
        }
        if (total_move.x != 0.
            && self.shifting
            && ClientPlayer::collides_at(
                self.block_registry,
                position.add(0., -0.1, 0.),
                world,
                self.shifting,
            ))
            && !ClientPlayer::collides_at(
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
                self.block_registry,
                position.add(total_move.x, -0.1, 0.),
                world,
                self.shifting,
            ))
            && !ClientPlayer::collides_at(
                self.block_registry,
                position.add(total_move.x, -0.1, total_move.z),
                world,
                self.shifting,
            )
        {
            total_move.z = 0.;
            self.velocity.z = 0.;
        }
        self.velocity.x *= 0.9;
        self.velocity.z *= 0.9;
        self.position += total_move;
        self.velocity.y -= delta_time * 2f32;
        self.shifting_animation += (if self.shifting { 1. } else { -1. }) * delta_time * 4.;
        self.shifting_animation = self.shifting_animation.clamp(0., 0.5);
    }
    fn collides_at(
        block_registry: &BlockRegistry,
        position: util::Position,
        world: &World,
        shifting: bool,
    ) -> bool {
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

pub struct Chunk<'a> {
    blocks: [[[u32; 16]; 16]; 16],
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
    modified: bool,
    front: Option<Rc<RefCell<Chunk<'a>>>>,
    back: Option<Rc<RefCell<Chunk<'a>>>>,
    left: Option<Rc<RefCell<Chunk<'a>>>>,
    right: Option<Rc<RefCell<Chunk<'a>>>>,
    up: Option<Rc<RefCell<Chunk<'a>>>>,
    down: Option<Rc<RefCell<Chunk<'a>>>>,
}
impl<'a> Chunk<'a> {
    fn set_neighbor_by_face(&mut self, face: &Face, chunk: Option<&Rc<RefCell<Chunk<'a>>>>) {
        match face {
            Face::Front => self.front = chunk.map(|e| e.clone()),
            Face::Back => self.back = chunk.map(|e| e.clone()),
            Face::Left => self.left = chunk.map(|e| e.clone()),
            Face::Right => self.right = chunk.map(|e| e.clone()),
            Face::Up => self.up = chunk.map(|e| e.clone()),
            Face::Down => self.down = chunk.map(|e| e.clone()),
        }
    }
    pub fn new(
        position: ChunkPosition,
        block_registry: &'a BlockRegistry,
        blocks: [[[u32; 16]; 16]; 16],
    ) -> Self {
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
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        Chunk {
            blocks,
            vao,
            vbo,
            vertex_count: 0,
            position,
            block_registry,
            modified: true,
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
        }
    }
    pub fn set_block(&mut self, x: u8, y: u8, z: u8, block_type: u32) {
        self.blocks[x as usize][y as usize][z as usize] = block_type;
        self.modified = true;
    }
    pub fn schedule_mesh_rebuild(&mut self) {
        self.modified = true;
    }
    pub fn get_block(&self, x: u8, y: u8, z: u8) -> u32 {
        return self.blocks[x as usize][y as usize][z as usize];
    }
    fn rebuild_chunk_mesh(&mut self) -> bool {
        if self.front.is_none()
            || self.back.is_none()
            || self.left.is_none()
            || self.right.is_none()
            || self.up.is_none()
            || self.down.is_none()
        {
            return false;
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
                                let original_offset_in_chunk = position.chunk_offset();
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
                                    connection.add_to_chunk_mesh(
                                        vertices,
                                        block.render_data,
                                        BlockPosition {
                                            x: bx,
                                            y: by,
                                            z: bz,
                                        },
                                    );
                                }
                            }

                            model.add_to_chunk_mesh(
                                vertices,
                                block.render_data,
                                BlockPosition {
                                    x: bx,
                                    y: by,
                                    z: bz,
                                },
                            );
                        }
                        BlockRenderType::Foliage(texture1, texture2, texture3, texture4) => {
                            if let Some(texture1) = texture1 {
                                StaticBlockModel::create_face(
                                    &mut foliage_vertices,
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
                                );
                            }
                            if let Some(texture2) = texture2 {
                                StaticBlockModel::create_face(
                                    &mut foliage_vertices,
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
                                );
                            }
                            if let Some(texture3) = texture3 {
                                StaticBlockModel::create_face(
                                    &mut foliage_vertices,
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
                                );
                            }
                            if let Some(texture4) = texture4 {
                                StaticBlockModel::create_face(
                                    &mut foliage_vertices,
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
                                );
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
        return true;
    }
    pub fn render(
        &mut self,
        shader: &glwrappers::Shader,
        render_foliage: bool,
        rendered_chunks_stat: &mut (i32, i32, i32, i32),
    ) {
        if self.modified {
            self.modified = !self.rebuild_chunk_mesh();
        }
        if !self.modified {
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
                    ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, self.foliage_vertex_count as i32);
                }
            }
        }
    }
    pub fn render_transparent(
        &self,
        shader: &glwrappers::Shader,
        rendered_chunks_stat: &mut (i32, i32, i32, i32),
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
    pub front: HashMap<u32, StaticBlockModel>,
    pub back: HashMap<u32, StaticBlockModel>,
    pub left: HashMap<u32, StaticBlockModel>,
    pub right: HashMap<u32, StaticBlockModel>,
    pub up: HashMap<u32, StaticBlockModel>,
    pub down: HashMap<u32, StaticBlockModel>,
}
impl StaticBlockModelConnections {
    pub fn by_face_mut(&mut self, face: &Face) -> &mut HashMap<u32, StaticBlockModel> {
        match face {
            Face::Front => &mut self.front,
            Face::Back => &mut self.back,
            Face::Left => &mut self.left,
            Face::Right => &mut self.right,
            Face::Up => &mut self.up,
            Face::Down => &mut self.down,
        }
    }
    pub fn by_face(&self, face: &Face) -> &HashMap<u32, StaticBlockModel> {
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
        StaticBlockModel,
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
#[derive(Clone)]
pub struct Block {
    pub render_type: BlockRenderType,
    pub render_data: u8,
    pub fluid: bool,
    pub no_collision: bool,
}
impl Block {
    pub fn new_air() -> Self {
        Block {
            render_data: 0,
            render_type: BlockRenderType::Air,
            fluid: false,
            no_collision: true,
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
    chunks: FxHashMap<ChunkPosition, Rc<RefCell<Chunk<'a>>>>,
    pub blocks_with_items: HashMap<BlockPosition, HashMap<u32, (f32, f32, f32, u32)>>,
    block_registry: &'a BlockRegistry,
}
impl<'a> World<'a> {
    pub fn new(block_registry: &'a BlockRegistry) -> Self {
        World {
            chunks: FxHashMap::default(),
            block_registry,
            blocks_with_items: HashMap::new(),
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
            )));
            self.chunks.insert(position, chunk.clone());
            for face in Face::all() {
                let offset = face.get_offset();
                let neighbor = self.chunks.get(&position.add(offset.x, offset.y, offset.z));
                {
                    chunk.borrow_mut().set_neighbor_by_face(&face, neighbor);
                }
                {
                    if let Some(neighbor) = neighbor {
                        neighbor
                            .borrow_mut()
                            .set_neighbor_by_face(&face.opposite(), Some(&chunk));
                    }
                }
            }
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
        self.blocks_with_items
            .drain_filter(|pos, _| pos.to_chunk_pos() == position);
    }
    pub fn get_chunk(&self, position: ChunkPosition) -> Option<Ref<'_, Chunk<'a>>> {
        match self.chunks.get(&position) {
            Some(chunk) => Some(chunk.borrow()),
            None => None,
        }
    }
    pub fn get_mut_chunk(&mut self, position: ChunkPosition) -> Option<RefMut<'_, Chunk<'a>>> {
        self.chunks
            .get_mut(&position)
            .map(|chunk| chunk.borrow_mut())
    }
    pub fn set_block(&mut self, position: BlockPosition, id: u32) -> Result<(), ()> {
        self.blocks_with_items.remove(&position);
        let chunk_position = position.to_chunk_pos();
        let offset = position.chunk_offset();
        if offset.0 == 0 {
            if let Some(mut chunk) = self.get_mut_chunk(chunk_position.add(-1, 0, 0)) {
                chunk.schedule_mesh_rebuild();
            }
        }
        if offset.0 == 15 {
            if let Some(mut chunk) = self.get_mut_chunk(chunk_position.add(1, 0, 0)) {
                chunk.schedule_mesh_rebuild();
            }
        }
        if offset.1 == 0 {
            if let Some(mut chunk) = self.get_mut_chunk(chunk_position.add(0, -1, 0)) {
                chunk.schedule_mesh_rebuild();
            }
        }
        if offset.1 == 15 {
            if let Some(mut chunk) = self.get_mut_chunk(chunk_position.add(0, 15, 0)) {
                chunk.schedule_mesh_rebuild();
            }
        }
        if offset.2 == 0 {
            if let Some(mut chunk) = self.get_mut_chunk(chunk_position.add(0, 0, -1)) {
                chunk.schedule_mesh_rebuild();
            }
        }
        if offset.2 == 15 {
            if let Some(mut chunk) = self.get_mut_chunk(chunk_position.add(0, 0, 1)) {
                chunk.schedule_mesh_rebuild();
            }
        }
        match self.get_mut_chunk(chunk_position) {
            Some(mut chunk) => {
                chunk.set_block(offset.0, offset.1, offset.2, id);
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
    pub fn render(
        &mut self,
        shader: &glwrappers::Shader,
        time: f32,
        player_position: ChunkPosition,
    ) -> (i32, i32, i32, i32) {
        let mut rendered_chunks_stat = (0, 0, 0, self.chunks.len() as i32);
        shader.set_uniform_float(shader.get_uniform_location("time\0").unwrap(), time);
        let mut rendered_chunks = Vec::new();
        for chunk in self.chunks.values() {
            let pos = { chunk.borrow().position.clone() };
            chunk.borrow_mut().render(
                shader,
                player_position.distance_squared(&pos) < 64,
                &mut rendered_chunks_stat,
            );
            rendered_chunks.push(chunk);
        }
        /*rendered_chunks.sort_by(|a, b| {
            let a = a.borrow();
            let b = b.borrow();
            a.position
                .distance_squared(&player_position)
                .cmp(&b.position.distance_squared(&player_position))
        });*/
        unsafe {
            ogl33::glBlendFunc(ogl33::GL_SRC_ALPHA, ogl33::GL_ONE_MINUS_SRC_ALPHA);
            ogl33::glEnable(ogl33::GL_BLEND);
        }
        for chunk in rendered_chunks {
            chunk
                .borrow()
                .render_transparent(shader, &mut rendered_chunks_stat);
        }
        unsafe {
            ogl33::glDisable(ogl33::GL_BLEND);
        }
        rendered_chunks_stat
    }
}
pub struct Entity {
    pub entity_type: u32,
    pub position: Position,
    pub rotation: f32,
    pub items: HashMap<u32, u32>,
}
#[derive(Clone, Debug)]
pub struct BlockModelCube {
    pub from: Position,
    pub to: Position,
    pub north_uv: (f32, f32, f32, f32),
    pub south_uv: (f32, f32, f32, f32),
    pub right_uv: (f32, f32, f32, f32),
    pub left_uv: (f32, f32, f32, f32),
    pub up_uv: (f32, f32, f32, f32),
    pub down_uv: (f32, f32, f32, f32),
    pub origin: (f32, f32, f32),
    pub rotation: (f32, f32, f32),
}
#[derive(Clone)]
pub struct StaticBlockModel {
    pub cubes: Vec<BlockModelCube>,
}
impl StaticBlockModel {
    pub fn new(json: &Vec<JsonValue>, texture: &AtlassedTexture) -> Self {
        let mut cubes = Vec::new();
        for json in json {
            for element in json["elements"].members() {
                assert_eq!(element["type"], "cube");
                let from = EntityModel::parse_array_into_position(&element["from"]);
                let to = EntityModel::parse_array_into_position(&element["to"]);
                let faces = &element["faces"];
                let rotation = if element["rotation"].is_null() {
                    (0., 0., 0.)
                } else {
                    StaticBlockModel::parse_array_into_three_tuple(&element["rotation"])
                };
                cubes.push(BlockModelCube {
                    from,
                    to,
                    north_uv: EntityModel::parse_uv(&faces["north"], texture),
                    south_uv: EntityModel::parse_uv(&faces["south"], texture),
                    right_uv: EntityModel::parse_uv(&faces["east"], texture),
                    left_uv: EntityModel::parse_uv(&faces["west"], texture),
                    up_uv: EntityModel::parse_uv(&faces["up"], texture),
                    down_uv: EntityModel::parse_uv(&faces["down"], texture),
                    origin: StaticBlockModel::parse_array_into_three_tuple(&element["origin"]),
                    rotation,
                });
            }
        }
        StaticBlockModel { cubes }
    }
    pub fn parse_array_into_three_tuple(json: &JsonValue) -> (f32, f32, f32) {
        (
            json[0].as_f32().unwrap(),
            json[1].as_f32().unwrap(),
            json[2].as_f32().unwrap(),
        )
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
                cube.rotation,
                cube.origin,
            );
        }
    }
    fn rotate_point(
        point: (f32, f32, f32),
        matrix: &Mat4,
        origin: (f32, f32, f32),
        position: BlockPosition,
    ) -> Position {
        let vec = matrix.transform_vec3(Vec3::new(
            point.0 - 0.5 - origin.0,
            point.1 - origin.1,
            point.2 - 0.5 - origin.2,
        ));
        Position {
            x: vec.x + 0.5 + origin.0 + (position.x as f32),
            y: vec.y + origin.1 + (position.y as f32),
            z: vec.z + 0.5 + origin.2 + (position.z as f32),
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
        rotation: (f32, f32, f32),
        origin: (f32, f32, f32),
    ) {
        let origin = (origin.0 / 16., origin.1 / 16., origin.2 / 16.);
        let matrix = Mat4::from_euler_angles(
            rotation.0.to_radians(),
            rotation.1.to_radians(),
            rotation.2.to_radians(),
        );
        let p000 =
            StaticBlockModel::rotate_point((from.x, from.y, from.z), &matrix, origin, position);
        let p001 =
            StaticBlockModel::rotate_point((from.x, from.y, to.z), &matrix, origin, position);
        let p010 =
            StaticBlockModel::rotate_point((from.x, to.y, from.z), &matrix, origin, position);
        let p011 = StaticBlockModel::rotate_point((from.x, to.y, to.z), &matrix, origin, position);
        let p100 =
            StaticBlockModel::rotate_point((to.x, from.y, from.z), &matrix, origin, position);
        let p101 = StaticBlockModel::rotate_point((to.x, from.y, to.z), &matrix, origin, position);
        let p110 = StaticBlockModel::rotate_point((to.x, to.y, from.z), &matrix, origin, position);
        let p111 = StaticBlockModel::rotate_point((to.x, to.y, to.z), &matrix, origin, position);
        StaticBlockModel::create_face(vertices, p000, p100, p101, p001, down, render_data);
        StaticBlockModel::create_face(vertices, p010, p110, p111, p011, up, render_data);
        StaticBlockModel::create_face(vertices, p000, p001, p011, p010, west, render_data);
        StaticBlockModel::create_face(vertices, p100, p101, p111, p110, east, render_data);
        StaticBlockModel::create_face(vertices, p000, p100, p110, p010, north, render_data);
        StaticBlockModel::create_face(vertices, p001, p101, p111, p011, south, render_data);
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
                x: position.x + (self.render_data.hitbox_w / 2.),
                y: position.y,
                z: position.z + (self.render_data.hitbox_d / 2.),
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
pub struct ParticleManager {
    renderer: ParticleRenderer,
    particles: Vec<Particle>,
}
impl ParticleManager {
    pub fn new() -> Self {
        let mut particles = Vec::new();
        for i in 0..10 {
            particles.push(Particle {
                position: Position {
                    x: (i as f32) * 1.5,
                    y: 50.,
                    z: 0.,
                },
                color: (0., 1., 0.),
                velocity: (0., -0.3, 0.),
                gravity: 1.,
                lifetime: -10.,
                blendout_lifetime: 7.,
                destroy_on_collision: true,
                destroyed: false,
            });
        }
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
        self.particles.drain_filter(|p| p.destroyed);
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
            ogl33::glVertexAttribPointer(1, 3, ogl33::GL_FLOAT, ogl33::GL_FALSE, 28, 0 as *const _);
            ogl33::glVertexAttribDivisor(1, 1);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glVertexAttribPointer(
                2,
                4,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                28,
                12 as *const _,
            );
            ogl33::glVertexAttribDivisor(2, 1);
            ogl33::glEnableVertexAttribArray(2);
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
    gravity: f32,
    lifetime: f32,
    blendout_lifetime: f32,
    destroy_on_collision: bool,
    destroyed: bool,
}
