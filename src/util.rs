use endio::LERead;
use endio::LEWrite;

use enum_iterator::Sequence;

use json::JsonValue;
use ultraviolet::*;

pub struct BlockRenderData {
    pub json: JsonValue,
}
pub struct EntityRenderData {
    pub model: String,
    pub texture: String,
}
pub struct ItemRenderData {
    pub name: String,
    pub texture: String,
}
#[repr(u8)]
pub enum NetworkMessageS2C {
    SetBlock(i32, i32, i32, u32) = 0,
    LoadChunk(i32, i32, i32, Vec<u8>) = 1,
    UnloadChunk(i32, i32, i32) = 2,
    AddEntity(u32, u32, f32, f32, f32, f32) = 3,
    MoveEntity(u32, f32, f32, f32, f32) = 4,
    DeleteEntity(u32) = 5,
    InitializeContent(
        Vec<BlockRenderData>,
        Vec<EntityRenderData>,
        Vec<ItemRenderData>,
    ) = 6,
    GuiData(json::JsonValue) = 7,
}
fn write_string(data: &mut Vec<u8>, value: &String) {
    data.write_be(value.len() as u16).unwrap();
    for ch in value.as_bytes() {
        data.write_be(*ch).unwrap();
    }
}
fn read_string(data: &mut &[u8]) -> String {
    let len: u16 = data.read_be().unwrap();
    let mut str = Vec::new();
    for _ in 0..len {
        let ch: u8 = data.read_be().unwrap();
        str.push(ch);
    }
    let str = String::from_utf8(str).unwrap();
    str
}
impl NetworkMessageS2C {
    pub fn from_data(mut data: &[u8]) -> Option<Self> {
        let id: u8 = data.read_be().unwrap();
        match id {
            0 => Some(NetworkMessageS2C::SetBlock(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            1 => Some(NetworkMessageS2C::LoadChunk(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                {
                    let length: u32 = data.read_be().unwrap();
                    let mut blocks_data: Vec<u8> = Vec::with_capacity(length as usize);
                    for _ in 0..length {
                        blocks_data.push(data.read_be().unwrap());
                    }
                    blocks_data
                },
            )),
            2 => Some(NetworkMessageS2C::UnloadChunk(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            3 => Some(NetworkMessageS2C::AddEntity(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            4 => Some(NetworkMessageS2C::MoveEntity(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            5 => Some(NetworkMessageS2C::DeleteEntity(data.read_be().unwrap())),
            6 => {
                let mut blocks = Vec::new();
                let mut entities = Vec::new();
                let mut items = Vec::new();
                let size: u16 = data.read_be().unwrap();
                for _ in 0..size {
                    let json = read_string(&mut data);
                    blocks.push(BlockRenderData {
                        json: json::parse(json.as_str()).unwrap(),
                    });
                }
                let size: u16 = data.read_be().unwrap();
                for _ in 0..size {
                    let model = read_string(&mut data);
                    let texture = read_string(&mut data);
                    entities.push(EntityRenderData { model, texture });
                }
                let size: u16 = data.read_be().unwrap();
                for _ in 0..size {
                    let name = read_string(&mut data);
                    let texture = read_string(&mut data);
                    items.push(ItemRenderData { name, texture });
                }
                Some(NetworkMessageS2C::InitializeContent(
                    blocks, entities, items,
                ))
            }
            7 => Some(NetworkMessageS2C::GuiData(
                json::parse(read_string(&mut data).as_str()).unwrap(),
            )),
            _ => None,
        }
    }
}
pub enum NetworkMessageC2S {
    LeftClickBlock(i32, i32, i32),
    RightClickBlock(i32, i32, i32, Face, bool),
    PlayerPosition(f32, f32, f32, bool, f32),
    MouseScroll(i32, i32),
    Keyboard(i32, bool, bool),
    GuiClick(String, MouseButton),
    GuiClose,
}
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum MouseButton {
    LEFT = 0,
    RIGHT = 1,
}
impl NetworkMessageC2S {
    pub fn to_data(&self) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        match self {
            Self::LeftClickBlock(x, y, z) => {
                data.write_be(0u8).unwrap();
                data.write_be(x.to_owned()).unwrap();
                data.write_be(y.to_owned()).unwrap();
                data.write_be(z.to_owned()).unwrap();
            }
            Self::RightClickBlock(x, y, z, face, shifting) => {
                data.write_be(1u8).unwrap();
                data.write_be(x.to_owned()).unwrap();
                data.write_be(y.to_owned()).unwrap();
                data.write_be(z.to_owned()).unwrap();
                data.write_be(face.to_owned() as u8).unwrap();
                data.write_be(shifting.to_owned()).unwrap();
            }
            Self::PlayerPosition(x, y, z, shifting, rotation) => {
                data.write_be(2u8).unwrap();
                data.write_be(x.to_owned()).unwrap();
                data.write_be(y.to_owned()).unwrap();
                data.write_be(z.to_owned()).unwrap();
                data.write_be(shifting.to_owned()).unwrap();
                data.write_be(rotation.to_owned()).unwrap();
            }
            Self::MouseScroll(x, y) => {
                data.write_be(3u8).unwrap();
                data.write_be(*x).unwrap();
                data.write_be(*y).unwrap();
            }
            Self::Keyboard(key, down, repeat) => {
                data.write_be(4u8).unwrap();
                data.write_be(*key).unwrap();
                data.write_be(*down).unwrap();
                data.write_be(*repeat).unwrap();
            }
            Self::GuiClick(id, button) => {
                data.write_be(5u8).unwrap();
                write_string(&mut data, id);
                data.write_be((*button) as u8).unwrap();
            }
            Self::GuiClose => {
                data.write_be(6u8).unwrap();
            }
        };
        data
    }
}

#[derive(Sequence, Debug, Clone, Copy)]
pub enum Face {
    Front = 0,
    Back = 1,
    Up = 2,
    Down = 3,
    Left = 4,
    Right = 5,
}
impl Face {
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Face::Front),
            1 => Some(Face::Back),
            2 => Some(Face::Up),
            3 => Some(Face::Down),
            4 => Some(Face::Left),
            5 => Some(Face::Right),
            _ => None,
        }
    }
    pub fn get_vertices(&self) -> [Vec3; 4] {
        match self {
            Self::Up => [
                Vec3 {
                    x: 0.,
                    y: 1.,
                    z: 0.,
                },
                Vec3 {
                    x: 1.,
                    y: 1.,
                    z: 0.,
                },
                Vec3 {
                    x: 1.,
                    y: 1.,
                    z: 1.,
                },
                Vec3 {
                    x: 0.,
                    y: 1.,
                    z: 1.,
                },
            ],
            Self::Down => [
                Vec3 {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                },
                Vec3 {
                    x: 1.,
                    y: 0.,
                    z: 0.,
                },
                Vec3 {
                    x: 1.,
                    y: 0.,
                    z: 1.,
                },
                Vec3 {
                    x: 0.,
                    y: 0.,
                    z: 1.,
                },
            ],
            Self::Front => [
                Vec3 {
                    x: 1.,
                    y: 1.,
                    z: 0.,
                },
                Vec3 {
                    x: 0.,
                    y: 1.,
                    z: 0.,
                },
                Vec3 {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                },
                Vec3 {
                    x: 1.,
                    y: 0.,
                    z: 0.,
                },
            ],
            Self::Back => [
                Vec3 {
                    x: 1.,
                    y: 1.,
                    z: 1.,
                },
                Vec3 {
                    x: 0.,
                    y: 1.,
                    z: 1.,
                },
                Vec3 {
                    x: 0.,
                    y: 0.,
                    z: 1.,
                },
                Vec3 {
                    x: 1.,
                    y: 0.,
                    z: 1.,
                },
            ],
            Self::Left => [
                Vec3 {
                    x: 0.,
                    y: 1.,
                    z: 0.,
                },
                Vec3 {
                    x: 0.,
                    y: 1.,
                    z: 1.,
                },
                Vec3 {
                    x: 0.,
                    y: 0.,
                    z: 1.,
                },
                Vec3 {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                },
            ],
            Self::Right => [
                Vec3 {
                    x: 1.,
                    y: 1.,
                    z: 0.,
                },
                Vec3 {
                    x: 1.,
                    y: 1.,
                    z: 1.,
                },
                Vec3 {
                    x: 1.,
                    y: 0.,
                    z: 1.,
                },
                Vec3 {
                    x: 1.,
                    y: 0.,
                    z: 0.,
                },
            ],
        }
    }
    pub fn get_offset(&self) -> BlockPosition {
        match self {
            Self::Front => BlockPosition { x: 0, y: 0, z: -1 },
            Self::Back => BlockPosition { x: 0, y: 0, z: 1 },
            Self::Left => BlockPosition { x: -1, y: 0, z: 0 },
            Self::Right => BlockPosition { x: 1, y: 0, z: 0 },
            Self::Up => BlockPosition { x: 0, y: 1, z: 0 },
            Self::Down => BlockPosition { x: 0, y: -1, z: 0 },
        }
    }
    pub fn opposite(&self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Front => Self::Back,
            Self::Back => Self::Front,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl Position {
    pub fn new(pos: ultraviolet::Vec3) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        }
    }
    pub fn add_other(&self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
    pub fn add(&self, x: f32, y: f32, z: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            z: self.z + z,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl std::ops::Add for BlockPosition {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        BlockPosition {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}
impl BlockPosition {
    pub fn is_inside_origin_chunk(&self) -> bool {
        self.x >= 0 && self.x <= 15 && self.y >= 0 && self.y <= 15 && self.z >= 0 && self.z <= 15
    }
    pub fn chunk_offset(&self) -> (u8, u8, u8) {
        (
            self.x.rem_euclid(16) as u8,
            self.y.rem_euclid(16) as u8,
            self.z.rem_euclid(16) as u8,
        )
    }
    pub fn to_chunk_pos(&self) -> ChunkPosition {
        ChunkPosition {
            x: ((self.x as f32) / 16f32).floor() as i32,
            y: ((self.y as f32) / 16f32).floor() as i32,
            z: ((self.z as f32) / 16f32).floor() as i32,
        }
    }
}
impl Position {
    pub fn to_chunk_pos(&self) -> ChunkPosition {
        ChunkPosition {
            x: ((self.x as f32) / 16f32).floor() as i32,
            y: ((self.y as f32) / 16f32).floor() as i32,
            z: ((self.z as f32) / 16f32).floor() as i32,
        }
    }
    pub fn to_block_pos(&self) -> BlockPosition {
        BlockPosition {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl ChunkPosition {
    pub fn with_offset(&self, face: &Face) -> Self {
        let offset = face.get_offset();
        ChunkPosition {
            x: self.x + offset.x,
            y: self.y + offset.y,
            z: self.z + offset.z,
        }
    }
    pub fn add(&self, x: i32, y: i32, z: i32) -> Self {
        ChunkPosition {
            x: self.x + x,
            y: self.y + y,
            z: self.z + z,
        }
    }
}
