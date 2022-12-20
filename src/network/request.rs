use super::*;
use wire::*;
fn decode<'a, T: wire::Wire<'a>>(pkt: &'a [u8]) -> Result<T, Disconnection> {
    Ok(T::decode(pkt)?.0)
}

impl crate::World {
    pub(super) fn play_request(&mut self, pid: usize, mut inboxes: Inboxes, pkt: &[u8]) -> Result<(), Disconnection> {
        let (id, pkt) = wire::varint(pkt).ok_or(Disconnection::new())?;
        match id {
            0x00 => {} // confirm Position(()) packet. Maybe trusted clients wont move in the world until we get this?
            0x03 => self.run_command(pid, inboxes, decode(pkt)?),
            0x04 => self.chat_message(inboxes, decode(pkt)?),
            0x06 => {
                match decode(pkt)? {
                    0u8 => { // do respawn plz
                        inboxes.reborrow().get(pid).unwrap().submit(Response::Respawn());
                        self.request_move(pid, inboxes.reborrow(), 0.0, 0.0, 0.0);
                        inboxes.reborrow().get(pid).unwrap().submit(Response::Position());
                    }
                    _ => todo!()
                }
            }
            0x07 => {
                let (locale, max_view_distance): (&[u8], _) = decode(pkt)?;
                self.request_view_distance(pid, inboxes.get(pid).unwrap(), max_view_distance);
            }
            0x0a => {
                let (window, _stateid, slot, button, mode): (u8, wire::var<i32>, i32, u8, wire::var<i32>) = decode(pkt)?;
                // players' slot in window was interacted with
            }
            0x0b => self.closed_inventory(pid, decode(pkt)?), // close container
            0x0c => log::trace!("message on plugin channel {:?}", String::from_utf8_lossy(decode(pkt)?)),
            0x13 => {
                let (x, feet_y, z, on_ground): (_, _, _, bool) = decode(pkt)?;
                self.request_move(pid, inboxes, x, feet_y, z);
            }
            0x14 => {
                let (x, feet_y, z, yaw, pitch, on_ground): (_, _, _, f32, f32, bool) = decode(pkt)?;
                self.request_move(pid, inboxes, x, feet_y, z);
            }
            0x15 => {
                let (yaw, pitch, on_ground): (f32, f32, bool) = decode(pkt)?;
            }
            0x16 => log::info!("is on ground? {:?}", decode::<bool>(pkt)?),
            0x1b => {
                let flags: u8 = decode(pkt)?;
                let _is_flying = flags & 2 != 0;
            }
            0x1c => {
                let (var(status), Position(pos), face, var(seq)) = decode(pkt)?;
                let _: BlockFace = face;
                match status {
                    0 => { // started digging. instabreak in creative?
                        self.break_at(inboxes.reborrow(), pos);
                        inboxes.get(pid).unwrap().submit(Response::AckBlockChange(seq));
                    }
                    5 => todo!(), // todo
                    6 => todo!("swap item in hand"),
                    _ => todo!()
                }
            } // interacted with block
            0x1d => {} // player command (crouching, running)
            0x1f => self.acknowledge_ping(pid),
            // 0x20 => {}
            0x27 => self.set_held_item(pid, inboxes, decode::<i16>(pkt)? as u8),
            0x2a => {
                let (slot, item) = decode(pkt)?;
                self.set_creative_slot(pid, inboxes, slot, item);
            }
            0x2E => {
                let hand: Hand = decode(pkt)?;
                // player swung `hand`. probably forward to other players
            } 
            0x30 => {
                let (hand, Position(position), face, pos_on_block, inblock, var(seq)) = decode(pkt)?;
                let _: bool = inblock;
                let _: V3<f32> = pos_on_block;
                self.use_item_at_block(pid, inboxes.reborrow(), position, hand, face);
                inboxes.get(pid).unwrap().submit(Response::AckBlockChange(seq));
            }
            // 0x31 => {} // use item
            _ => todo!("unimpled play packet 0x{id:02x}")
        }
        Ok(())
    }
}