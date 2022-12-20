use crate::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct var<T>(pub T);
pub trait Wire<'a>: Sized {
    fn decode(pkt: &'a [u8]) -> Result<(Self, &'a [u8]), Disconnection>;
}
macro_rules! impl_wire {
    {} => {};
    {$t:ident $($rt:ident)*} => {
        impl<'a, $t: Wire<'a>, $($rt: Wire<'a>),*> Wire<'a> for ($t,$($rt,)*) {
            fn decode(pkt: &'a [u8]) -> Result<(Self, &[u8]), Disconnection> {
                let ($t, pkt) = $t::decode(pkt)?;
                $(let ($rt, pkt) = $rt::decode(pkt)?;)*
                Ok((($t, $($rt,)*), pkt))
            }
        }
        impl_wire!($($rt)*);
    }
}
impl_wire!(A B C D E F);
impl<'a, T: Wire<'a>> Wire<'a> for V3<T> {
    fn decode(pkt: &'a [u8]) -> Result<(Self, &[u8]), Disconnection> {
        let ((x, y, z), pkt) = Wire::decode(pkt)?;
        Ok((Self { x, y, z }, pkt))
    }
}
impl Wire<'_> for var<i32> {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        varint(pkt).map(|(n, rem)| (Self(n), rem)).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for u8 {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        pkt.split_first().map(|(b, rem)| (*b, rem)).ok_or(Disconnection::new())
    }
}
impl<'a> Wire<'a> for &'a [u8] {
    fn decode(pkt: &'a [u8]) -> Result<(Self, &[u8]), Disconnection> {
        str(pkt).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for i32 {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        i32(pkt).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for f32 {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        f32(pkt).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for f64 {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        f64(pkt).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for i16 {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        i16(pkt).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for u16 {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        u16(pkt).ok_or(Disconnection::new())
    }
}
impl<'a, T: Wire<'a>> Wire<'a> for Option<T> {
    fn decode(pkt: &'a [u8]) -> Result<(Self, &[u8]), Disconnection> {
        let (present, pkt) = bool(pkt).ok_or(Disconnection::new())?;
        if present {
            let (value, pkt) = T::decode(pkt)?;
            Ok((Some(value), pkt))
        } else {
            Ok((None, pkt))
        }
    }
}
impl Wire<'_> for crate::types::Item {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        Wire::decode(pkt).and_then(|(id, rem)| Ok((Self::new(id).ok_or(Disconnection::new())?, rem)))
    }
}
pub fn byte(buf: &[u8]) -> Option<(u8, &[u8])> {
    buf.split_first().map(|(&b, r)| (b, r))
}
pub fn bool(buf: &[u8]) -> Option<(bool, &[u8])> {
    byte(buf).map(|(b, r)| (b != 0, r))
}
macro_rules! be {
    { $($i:ident)* } => {
        $(
            pub fn $i(buf: &[u8]) -> Option<($i, &[u8])> {
                (buf.len() >= core::mem::size_of::<$i>()).then(|| {
                    let (n, rem) = buf.split_at(core::mem::size_of::<$i>());
                    ($i::from_be_bytes(n.try_into().unwrap()), rem)
                })
            }
        )*
    }
}
be! { u16 i16 i32 i64 u64 f32 f64 }

pub fn varint(buf: &[u8]) -> Option<(i32, &[u8])> {
    let mut n = 0u32;
    let mut i = 0;
    loop {
        let b = *buf.get(i)?;
        n |= (b as u32 & 0b111_1111) << 7 * i;
        i += 1;
        if b >> 7 == 0 || i == 5 {
            break;
        }
    }
    Some((i32::from_ne_bytes(n.to_ne_bytes()), &buf[i..]))
}
pub fn str(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let (l, rem) = varint(buf)?;
    if l as usize <= rem.len() {
        Some(rem.split_at(l as usize))
    } else {
        None
    }
}
pub struct Position(pub V3<i32>);
impl Wire<'_> for Position {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        pos(pkt).map(|(pos, rem)| (Self(pos), rem)).ok_or(Disconnection::new())
    }
}
impl Wire<'_> for Hand {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        match u8::decode(pkt)? {
            (0, rem) => Ok((Self::Main, rem)),
            (1, rem) => Ok((Self::Secondary, rem)),
            _ => Err(Disconnection::new())
        }
    }
}
impl Wire<'_> for BlockFace {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        let (id, rem) = u8::decode(pkt)?;
        Ok((match id {
            0 => Self::Bottom,
            1 => Self::Top,
            2 => Self::North,
            3 => Self::South,
            4 => Self::West,
            5 => Self::East,
            _ => return Err(Disconnection::new())
        }, rem))
    }
}
impl<'a> Wire<'a> for &'a str {
    fn decode(pkt: &'a [u8]) -> Result<(Self, &'a [u8]), Disconnection> {
        Wire::decode(pkt).and_then(|(buf, rem)| Ok((std::str::from_utf8(buf).map_err(|e| Disconnection::new())?, rem)))
    }
}
impl Wire<'_> for bool {
    fn decode(pkt: &[u8]) -> Result<(Self, &[u8]), Disconnection> {
        let (id, rem) = u8::decode(pkt)?;
        Ok((match id {
            0 => false,
            1 => true,
            _ => return Err(Disconnection::new())
        }, rem))
    }
}
pub fn pos(buf: &[u8]) -> Option<(V3<i32>, &[u8])> {
    let (position, rem) = u64(buf)?;
    let mut x = (position >> 38) as i32;
    let mut y = (position & 0xFFF) as i32;
    let mut z = ((position >> 12) & 0x3FFFFFF) as i32;
    if x >= 1 << 25 { x -= 1 << 26 }
    if y >= 1 << 11 { y -= 1 << 12 }
    if z >= 1 << 25 { z -= 1 << 26 }
    Some((V3(x, y, z), rem))
}