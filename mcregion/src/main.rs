// use std::*;
// use io::prelude::*;
// enum Flow<T = ()> {
//     Skip(T),
//     Continue,
//     Stop,
// }
// trait Visitor {
//     fn byte(&mut self, name: &[u8], value: u8) -> Flow<!> {
//         Flow::Continue
//     }
//     fn map(&mut self, name: &[u8]) -> Flow {
//         Flow::Skip
//     }
//     fn unnamed_map(&mut self) -> Flow {
//         self.map(b"UNNAMED")
//     }
//     fn map_end(&mut self) -> Flow<!> {
//         Flow::Continue
//     }
// }
// impl<T: Visitor> Visitor for &mut T {
//     fn byte(&mut self, name: &[u8], value: u8) -> Flow<!> {
//         (*self).byte(name, value)
//     }
//     fn map(&mut self, name: &[u8]) -> Flow {
//         (*self).map(name)
//     }
//     fn unnamed_map(&mut self) -> Flow {
//         (*self).unnamed_map()
//     }
//     fn map_end(&mut self) -> Flow<!> {
//         (*self).map_end()
//     }
// }
// #[derive(Debug)]
// struct World {
//     allow_commands: bool,
// }
// fn main() {
//     let savepath = path::PathBuf::from(env::args_os().nth(1).unwrap());
//     let mut level = vec![];
//     flate2::read::GzDecoder::new(fs::File::open(savepath.join("level.dat")).unwrap())
//         .read_to_end(&mut level)
//         .unwrap();
//     impl Visitor for World {
//         fn byte(&mut self, name: &[u8], value: u8) -> bool {
//             let found = matches!((&self.1, name), (Compound::Data, b"allowCommands"));
//             self.allow_commands |= found && value == 1;
//             !found
//         }
//     }
//     macro_rules! Parent {
//         ($name:literal) => {{
//             struct Parent<T>(T, bool);
//             impl Visitor for World {
//                 fn byte(&mut self, name: &[u8], value: u8) -> Flow<!> {
//                     if self.1 {
//                         self.0.byte(name, value)
//                     } else {
//                         Flow::Continue
//                     }
//                 }
//                 fn map(&mut self, name: &[u8]) -> Flow {
//                     if self.1 {
//                         self.0.map(name)
//                     } else if name == $name {
//                         self.1 = true;
//                         Flow::Continue
//                     } else {
//                         Flow::Skip
//                     }
//                 }
//                 fn unnamed
//                 fn map_end(&mut self) -> Flow<!> {
//                     if self.1 {
//                         self.0.map_end()
//                     } else {
//                         Flow::Stop
//                     }
//                 }
//             }
//             Parent
//         }};
//     }
//     let mut world = World { allow_commands: false };
//     // parse_nbt(&level, (&mut world, Reader::In(Compound::Top)));
//     parse_nbt(&level, Parent!("")(Parent!("Data")(&mut world)));
//     println!("{:?}", world);
// }
// fn parse_nbt(nbt: &[u8], mut visitor: impl Visitor) {
//     const END: u8 = 0;
//     const BYTE: u8 = 1;
//     const SHORT: u8 = 2;
//     const INT: u8 = 3;
//     const LONG: u8 = 4;
//     const FLOAT: u8 = 5;
//     const DOUBLE: u8 = 6;
//     const STRING: u8 = 8;
//     const LIST: u8 = 9;
//     const COMPOUND: u8 = 10;
//     const INT_ARRAY: u8 = 11;
//     enum ParseState {
//         Compound,
//         CompoundList(u32),
//     }
//     let mut cursor = nbt;
//     let mut stack = vec![ParseState::Compound];
//     while let Some(state) = stack.last_mut() {
//         match state {
//             ParseState::CompoundList(0) => {
//                 stack.pop();
//             }
//             ParseState::CompoundList(els) => {
//                 *els -= 1;
//                 visitor = visitor.unnamed_map().unwrap();
//                 stack.push(ParseState::Compound);
//             }
//             ParseState::Compound => {
//                 if cursor.is_empty() || cursor[0] == END {
//                     if !cursor.is_empty() {
//                         cursor = &cursor[1..];
//                     }
//                     stack.pop();
//                     if let Some(_visitor) = visitor.map_end() {
//                         visitor = _visitor;
//                     } else {
//                         break;
//                     }
//                     continue;
//                 }
//                 let tag = cursor[0];
//                 let name_len = u16::from_be_bytes(cursor[1..3].try_into().unwrap());
//                 let (name, rem) = cursor[3..].split_at(name_len as usize);
//                 match tag {
//                     BYTE => {
//                         cursor = &rem[1..];
//                         if let Some(read) = visitor.byte(name, rem[0]) {
//                             visitor = read;
//                         } else {
//                             break;
//                         }
//                     },
//                     SHORT => cursor = &rem[2..],
//                     INT => cursor = &rem[4..],
//                     FLOAT => cursor = &rem[4..],
//                     LONG => cursor = &rem[8..],
//                     DOUBLE => cursor = &rem[8..],
//                     INT_ARRAY => {
//                         let len = u32::from_be_bytes(rem[..4].try_into().unwrap());
//                         cursor = &rem[4 + len as usize * 4..];
//                     },
//                     STRING => {
//                         let len = u16::from_be_bytes(rem[..2].try_into().unwrap());
//                         cursor = &rem[2 + len as usize..];
//                     }
//                     LIST => {
//                         let tag = rem[0];
//                         let elements = u32::from_be_bytes(rem[1..5].try_into().unwrap());
//                         cursor = &rem[5..];
//                         match tag {
//                             STRING => {
//                                 for _ in 0..elements {
//                                     let len = u16::from_be_bytes(cursor[..2].try_into().unwrap());
//                                     cursor = &cursor[2 + len as usize..];
//                                 }
//                             },
//                             INT => {
//                                 cursor = &cursor[elements as usize * 4..];
//                             }
//                             FLOAT => {
//                                 cursor = &cursor[elements as usize * 4..];
//                             }
//                             DOUBLE => {
//                                 cursor = &cursor[elements as usize * 8..];
//                             }
//                             COMPOUND => {
//                                 stack.push(ParseState::CompoundList(elements));
//                             }
//                             END => assert_eq!(elements, 0),
//                             _ => todo!("{tag}"),
//                         };
//                     }
//                     COMPOUND => {
//                         visitor = visitor.map(name).unwrap();
//                         stack.push(ParseState::Compound);
//                         cursor = rem;
//                     }
//                     _ => todo!("{tag}"),
//                 }
//             }
//         }
//     }
// }
use std::*;
use io::prelude::*;
fn main() {
    let mut buf = vec![0; 18 * 1024 * 1024];
    let savepath = path::PathBuf::from(env::args_os().nth(1).unwrap());
    let mut leveldat = fs::File::open(savepath.join("level.dat")).unwrap();
    // let mut leveldat = fs::File::open(savepath.join("region/r.0.-1.mca")).unwrap();
    // let mut leveldat = fs::File::open(savepath.join("region/r.-1.0.mca")).unwrap();
    let n = leveldat.read(&mut buf).unwrap();
    let mut level = vec![];
    flate2::read::GzDecoder::new(&buf[..n]).read_to_end(&mut level).unwrap();
    // println!("{:?}", String::from_utf8_lossy(&level));
    // let level = b"\n\0\0\n\0\x04Data\x04\0\nRandomSeed\xAA\xAA\xAA\xAA\xAA\xAA\xAA\xAA\0\0\0".as_slice();
    let nbt = enter_compound(b"", &level).unwrap();
    let mut nbt = enter_compound(b"Data", nbt).unwrap();
    while let Some((tag, name, rem)) = entry(nbt).unwrap() {
        println!("{:?}", String::from_utf8_lossy(name));
        match name {
            b"allowCommands" => {
                dbg!(rem[0] == 1);
                break;
            }
            _ => nbt = skip_tag(dbg!(tag), nbt).unwrap(),
        }
    }
    // loop {
    //     let tag = nbt[0];
    //     let name_len = u16::from_be_bytes(nbt[1..=2].try_into().unwrap());
    //     let name = &nbt[2..][..name_len as usize];
    //     nbt = &nbt[2 + name_len as usize..];
    //     const ALLOW_COMMANDS: &[u8] = b"allowCommands";
    //     match name {
    //         ALLOW_COMMANDS => {
    //             assert_eq!(tag, 1);
    //             let allow_commands = nbt[0] == 1;
    //             dbg!(allow_commands);
    //             break;
    //         }
    //         _ => nbt = skip_tag(tag, nbt).unwrap(),
    //     }
    // }
    // let i = skip_tag(10, &level).unwrap();
    // let i = level.len() - 11 - skip_tag(8, &level[11..]).unwrap().len();
    // let i = String::from_utf8_lossy(&level[11..][..i]);
    // println!("{i:?} {}", level.len());
}
fn entry(nbt: &[u8]) -> Option<Option<(u8, &[u8], &[u8])>> {
    if nbt.is_empty() { return Some(None); }
    let name_len = u16::from_be_bytes(nbt.get(1..=2)?.try_into().unwrap()) as usize;
    dbg!(name_len);
    let name = nbt.get(3..3 + name_len)?;
    Some(Some((nbt[0], name, &nbt[3 + name_len..])))
}
fn enter_compound<'a>(name: &[u8], nbt: &'a [u8]) -> Option<&'a [u8]> {
    let end = 3 + name.len();
    nbt.get(end..)
        .filter(|_| &nbt[3..end] == name && nbt[0] == 10 && &nbt[1..=2] == (name.len() as u16).to_be_bytes())
}
use std::convert::TryInto;

