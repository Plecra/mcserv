use crate::prelude::*;

const STATUS: &[u8] = br#"{
    "version": {"name": "1.19","protocol": 759},
    "players": {"max": 1,"online": 0,"sample": []},
    "description": ["",{"text":"g","color":"red"},{"text":"a","color":"gold"},{"text":"a","color":"yellow"},{"text":"a","color":"dark_green"},{"text":"a","color":"aqua"},{"text":"a","color":"blue"},{"text":"y","color":"dark_purple"},{"text":" minecraft"}]
}"#;
macro_rules! response {
    {$world:ident, $pid:ident; $($name:ident($($field:ident : $t:ty),*): $id:literal $e:expr)*} => {
        #[derive(Debug)]
        pub enum Response {
            $($name($($t),*),)*
        }
        impl Response {
            pub fn write<'a>(&self, $world: &crate::World, $pid: usize, pkt: &'a mut [u8]) -> &'a [u8] {
                let n = match self {
                    $(Response::$name($($field),*) => ToWire::encode(&($id as u8, $e), &mut pkt[5..]),)*
                };
                let start = 5 - var(n as u32).byte_len();
                var(n as u32).encode(&mut pkt[start..5]);
                &pkt[start..5 + n]
            }
        }
    };
}
response! {
    world, pid;
    Status(): 0 STATUS
    Pong(n: u64): 1 n
    Chat(msg: String): 0x5F (serde_json::to_string(&serde_json::json!({
        "text": msg
    })).unwrap().into_bytes(), 0u8)
    Login(name: Name): 2 (name.0, name.as_str(), &[(); 0][..])

    Play(mode: GameMode): 0x23 (
        1i32, // eid
        false, // is hardcore
        match mode {
            GameMode::Survival => 0u8,
            GameMode::Creative => 1
        },
        -1i8, // no previous gamemode

        &[0u8; 0][..], // dimensions
        ToWireFn(registry_data),

        "", // dimension type
        "", // dimension name
        0u64, // hashed seed
        2u8, // max players (ignored)
        8u8, //render distance
        8u8, // simulation distance
        false, // reduced debuginfo
        true, // respawn screen enabled
        false, // is debug world
        true, // is superflat
        None::<()>, // death location (disabled rn)
    )
    MoveFast(): 0x65 (
        1u8, // eid
        &[("generic.movement_speed",
           10.0f64,
           &[(); 0][..])][..]
    )
    Ping(): 0x2D 35453423i32
    AckBlockChange(seq: i32): 0x05 var(*seq)
    SetBlock(pos: V3<i32>, id: Option<Block>): 0x09 (
        (pos.x as i64) << 38 | ((pos.z as i64 & 0x3FFFFFF) << 12) | (pos.y as i64 & 0xFFF),
        var(id.map_or(0, |b| b.net_id()) as u32)
    )
    CenterChunk(x: i32, z: i32): 0x48 [var(*x), var(*z)]
    LoadChunk(x: i32, z: i32): 0x1F {
        let un_skylit_chunks = 0b11_1111_1111_1111_1111_1101_1111u64;
        let un_blocklit_chunks = 0b11_1111_1111_1111_1111_1111_1111u64;
        let mut chunkdata = Vec::with_capacity(10 * 1024);
        let chunk = world.chunk_at(*x, *z);
        match &chunk.content {
            crate::world::ChunkContent::OneToOne { nonaircounts, blocks } => {
                for (nonaircount, blocks) in nonaircounts.iter().zip(blocks.chunks(16 * 16 * 16 / 4)) {
                    chunkdata.extend(nonaircount.to_be_bytes());
                    if *nonaircount == 0 {
                        chunkdata.push(0);
                        chunkdata.push(0);
                        chunkdata.push(0);
                    } else {
                        chunkdata.push(15u8);
                        let n = var(4096 / 4);
                        let len = chunkdata.len();
                        chunkdata.resize(len + n.byte_len(), 0);
                        n.encode(&mut chunkdata[len..]);
                        for blocks in blocks {
                            chunkdata.extend(blocks.to_be_bytes());
                        }
                    }
                    chunkdata.extend([0, 0, 0]); // no biome data
                }
            }
        }
        // load chunk from world
        (
            x, z,
            ToWireFn(|mut dst: &mut [u8]| {
                let len = dst.len();
                fastnbt::to_writer(&mut dst, &fastnbt::nbt!({
                    "MOTION_BLOCKING": fastnbt::LongArray::new((0..((256 / (64 / 9)) + 1)).map(|_| 0).collect())
                })).unwrap();
                len - dst.len()
            }),

            // var(8 * 24),
            // [(4096u16, 0u8, 1 + (((x + z) % 2) == 0) as u8, 0u8, 0u8, 0u8, 0u8); 4],
            // [(0u16, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8); 20],
            chunkdata,
            
            &[(); 0][..], // no tile entities

            1u8, // trust server lighting

            [&[!un_skylit_chunks][..], &[!un_blocklit_chunks]],
            [&[un_skylit_chunks][..], &[un_blocklit_chunks]],
            [&[&[0xFFu8; 2048][..]][..], &[][..]]
        )
    }
    UnloadChunk(x: i32, z: i32): 0x1A (x, z)
    Position(): 0x36 (
        world.player_pos(pid),
        (0.0f32, 0.0f32),
        0b000_00000u8, // positions are relative?
        0u8, // teleport id
        false, // should dismount?
    )
    SetInventorySlot(slot: u16, id: Item, count: u8, seq: u32): 0x13 (
        0u8,
        var(*seq),
        slot,
        Some((id, count, 0u8))
    )
    SetRenderDistance(distance: u8): 0x49 distance
    SetHealth(health: f32, food: i32, saturation: f32): 0x52 (health, var(*food), saturation)
    Respawn(): 0x3B (
        &b""[..],
        &b""[..],
        0u64,
        1u8,
        -1i8,
        false,
        true,
        true,
        None::<()>,
    )
}

