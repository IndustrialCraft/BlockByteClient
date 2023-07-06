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
    pub hitbox_w: f32,
    pub hitbox_h: f32,
    pub hitbox_d: f32,
}
pub struct ItemRenderData {
    pub name: String,
    pub model: ItemModel,
}
pub enum ItemModel {
    Texture(String),
    Block(u32),
}
#[repr(u8)]
pub enum NetworkMessageS2C {
    SetBlock(i32, i32, i32, u32) = 0,
    LoadChunk(i32, i32, i32, Vec<u8>) = 1,
    UnloadChunk(i32, i32, i32) = 2,
    AddEntity(u32, u32, f32, f32, f32, f32, u32, f32) = 3,
    MoveEntity(u32, f32, f32, f32, f32) = 4,
    DeleteEntity(u32) = 5,
    GuiData(json::JsonValue) = 6,
    BlockBreakTimeResponse(u32, f32) = 7,
    EntityAddItem(u32, u32, u32) = 8,
    BlockAddItem(i32, i32, i32, f32, f32, f32, u32, u32) = 9,
    BlockRemoveItem(i32, i32, i32, u32) = 10,
    BlockMoveItem(i32, i32, i32, f32, f32, f32, u32) = 11,
    Knockback(f32, f32, f32, bool) = 12,
    FluidSelectable(bool) = 13,
    PlaySound(String, f32, f32, f32, f32, f32, bool) = 14,
    EntityAnimation(u32, u32) = 15,
    ChatMessage(String) = 16,
    PlayerAbilities(f32, MovementType) = 17,
    TeleportPlayer(f32, f32, f32) = 18,
    BlockAnimation(i32, i32, i32, u32) = 19,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MovementType {
    Normal,
    Fly,
    NoClip,
}
impl MovementType {
    pub fn from_data(data: &mut &[u8]) -> Self {
        let id: u8 = data.read_be().unwrap();
        match id {
            0 => MovementType::Normal,
            1 => MovementType::Fly,
            2 => MovementType::NoClip,
            _ => panic!("unknown movementtype"),
        }
    }
}
fn write_string(data: &mut Vec<u8>, value: &String) {
    data.write_be(value.len() as u16).unwrap();
    for ch in value.as_bytes() {
        data.write_be(*ch).unwrap();
    }
}
pub fn read_string(data: &mut &[u8]) -> String {
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
            6 => Some(NetworkMessageS2C::GuiData(
                json::parse(read_string(&mut data).as_str()).unwrap(),
            )),
            7 => Some(NetworkMessageS2C::BlockBreakTimeResponse(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            8 => Some(NetworkMessageS2C::EntityAddItem(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            9 => Some(Self::BlockAddItem(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            10 => Some(Self::BlockRemoveItem(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            11 => Some(Self::BlockMoveItem(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            12 => Some(Self::Knockback(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            13 => Some(Self::FluidSelectable(data.read_be().unwrap())),
            14 => Some(Self::PlaySound(
                read_string(&mut data),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            15 => Some(Self::EntityAnimation(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            16 => Some(Self::ChatMessage(read_string(&mut data))),
            17 => Some(Self::PlayerAbilities(
                data.read_be().unwrap(),
                MovementType::from_data(&mut data),
            )),
            18 => Some(Self::TeleportPlayer(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            19 => Some(Self::BlockAnimation(
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
                data.read_be().unwrap(),
            )),
            _ => None,
        }
    }
}
pub enum NetworkMessageC2S {
    BreakBlock(i32, i32, i32),
    RightClickBlock(i32, i32, i32, Face, bool),
    PlayerPosition(f32, f32, f32, bool, f32, bool),
    MouseScroll(i32, i32),
    Keyboard(i32, bool, bool),
    GuiClick(String, MouseButton, bool),
    GuiClose,
    RequestBlockBreakTime(u32, BlockPosition),
    LeftClickEntity(u32),
    RightClickEntity(u32),
    GuiScroll(String, i32, i32, bool),
    RightClick(bool),
    SendMessage(String),
    ConnectionMode,
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
            Self::BreakBlock(x, y, z) => {
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
            Self::PlayerPosition(x, y, z, shifting, rotation, moving) => {
                data.write_be(2u8).unwrap();
                data.write_be(x.to_owned()).unwrap();
                data.write_be(y.to_owned()).unwrap();
                data.write_be(z.to_owned()).unwrap();
                data.write_be(shifting.to_owned()).unwrap();
                data.write_be(rotation.to_owned()).unwrap();
                data.write_be(moving.to_owned()).unwrap();
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
            Self::GuiClick(id, button, shift) => {
                data.write_be(5u8).unwrap();
                write_string(&mut data, id);
                data.write_be((*button) as u8).unwrap();
                data.write_be(*shift).unwrap();
            }
            Self::GuiClose => {
                data.write_be(6u8).unwrap();
            }
            Self::RequestBlockBreakTime(id, block_position) => {
                data.write_be(7u8).unwrap();
                data.write_be(*id).unwrap();
                data.write_be(block_position.x).unwrap();
                data.write_be(block_position.y).unwrap();
                data.write_be(block_position.z).unwrap();
            }
            Self::LeftClickEntity(id) => {
                data.write_be(8u8).unwrap();
                data.write_be(*id).unwrap();
            }
            Self::RightClickEntity(id) => {
                data.write_be(9u8).unwrap();
                data.write_be(*id).unwrap();
            }
            Self::GuiScroll(id, x, y, shifting) => {
                data.write_be(10u8).unwrap();
                write_string(&mut data, id);
                data.write_be(*x).unwrap();
                data.write_be(*y).unwrap();
                data.write_be(*shifting).unwrap();
            }
            Self::RightClick(shifting) => {
                data.write_be(11u8).unwrap();
                data.write_be(*shifting).unwrap();
            }
            Self::SendMessage(message) => {
                data.write_be(12u8).unwrap();
                write_string(&mut data, message);
            }
            Self::ConnectionMode => {
                data.write_be(13u8).unwrap();
                data.write_be(0u8).unwrap();
            }
        };
        data
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}
impl Corner {
    pub fn map(&self, coords: (f32, f32, f32, f32)) -> (f32, f32) {
        match self {
            Self::UpLeft => (coords.0, coords.1),
            Self::UpRight => (coords.2, coords.1),
            Self::DownLeft => (coords.0, coords.3),
            Self::DownRight => (coords.2, coords.3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Front = 0,
    Back = 1,
    Up = 2,
    Down = 3,
    Left = 4,
    Right = 5,
}
impl Face {
    const FACES: [Face; 6] = [
        Face::Front,
        Face::Back,
        Face::Up,
        Face::Down,
        Face::Left,
        Face::Right,
    ];
    pub fn all() -> &'static [Face; 6] {
        &Face::FACES
    }
    pub fn get_name(&self) -> &str {
        match self {
            Face::Front => "front",
            Face::Back => "back",
            Face::Left => "left",
            Face::Right => "right",
            Face::Up => "up",
            Face::Down => "down",
        }
    }
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
    pub fn get_vertices(&self) -> [(Vec3, Corner); 4] {
        match self {
            Self::Up => [
                (
                    Vec3 {
                        x: 0.,
                        y: 1.,
                        z: 0.,
                    },
                    Corner::UpLeft,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 1.,
                        z: 1.,
                    },
                    Corner::DownLeft,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 1.,
                        z: 1.,
                    },
                    Corner::DownRight,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 1.,
                        z: 0.,
                    },
                    Corner::UpRight,
                ),
            ],
            Self::Down => [
                (
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    },
                    Corner::UpLeft,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 0.,
                        z: 0.,
                    },
                    Corner::UpRight,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 0.,
                        z: 1.,
                    },
                    Corner::DownRight,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 1.,
                    },
                    Corner::DownLeft,
                ),
            ],
            Self::Front => [
                (
                    Vec3 {
                        x: 1.,
                        y: 1.,
                        z: 0.,
                    },
                    Corner::UpLeft,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 0.,
                        z: 0.,
                    },
                    Corner::DownLeft,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    },
                    Corner::DownRight,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 1.,
                        z: 0.,
                    },
                    Corner::UpRight,
                ),
            ],
            Self::Back => [
                (
                    Vec3 {
                        x: 1.,
                        y: 1.,
                        z: 1.,
                    },
                    Corner::UpRight,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 1.,
                        z: 1.,
                    },
                    Corner::UpLeft,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 1.,
                    },
                    Corner::DownLeft,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 0.,
                        z: 1.,
                    },
                    Corner::DownRight,
                ),
            ],
            Self::Left => [
                (
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 1.,
                    },
                    Corner::DownRight,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 1.,
                        z: 1.,
                    },
                    Corner::UpRight,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 1.,
                        z: 0.,
                    },
                    Corner::UpLeft,
                ),
                (
                    Vec3 {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    },
                    Corner::DownLeft,
                ),
            ],
            Self::Right => [
                (
                    Vec3 {
                        x: 1.,
                        y: 1.,
                        z: 0.,
                    },
                    Corner::UpRight,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 1.,
                        z: 1.,
                    },
                    Corner::UpLeft,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 0.,
                        z: 1.,
                    },
                    Corner::DownLeft,
                ),
                (
                    Vec3 {
                        x: 1.,
                        y: 0.,
                        z: 0.,
                    },
                    Corner::DownRight,
                ),
            ],
        }
    }
    #[inline(always)]
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
    #[inline(always)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    #[inline(always)]
    pub fn offset_from_origin_chunk(&self) -> Option<Face> {
        if self.x < 0 {
            return Some(Face::Left);
        }
        if self.x >= 16 {
            return Some(Face::Right);
        }
        if self.y < 0 {
            return Some(Face::Down);
        }
        if self.y >= 16 {
            return Some(Face::Up);
        }
        if self.z < 0 {
            return Some(Face::Front);
        }
        if self.z >= 16 {
            return Some(Face::Back);
        }
        return None;
    }
    #[inline(always)]
    pub fn chunk_offset(&self) -> (u8, u8, u8) {
        (
            self.x.rem_euclid(16) as u8,
            self.y.rem_euclid(16) as u8,
            self.z.rem_euclid(16) as u8,
        )
    }
    #[inline(always)]
    pub fn to_chunk_pos(&self) -> ChunkPosition {
        ChunkPosition {
            x: ((self.x as f32) / 16f32).floor() as i32,
            y: ((self.y as f32) / 16f32).floor() as i32,
            z: ((self.z as f32) / 16f32).floor() as i32,
        }
    }
    #[inline(always)]
    pub fn to_position(&self) -> Position {
        Position {
            x: self.x as f32,
            y: self.y as f32,
            z: self.z as f32,
        }
    }
}
impl Position {
    #[inline(always)]
    pub fn to_chunk_pos(&self) -> ChunkPosition {
        ChunkPosition {
            x: ((self.x as f32) / 16f32).floor() as i32,
            y: ((self.y as f32) / 16f32).floor() as i32,
            z: ((self.z as f32) / 16f32).floor() as i32,
        }
    }
    #[inline(always)]
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
    pub fn distance_squared(&self, other: &ChunkPosition) -> u32 {
        let xd = self.x - other.x;
        let yd = self.y - other.y;
        let zd = self.z - other.z;
        (xd * xd + yd * yd + zd * zd) as u32
    }
}