#[derive(Debug)]
pub enum Error {
    NeedMore,
    UnknownTag,
    TooDeep,
    UnexpectedEnd,
    OverlongList,
}
pub fn skip_tag(tag: u8, mut nbt: &[u8]) -> Result<&[u8], Error> {
    let mut stack = [(0, 0); 8];
    let mut j = 0;
    let mut frame = &mut stack[j];
    *frame = (tag, 1);

    loop {
        frame.1 -= 1;
        let i = match frame.0 {
            1 => 1,
            2 => 2,
            3 => 4,
            4 => 8,
            5 => 4,
            6 => 8,
            7 => 4 + u32::from_be_bytes(nbt.get(..4).ok_or(Error::NeedMore)?.try_into().unwrap()) as usize,
            8 => 2 + u16::from_be_bytes(nbt.get(..2).ok_or(Error::NeedMore)?.try_into().unwrap()) as usize,
            9 => {
                let len = u32::from_be_bytes(nbt.get(1..=4).ok_or(Error::NeedMore)?.try_into().unwrap());
                let len: u8 = len.try_into().map_err(|_| Error::OverlongList)?;
                let tag = *nbt.get(0).ok_or(Error::NeedMore)?;
                j += 1;
                frame = stack.get_mut(j).ok_or(Error::TooDeep)?;
                *frame = (tag, len);

                5
            }
            10 => {
                match nbt.get(0) {
                    None => 0,
                    Some(0) => 1,
                    Some(tag) => {
                        frame.1 += 1;
                        let name_len = u16::from_be_bytes(nbt.get(1..=2).ok_or(Error::NeedMore)?.try_into().unwrap());
                        j += 1;
                        frame = stack.get_mut(j).ok_or(Error::TooDeep)?;
                        *frame = (*tag, 1);
        
                        3 + name_len as usize

                    }
                }
            },
            11 => 4 + 4 * u32::from_be_bytes(nbt.get(..4).ok_or(Error::NeedMore)?.try_into().unwrap()) as usize,
            12 => 4 + 8 * u32::from_be_bytes(nbt.get(..4).ok_or(Error::NeedMore)?.try_into().unwrap()) as usize,
            _ => return Err(Error::UnknownTag),
        };
        nbt = nbt.get(dbg!(i)..).ok_or(Error::NeedMore)?;
        while dbg!(frame.1) == 0 {
            if j > 0 {
                j -= 1;
                frame = stack.get_mut(j).ok_or(Error::TooDeep)?;
            } else {
                dbg!("ret");
                return Ok(nbt);
            }
        }
    }
}