macro_rules! snbt {
    ({
        $($key:literal : $v:tt),*
    }) => {
        (
            $(($key, snbt!($v))),*
        )
    }
}
fn registry_data(mut pkt: &mut [u8]) -> usize {
    let len = pkt.len();
    log::warn!("using a very odd version of the world NBT data");
    fastnbt::to_writer(&mut pkt, &fastnbt::nbt! ({
        // chat is not culled
        "minecraft:chat_type": {
            "value": [
            {
                "name": "minecraft:chat",
                "id": 0,
                "element": {
                "chat": {
                    "decoration": {
                        "parameters": [
                            "sender",
                            "content"
                        ],
                        "translation_key": "chat.type.text",
                        "style": {}
                    }
                },
                "narration": {
                    "decoration": {
                    "parameters": [
                        "sender",
                        "content"
                    ],
                    "translation_key": "chat.type.text.narrate",
                    "style": {}
                    },
                    "priority": "chat"
                }
                }
            },
            {
                "element": {
                "narration": {
                    "priority": "system"
                },
                "chat": {}
                },
                "name": "minecraft:system",
                "id": 1
            },
            {
                "id": 2,
                "name": "minecraft:game_info",
                "element": {
                "overlay": {}
                }
            },
            {
                "element": {
                "narration": {
                    "priority": "chat",
                    "decoration": {
                    "style": {},
                    "parameters": [
                        "sender",
                        "content"
                    ],
                    "translation_key": "chat.type.text.narrate"
                    }
                },
                "chat": {
                    "decoration": {
                    "style": {},
                    "parameters": [
                        "sender",
                        "content"
                    ],
                    "translation_key": "chat.type.announcement"
                    }
                }
                },
                "id": 3,
                "name": "minecraft:say_command"
            },
            {
                "id": 4,
                "element": {
                "chat": {
                    "decoration": {
                    "translation_key": "commands.message.display.incoming",
                    "style": {
                        "italic": 1,
                        "color": "gray"
                    },
                    "parameters": [
                        "sender",
                        "content"
                    ]
                    }
                },
                "narration": {
                    "priority": "chat",
                    "decoration": {
                    "translation_key": "chat.type.text.narrate",
                    "style": {},
                    "parameters": [
                        "sender",
                        "content"
                    ]
                    }
                }
                },
                "name": "minecraft:msg_command"
            },
            {
                "name": "minecraft:team_msg_command",
                "id": 5,
                "element": {
                "narration": {
                    "priority": "chat",
                    "decoration": {
                    "style": {},
                    "parameters": [
                        "sender",
                        "content"
                    ],
                    "translation_key": "chat.type.text.narrate"
                    }
                },
                "chat": {
                    "decoration": {
                    "translation_key": "chat.type.team.text",
                    "style": {},
                    "parameters": [
                        "team_name",
                        "sender",
                        "content"
                    ]
                    }
                }
                }
            },
            {
                "id": 6,
                "element": {
                "chat": {
                    "decoration": {
                    "translation_key": "chat.type.emote",
                    "style": {},
                    "parameters": [
                        "sender",
                        "content"
                    ]
                    }
                },
                "narration": {
                    "priority": "chat",
                    "decoration": {
                    "style": {},
                    "translation_key": "chat.type.emote",
                    "parameters": [
                        "sender",
                        "content"
                    ]
                    }
                }
                },
                "name": "minecraft:emote_command"
            },
            {
                "element": {
                "chat": {},
                "narration": {
                    "priority": "chat"
                }
                },
                "id": 7,
                "name": "minecraft:tellraw_command"
            }
            ],
            "type": "minecraft:chat_type"
        },
        "minecraft:dimension_type": {
            "type": "minecraft:dimension_type",
            "value": [
            {
                "id": 0,
                "name": "",
                "element": {
                "ultrawarm": 0,
                "logical_height": 384,
                "infiniburn": "#minecraft:infiniburn_overworld",
                "piglin_safe": 0,
                "ambient_light": 0.0,
                "has_skylight": 1,
                "effects": "",
                "has_raids": 0,
                "monster_spawn_block_light_limit": 0,
                "respawn_anchor_works": 0,
                "height": 384,
                "has_ceiling": 0,
                "monster_spawn_light_level": {
                    "value": {
                    "max_inclusive": 7,
                    "min_inclusive": 0
                    },
                    "type": "minecraft:uniform"
                },
                "natural": 0,
                "min_y": -64,
                "coordinate_scale": 1.0,
                "bed_works": 0
                }
            }
            ]
        },
        "minecraft:worldgen/biome": {
            "type": "minecraft:worldgen/biome",
            "value": [
            {
                "id": 0,
                "element": {
                "precipitation": "none",
                "temperature": 0.5,
                "downfall": 0.5,
                "effects": {
                    "water_color": 4159204,
                    "mood_sound": {
                    "sound": "minecraft:ambient.cave",
                    "offset": 2.0,
                    "block_search_extent": 8,
                    "tick_delay": 6000
                    },
                    "water_fog_color": 329011,
                    "fog_color": 12638463,
                    "sky_color": 8103167
                }
                },
                "name": "minecraft:the_void"
            },
            {
                "id": 1,
                "name": "minecraft:plains",
                "element": {
                "temperature": 0.8,
                "precipitation": "none",
                "downfall": 0.0,
                "effects": {
                    "water_fog_color": 329011,
                    "fog_color": 12638463,
                    "water_color": 4159204,
                    "sky_color": 7907327
                }
                }
            }
            ]
        }
    })).unwrap();
    len - pkt.len()
}

