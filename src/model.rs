use endio::LERead;
use endio::LEWrite;
use std::collections::BTreeMap;
use std::path::Path;
use ultraviolet::Mat4;
use ultraviolet::Vec3;
use ultraviolet::Vec4;

use crate::game::AtlassedTexture;
use crate::glwrappers::Vertex;
use crate::util;
use crate::util::Position;
#[derive(Clone)]
pub struct Model {
    root_bone: Bone,
    animations: Vec<Animation>,
    texture: AtlassedTexture,
}
impl Model {
    pub fn new_from_file(file: &Path, texture: AtlassedTexture) -> Self {
        Model::new(std::fs::read(file).unwrap(), texture)
    }
    pub fn new(data: Vec<u8>, texture: AtlassedTexture) -> Self {
        let mut data = data.as_slice();
        Model {
            root_bone: Bone::from_stream(&mut data),
            animations: {
                let animations_cnt: u32 = data.read_be().unwrap();
                let mut animations = Vec::with_capacity(animations_cnt as usize);
                for _ in 0..animations_cnt {
                    animations.push(Animation {
                        name: util::read_string(&mut data),
                        length: data.read_be().unwrap(),
                    })
                }
                animations
            },
            texture,
        }
    }
    pub fn add_vertices<F>(
        &self,
        vertex_consumer: &mut F,
        animation: Option<(String, f32)>,
        position: Vec3,
        rotation: Vec3,
        rotation_origin: Vec3,
        scale: Vec3,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let mut animation_id = None;
        let mut animation_length = 1f32;
        if let Some((animation, time)) = animation {
            for i in 0..self.animations.len() {
                let search_animation = self.animations.get(i).unwrap();
                if search_animation.name == animation {
                    animation_id = Some((i as u32, time));
                    animation_length = search_animation.length;
                }
            }
        }
        self.root_bone.add_vertices(
            vertex_consumer,
            animation_id.map(|id| (id.0, id.1 % animation_length)),
            Mat4::from_nonuniform_scale(scale)
                * Bone::create_rotation_matrix_with_origin(
                    &rotation,
                    &(rotation_origin + position),
                )
                * Mat4::from_translation(position),
            &self.texture,
        );
    }
    pub fn add_vertices_simple<F>(
        &self,
        vertex_consumer: &mut F,
        animation: Option<(String, f32)>,
        position: Vec3,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let mut animation_id = None;
        let mut animation_length = 1f32;
        if let Some((animation, time)) = animation {
            for i in 0..self.animations.len() {
                let search_animation = self.animations.get(i).unwrap();
                if search_animation.name == animation {
                    animation_id = Some((i as u32, time));
                    animation_length = search_animation.length;
                }
            }
        }
        self.root_bone.add_vertices(
            vertex_consumer,
            animation_id.map(|id| (id.0, id.1 % animation_length)),
            Mat4::from_translation(position),
            &self.texture,
        );
    }
}
#[derive(Clone)]
struct Bone {
    child_bones: Vec<Bone>,
    cube_elements: Vec<CubeElement>,
    animations: BTreeMap<u32, AnimationData>,
    origin: Vec3,
}
impl Bone {
    pub fn from_stream(data: &mut &[u8]) -> Self {
        Bone {
            origin: from_stream_to_vec3(data),
            child_bones: {
                let child_bones_cnt: u32 = data.read_be().unwrap();
                let mut child_bones = Vec::with_capacity(child_bones_cnt as usize);
                for _ in 0..child_bones_cnt {
                    child_bones.push(Bone::from_stream(data));
                }
                child_bones
            },
            cube_elements: {
                let cube_elements_cnt: u32 = data.read_be().unwrap();
                let mut cube_elements = Vec::with_capacity(cube_elements_cnt as usize);
                for _ in 0..cube_elements_cnt {
                    cube_elements.push(CubeElement::from_stream(data));
                }
                cube_elements
            },
            animations: {
                let mut animation = BTreeMap::new();
                let animations_cnt: u32 = data.read_be().unwrap();
                for _ in 0..animations_cnt {
                    animation.insert(data.read_be().unwrap(), AnimationData::from_stream(data));
                }
                animation
            },
        }
    }
    pub fn add_vertices<F>(
        &self,
        vertex_consumer: &mut F,
        animation: Option<(u32, f32)>,
        parent_matrix: Mat4,
        texture: &AtlassedTexture,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let animation_data = animation.and_then(|id| self.animations.get(&id.0));
        let animation_matrix = if let Some(animation_data) = animation_data {
            let animation_data = animation_data.get_for_time(animation.unwrap().1);
            Mat4::from_nonuniform_scale(animation_data.2)
                * Bone::create_rotation_matrix_with_origin(&animation_data.1, &self.origin)
                * Mat4::from_translation(Vec3 {
                    x: -animation_data.0.x,
                    y: animation_data.0.y,
                    z: -animation_data.0.z,
                })
        } else {
            Mat4::identity()
        };
        let bone_matrix = parent_matrix * animation_matrix;
        for child in &self.child_bones {
            child.add_vertices(vertex_consumer, animation, bone_matrix, texture);
        }
        for cube in &self.cube_elements {
            Bone::create_cube(
                vertex_consumer,
                cube.position,
                cube.scale,
                &cube.front,
                &cube.back,
                &cube.up,
                &cube.down,
                &cube.left,
                &cube.right,
                bone_matrix
                    * Bone::create_rotation_matrix_with_origin(&cube.rotation, &cube.origin),
                texture,
            );
        }
    }
    fn create_rotation_matrix_with_origin(rotation: &Vec3, origin: &Vec3) -> Mat4 {
        let translation = Mat4::from_translation(origin.clone());
        translation
            * Mat4::from_euler_angles(rotation.x, rotation.y, rotation.z)
            * translation.inversed()
    }
    fn create_cube<F>(
        vertex_consumer: &mut F,
        position: Vec3,
        size: Vec3,
        north: &CubeElementFace,
        south: &CubeElementFace,
        up: &CubeElementFace,
        down: &CubeElementFace,
        west: &CubeElementFace,
        east: &CubeElementFace,
        matrix: Mat4,
        texture: &AtlassedTexture,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let p000 = matrix
            * Vec4 {
                x: position.x,
                y: position.y,
                z: position.z,
                w: 1.,
            };
        let p001 = matrix
            * Vec4 {
                x: position.x,
                y: position.y,
                z: position.z + size.z,
                w: 1.,
            };
        let p010 = matrix
            * Vec4 {
                x: position.x,
                y: position.y + size.y,
                z: position.z,
                w: 1.,
            };
        let p011 = matrix
            * Vec4 {
                x: position.x,
                y: position.y + size.y,
                z: position.z + size.z,
                w: 1.,
            };
        let p100 = matrix
            * Vec4 {
                x: position.x + size.x,
                y: position.y,
                z: position.z,
                w: 1.,
            };
        let p101 = matrix
            * Vec4 {
                x: position.x + size.x,
                y: position.y,
                z: position.z + size.z,
                w: 1.,
            };
        let p110 = matrix
            * Vec4 {
                x: position.x + size.x,
                y: position.y + size.y,
                z: position.z,
                w: 1.,
            };
        let p111 = matrix
            * Vec4 {
                x: position.x + size.x,
                y: position.y + size.y,
                z: position.z + size.z,
                w: 1.,
            };
        Bone::create_face(vertex_consumer, p000, p100, p101, p001, down, texture);
        Bone::create_face(vertex_consumer, p010, p110, p111, p011, up, texture);
        Bone::create_face(vertex_consumer, p000, p001, p011, p010, west, texture);
        Bone::create_face(vertex_consumer, p100, p101, p111, p110, east, texture);
        Bone::create_face(vertex_consumer, p000, p100, p110, p010, south, texture);
        Bone::create_face(vertex_consumer, p001, p101, p111, p011, north, texture);
    }
    fn create_face<F>(
        vertex_consumer: &mut F,
        p1: Vec4,
        p2: Vec4,
        p3: Vec4,
        p4: Vec4,
        uv: &CubeElementFace,
        texture: &AtlassedTexture,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let uv4 = texture.map_uv((uv.u1, uv.v1));
        let uv3 = texture.map_uv((uv.u2, uv.v1));
        let uv2 = texture.map_uv((uv.u2, uv.v2));
        let uv1 = texture.map_uv((uv.u1, uv.v2));
        let v1 = (
            Vec3 {
                x: p1.x,
                y: p1.y,
                z: p1.z,
            },
            uv1.0,
            uv1.1,
        );
        let v2 = (
            Vec3 {
                x: p2.x,
                y: p2.y,
                z: p2.z,
            },
            uv2.0,
            uv2.1,
        );
        let v3 = (
            Vec3 {
                x: p3.x,
                y: p3.y,
                z: p3.z,
            },
            uv3.0,
            uv3.1,
        );
        let v4 = (
            Vec3 {
                x: p4.x,
                y: p4.y,
                z: p4.z,
            },
            uv4.0,
            uv4.1,
        );
        vertex_consumer.call_mut(v1);
        vertex_consumer.call_mut(v2);
        vertex_consumer.call_mut(v3);
        vertex_consumer.call_mut(v3);
        vertex_consumer.call_mut(v4);
        vertex_consumer.call_mut(v1);
    }
}
#[derive(Clone)]
struct CubeElement {
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
    origin: Vec3,
    front: CubeElementFace,
    back: CubeElementFace,
    left: CubeElementFace,
    right: CubeElementFace,
    up: CubeElementFace,
    down: CubeElementFace,
}
impl CubeElement {
    pub fn from_stream(data: &mut &[u8]) -> Self {
        Self {
            position: from_stream_to_vec3(data),
            scale: from_stream_to_vec3(data),
            rotation: from_stream_to_vec3(data),
            origin: from_stream_to_vec3(data),
            front: CubeElement::face_from_stream(data),
            back: CubeElement::face_from_stream(data),
            left: CubeElement::face_from_stream(data),
            right: CubeElement::face_from_stream(data),
            up: CubeElement::face_from_stream(data),
            down: CubeElement::face_from_stream(data),
        }
    }
    fn face_from_stream(data: &mut &[u8]) -> CubeElementFace {
        CubeElementFace {
            u1: data.read_be().unwrap(),
            v1: data.read_be().unwrap(),
            u2: data.read_be().unwrap(),
            v2: data.read_be().unwrap(),
        }
    }
}
#[derive(Clone)]
struct CubeElementFace {
    u1: f32,
    v1: f32,
    u2: f32,
    v2: f32,
}
#[derive(Clone)]
struct AnimationData {
    position: Vec<AnimationKeyframe>,
    rotation: Vec<AnimationKeyframe>,
    scale: Vec<AnimationKeyframe>,
}
impl AnimationData {
    pub fn from_stream(data: &mut &[u8]) -> Self {
        Self {
            position: AnimationData::animation_keyframes_from_stream(data),
            rotation: AnimationData::animation_keyframes_from_stream(data),
            scale: AnimationData::animation_keyframes_from_stream(data),
        }
    }
    fn animation_keyframes_from_stream(data: &mut &[u8]) -> Vec<AnimationKeyframe> {
        let size: u32 = data.read_be().unwrap();
        let mut keyframes = Vec::with_capacity(size as usize);
        for _ in 0..size {
            keyframes.push(AnimationKeyframe {
                data: from_stream_to_vec3(data),
                time: data.read_be().unwrap(),
            })
        }
        keyframes
    }
    pub fn get_for_time(&self, time: f32) -> (Vec3, Vec3, Vec3) {
        (
            AnimationData::get_channel_for_time(&self.position, time, 0.),
            AnimationData::get_channel_for_time(&self.rotation, time, 0.),
            AnimationData::get_channel_for_time(&self.scale, time, 1.),
        )
    }
    fn get_channel_for_time(
        keyframes: &Vec<AnimationKeyframe>,
        time: f32,
        default_value: f32,
    ) -> Vec3 {
        let mut closest_time = f32::MAX;
        let mut closest = None;
        for keyframe in keyframes.iter().enumerate() {
            let time_diff = (keyframe.1.time - time).abs();
            if time_diff < closest_time {
                closest_time = time_diff;
                closest = Some(keyframe);
            }
        }
        if let None = closest {
            return Vec3::new(default_value, default_value, default_value);
        }
        let closest = closest.unwrap();
        let second = keyframes
            .get((closest.0 as i32 + (if closest.1.time < time { 1i32 } else { -1i32 })) as usize);
        let mut first = closest.1;
        let mut second = if let Some(second) = second {
            second
        } else {
            return first.data;
        };
        if second.time < first.time {
            (first, second) = (second, first);
        }
        let lerp_val = (time - first.time) / (second.time - first.time);
        Vec3 {
            x: (first.data.x * (1. - lerp_val)) + (second.data.x * lerp_val),
            y: (first.data.y * (1. - lerp_val)) + (second.data.y * lerp_val),
            z: (first.data.z * (1. - lerp_val)) + (second.data.z * lerp_val),
        }
        /*let keyframes_sorted = keyframes_sorted.iter();
        let first = keyframes_sorted.next();
        let second = keyframes_sorted.next().and(first);
        let first = first.map(||)*/
    }
}
#[derive(Clone, Copy)]
struct AnimationKeyframe {
    data: Vec3,
    time: f32,
}
fn from_stream_to_vec3(data: &mut &[u8]) -> Vec3 {
    Vec3 {
        x: data.read_be().unwrap(),
        y: data.read_be().unwrap(),
        z: data.read_be().unwrap(),
    }
}
#[derive(Clone)]
struct Animation {
    name: String,
    length: f32,
}
