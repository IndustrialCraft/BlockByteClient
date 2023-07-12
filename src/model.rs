use endio::LERead;
use endio::LEWrite;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use ultraviolet::Mat4;
use ultraviolet::Vec2;
use ultraviolet::Vec3;
use ultraviolet::Vec4;

use crate::game::AtlassedTexture;
use crate::glwrappers::Vertex;
use crate::util;
use crate::util::Corner;
use crate::util::ItemRenderData;
use crate::util::ItemSlot;
use crate::util::Position;
use crate::TextureAtlas;
#[derive(Clone)]
pub struct Model {
    root_bone: Bone,
    animations: Vec<Animation>,
    texture: AtlassedTexture,
    animation_mapping: Vec<u32>,
    item_mapping: Vec<String>,
}
impl Model {
    pub fn new_from_file(
        file: &Path,
        texture: AtlassedTexture,
        animation_mapping: Vec<String>,
        item_mapping: Vec<String>,
    ) -> Self {
        Model::new(
            std::fs::read(file).unwrap(),
            texture,
            animation_mapping,
            item_mapping,
        )
    }
    pub fn new(
        data: Vec<u8>,
        texture: AtlassedTexture,
        animation_mapping: Vec<String>,
        item_mapping: Vec<String>,
    ) -> Self {
        let mut data = data.as_slice();
        let root_bone = Bone::from_stream(&mut data, &item_mapping);
        let animations = {
            let animations_cnt: u32 = data.read_be().unwrap();
            let mut animations = Vec::with_capacity(animations_cnt as usize);
            for _ in 0..animations_cnt {
                animations.push(Animation {
                    name: util::read_string(&mut data),
                    length: data.read_be().unwrap(),
                })
            }
            animations
        };
        let animation_mapping = {
            let mut mapping = Vec::new();
            for animation in animation_mapping {
                for i in 0..animations.len() {
                    let search_animation = animations.get(i).unwrap();
                    if search_animation.name == animation {
                        mapping.push(i as u32);
                    }
                }
            }
            mapping
        };
        Model {
            root_bone,
            animations,
            texture,
            animation_mapping,
            item_mapping,
        }
    }
    pub fn add_vertices<F>(
        &self,
        vertex_consumer: &mut F,
        animation: Option<(u32, f32)>,
        position: Vec3,
        rotation: Vec3,
        rotation_origin: Vec3,
        scale: Vec3,
        item_rendering: Option<(&HashMap<u32, ItemSlot>, &ItemRenderer)>,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let mut animation_id = None;
        let mut animation_length = 1f32;
        if let Some((animation, time)) = animation {
            if let Some(mapping) = self.animation_mapping.get(animation as usize) {
                if let Some(animation) = self.animations.get(*mapping as usize) {
                    animation_id = Some((*mapping, time));
                    animation_length = animation.length;
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
            item_rendering,
        );
    }
    pub fn add_vertices_simple<F>(
        &self,
        vertex_consumer: &mut F,
        animation: Option<(String, f32)>,
        position: Vec3,
        item_rendering: Option<(&HashMap<u32, ItemSlot>, &ItemRenderer)>,
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
            item_rendering,
        );
    }
}
#[derive(Clone)]
struct Bone {
    child_bones: Vec<Bone>,
    cube_elements: Vec<CubeElement>,
    animations: BTreeMap<u32, AnimationData>,
    origin: Vec3,
    name: String,
    item_mapping: Vec<(u32, ItemElement)>,
}
impl Bone {
    pub fn from_stream(data: &mut &[u8], item_mapping: &Vec<String>) -> Self {
        let name = util::read_string(data);
        let origin = from_stream_to_vec3(data);
        let child_bones = {
            let child_bones_cnt: u32 = data.read_be().unwrap();
            let mut child_bones = Vec::with_capacity(child_bones_cnt as usize);
            for _ in 0..child_bones_cnt {
                child_bones.push(Bone::from_stream(data, item_mapping));
            }
            child_bones
        };
        let cube_elements = {
            let cube_elements_cnt: u32 = data.read_be().unwrap();
            let mut cube_elements = Vec::with_capacity(cube_elements_cnt as usize);
            for _ in 0..cube_elements_cnt {
                cube_elements.push(CubeElement::from_stream(data));
            }
            cube_elements
        };
        let item_mapping = {
            let item_elements_cnt: u32 = data.read_be().unwrap();
            let mut items = Vec::new();
            for _ in 0..item_elements_cnt {
                let name = util::read_string(data);
                let item_element = ItemElement::from_stream(data);
                for (index, item) in item_mapping.iter().enumerate() {
                    if item == &name {
                        items.push((index as u32, item_element));
                        break;
                    }
                }
            }
            items
        };
        let animations = {
            let mut animation = BTreeMap::new();
            let animations_cnt: u32 = data.read_be().unwrap();
            for _ in 0..animations_cnt {
                animation.insert(data.read_be().unwrap(), AnimationData::from_stream(data));
            }
            animation
        };
        Bone {
            item_mapping,
            name,
            origin,
            child_bones,
            cube_elements,
            animations,
        }
    }
    pub fn add_vertices<F>(
        &self,
        vertex_consumer: &mut F,
        animation: Option<(u32, f32)>,
        parent_matrix: Mat4,
        texture: &AtlassedTexture,
        item_rendering: Option<(&HashMap<u32, ItemSlot>, &ItemRenderer)>,
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
            child.add_vertices(
                vertex_consumer,
                animation,
                bone_matrix,
                texture,
                item_rendering,
            );
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
        if let Some(item_rendering) = item_rendering {
            for id in &self.item_mapping {
                if let Some(item) = item_rendering.0.get(&id.0) {
                    item_rendering.1.add_vertices(
                        vertex_consumer,
                        item,
                        &(bone_matrix * id.1.create_matrix()),
                        &id.1.size,
                    );
                }
            }
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
        Bone::create_face(
            vertex_consumer,
            p000,
            Corner::UpLeft,
            p100,
            Corner::UpRight,
            p101,
            Corner::DownRight,
            p001,
            Corner::DownLeft,
            down,
            texture,
        );
        Bone::create_face(
            vertex_consumer,
            p010,
            Corner::UpLeft,
            p011,
            Corner::UpRight,
            p111,
            Corner::DownRight,
            p110,
            Corner::DownRight,
            up,
            texture,
        );
        Bone::create_face(
            vertex_consumer,
            p000,
            Corner::DownLeft,
            p001,
            Corner::DownRight,
            p011,
            Corner::UpRight,
            p010,
            Corner::UpLeft,
            west,
            texture,
        );
        Bone::create_face(
            vertex_consumer,
            p100,
            Corner::DownLeft,
            p110,
            Corner::UpLeft,
            p111,
            Corner::UpRight,
            p101,
            Corner::DownRight,
            east,
            texture,
        );
        Bone::create_face(
            vertex_consumer,
            p000,
            Corner::DownLeft,
            p010,
            Corner::UpLeft,
            p110,
            Corner::UpRight,
            p100,
            Corner::DownRight,
            south,
            texture,
        );
        Bone::create_face(
            vertex_consumer,
            p001,
            Corner::DownLeft,
            p101,
            Corner::DownRight,
            p111,
            Corner::UpRight,
            p011,
            Corner::UpLeft,
            north,
            texture,
        );
    }
    fn create_face<F>(
        vertex_consumer: &mut F,
        p1: Vec4,
        pc1: Corner,
        p2: Vec4,
        pc2: Corner,
        p3: Vec4,
        pc3: Corner,
        p4: Vec4,
        pc4: Corner,
        uv: &CubeElementFace,
        texture: &AtlassedTexture,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let uv = {
            let uv1 = texture.map_uv((uv.u1, uv.v1));
            let uv2 = texture.map_uv((uv.u2, uv.v2));
            (uv1.0, uv1.1, uv2.0, uv2.1)
        };
        let uv1 = pc1.map(uv);
        let uv2 = pc2.map(uv);
        let uv3 = pc3.map(uv);
        let uv4 = pc4.map(uv);

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
struct ItemElement {
    position: Vec3,
    rotation: Vec3,
    origin: Vec3,
    size: Vec2,
}
impl ItemElement {
    pub fn from_stream(data: &mut &[u8]) -> Self {
        Self {
            position: from_stream_to_vec3(data),
            rotation: from_stream_to_vec3(data),
            origin: from_stream_to_vec3(data),
            size: from_stream_to_vec2(data),
        }
    }
    pub fn create_matrix(&self) -> Mat4 {
        Mat4::from_translation(Vec3 {
            x: self.position.x,
            y: self.position.y,
            z: self.position.z,
        }) * Bone::create_rotation_matrix_with_origin(
            &Vec3 {
                x: self.rotation.x,
                y: self.rotation.y + 180f32.to_radians(),
                z: self.rotation.z,
            },
            &self.origin,
        )
    }
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
fn from_stream_to_vec2(data: &mut &[u8]) -> Vec2 {
    Vec2 {
        x: data.read_be().unwrap(),
        y: data.read_be().unwrap(),
    }
}
#[derive(Clone)]
struct Animation {
    name: String,
    length: f32,
}

pub struct ItemRenderer<'a> {
    pub items: &'a HashMap<u32, ItemRenderData>,
    pub texture_atlas: &'a TextureAtlas,
}
impl<'a> ItemRenderer<'a> {
    pub fn add_vertices<F>(
        &self,
        vertex_consumer: &mut F,
        item: &ItemSlot,
        transformation: &Mat4,
        scale: &Vec2,
    ) where
        F: FnMut(Vec3, f32, f32),
    {
        let render_data = self.items.get(&item.item).unwrap();
        println!("rendering: {}", render_data.name);
        match &render_data.model {
            util::ItemModel::Texture(texture) => {
                println!("rendering_texture: {}", texture);
                let texture = self.texture_atlas.get(texture);
                Bone::create_face(
                    vertex_consumer,
                    *transformation * Vec4::new(0., 0., 0., 1.),
                    Corner::DownLeft,
                    *transformation * Vec4::new(0., 0., 1., 1.),
                    Corner::UpLeft,
                    *transformation * Vec4::new(1., 0., 1., 1.),
                    Corner::UpRight,
                    *transformation * Vec4::new(1., 0., 0., 1.),
                    Corner::DownRight,
                    &CubeElementFace {
                        u1: 0.,
                        v1: 0.,
                        u2: 1.,
                        v2: 1.,
                    },
                    texture,
                );
            }
            util::ItemModel::Block(_) => {}
        }
    }
}