trait ToWire {
    fn encode(&self, pkt: &mut [u8]) -> usize;
}
impl ToWire for () {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        0
    }
}
impl<T: ToWire + ?Sized> ToWire for &'_ T {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        (**self).encode(pkt)
    }
}
impl<T: ToWire> ToWire for [T] {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        let mut written = var(self.len()).encode(pkt);
        for v in self {
            written += v.encode(&mut pkt[written..]);
        }
        written
    }
}
impl<T: ToWire> ToWire for Vec<T> {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        self.as_slice().encode(pkt)
    }
}
impl ToWire for bool {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        (*self as u8).encode(pkt)
    }
}
impl ToWire for str {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        self.as_bytes().encode(pkt)
    }
}

use super::wire::var;
const CONTINUE_BIT: u8 = 0b1000_0000;
impl var<u32> {
    fn byte_len(&self) -> usize {
        let mut i = 0;
        let mut n = self.0;
        while n & !((!CONTINUE_BIT) as u32) != 0 {
            i += 1;
            n >>= 7;
        }
        i + 1
    }
}
impl ToWire for var<i32> {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        var(self.0 as u32).encode(pkt)
    }
}
impl ToWire for var<usize> {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        var(self.0 as u32).encode(pkt)
    }
}
impl ToWire for var<u32> {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        let mut i = 0;
        let mut n = self.0;
        while n & !((!CONTINUE_BIT) as u32) != 0 {
            pkt[i] = n as u8 & !CONTINUE_BIT | CONTINUE_BIT;
            i += 1;
            n >>= 7;
        }
        pkt[i] = n as u8;
        i + 1
    }
}
struct ToWireFn<F: Fn(&mut [u8]) -> usize>(F);
impl<F: Fn(&mut [u8]) -> usize> ToWire for ToWireFn<F> {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        (self.0)(pkt)
    }
}
impl<T: ToWire, const N: usize> ToWire for [T; N] {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        let mut written = 0;
        for v in self {
            written += v.encode(&mut pkt[written..]);
        }
        written
    }
}
impl ToWire for Block {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        var(self.net_id() as u32).encode(pkt)
    }
}
impl ToWire for Item {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        var(self.net_id() as u32).encode(pkt)
    }
}
impl<T: ToWire> ToWire for Option<T> {
    fn encode(&self, pkt: &mut [u8]) -> usize {
        match self {
            None => false.encode(pkt),
            Some(v) => (true, v).encode(pkt)
        }
    }
}

macro_rules! impl_tuple {
    ($a:ident $b:ident) => {
        impl<$a: ToWire, $b: ToWire> ToWire for ($a, $b) {
            fn encode(&self, pkt: &mut [u8]) -> usize {
                let written = self.0.encode(pkt);
                written + self.1.encode(&mut pkt[written..])
            }
        }
    };
    ($i:ident $($t:tt)*) => {
        impl<$i: ToWire, $($t: ToWire),*> ToWire for ($i, $($t),*) {
            fn encode(&self, pkt: &mut [u8]) -> usize {
                match self {
                    ($i, $($t),*) => ($i, ($($t,)*)).encode(pkt)
                }
            }
        }
        impl_tuple!($($t)*);
    }
}
impl_tuple!(A B C D E F G H I J K L M N O P Q R S T U V);
macro_rules! impl_n {
    ($($t:ident)*) => {$(
        impl ToWire for $t {
            fn encode(&self, pkt: &mut [u8]) -> usize {
                let bytes = self.to_be_bytes();
                pkt[..bytes.len()].copy_from_slice(&bytes);
                bytes.len()
            }
        }
    )*};
}
impl_n!(u8 i8 u16 i32 i64 u64 f32 f64);