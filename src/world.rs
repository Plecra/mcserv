use crate::prelude::*;

// What's the role of world state?
// Sometimes, there's a fixed template world that is readonly
//   If these are too large for memory (min 4 * 25*25 chunks I guess),
//   we want a way to stream the chunk data in.
//
//   Ah, that'd also mean that ephemeral edits need somewhere to live
//   if they're too big for memory
//
// What can the server do about IO failure?
//   It must keep trying to sync "true" worldstate to disk,
//   if that's not possible, we've got a fatal error in the system
//   and it'll need to shutdown.
//   Can probably have a "recovery mode" that will boot players to a lobby
//   and allow an admin to attempt to recover the servers' data.
//   stuff like mounting a new drive and pointing it at that.
//   (or, more easily, freeing up space by deleting files)
//   Of course, in this scenario, hotswapping the impl wont work.
// That is to say, we have to handle errors at the server layer. They need to be fed back to players,
// otherwise more and more stale edits would pile up, and we'd exhaust memory.
// Also, don't want to persist untouched chunks. They will be regenerated.
// 
pub struct Level {

}
impl Level {
    pub fn empty() -> Self {
        Self {}
    }
    pub fn from_path(p: impl AsRef<std::path::Path>) -> io::Result<Self> {
        Ok(Self {})
    }
}

pub struct Chunk {
    next_free_chunk: u32,
    visible_to: u32,

    pub content: ChunkContent,
}
pub enum ChunkContent {
    OneToOne {
        nonaircounts: [u16; 24],
        blocks: Box<[u64; (16 * 16 * 384) / 4]>,
    }
}
#[derive(Debug, Clone)]
struct Player {
    name: Name,
    position: (f64, f64, f64),
    view_distance: u8,
    hotbar: [Option<Item>; 10],
    selected_item: u8,

    // tick on which we acked
    // this means server lag causes timeouts. not sure about that...
    last_ping_ack: u32, 
}
pub struct World {
    first_free_chunk: u32,
    chunks: Vec<Chunk>,
    index: std::collections::HashMap<(i32, i32), u32>,

    players: Vec<Player>,
    tick: u32,
}
impl World {
    pub fn new() -> Self {
        Self {
            first_free_chunk: u32::MAX,
            chunks: vec![],
            index: Default::default(),
            players: vec![], 
            tick: 0,
        }
    }
    pub fn player_pos(&self, pid: usize) -> (f64, f64, f64) {
        self.players[pid].position
    }
    pub fn chunk_at(&self, x: i32, z: i32) -> &Chunk {
        let chunk = &self.chunks[self.index[&(x, z)] as usize];
        assert_ne!(chunk.visible_to, 0);
        chunk
    }
}
impl World {
    pub fn tick_until(&mut self, tickn: u32, mut inboxes: Inboxes) {
        while self.tick < tickn {
            self.tick(inboxes.reborrow());
            self.tick += 1;
        }
    }
    pub fn next_tick(&self) -> u32 {
        self.tick + 1
    }
    pub(super) fn login(&mut self, pid: usize, name: Name, mut inbox: Inbox) {
        let view_distance = 1;
        let new_player = Player {
            name,
            position: (0.0, 0.0, 0.0),
            view_distance: view_distance as u8,
            last_ping_ack: self.tick,
            hotbar: [None; 10],
            selected_item: 0,
        };
        inbox.submit(Response::Login(name));
        inbox.submit(Response::Play(GameMode::Creative));
        for x in -view_distance..=view_distance {
            for z in -view_distance..=view_distance {
                self.load_chunk(pid, inbox.reborrow(), (x, z));
            }
        }
        // inbox.submit(Response::MoveFast());
        inbox.submit(Response::Position());
        inbox.submit(Response::Chat(format!("server says hi {}", name.as_str())));
        // inbox.submit(Response::SetInventorySlot(0, ItemId(23), 30, 0));
        if self.players.len() <= pid {
            self.players.resize(pid + 1, new_player);
        } else {
            self.players[pid] = new_player;
        }
    }
    pub(crate) fn acknowledge_ping(&mut self, pid: usize) {
        self.players[pid].last_ping_ack = self.tick;
    }
    pub(crate) fn request_move(&mut self, pid: usize, mut inboxes: Inboxes, x: f64, y: f64, z: f64) {
        let player = &mut self.players[pid];
        let mut inbox = inboxes.get(pid).unwrap();
        let old_position = core::mem::replace(&mut player.position, (x, y, z));
        let chunkx = (x / 16.0).floor() as i32;
        let chunkz = (z / 16.0).floor() as i32;
        let oldchunkx = (old_position.0 / 16.0).floor() as i32;
        let oldchunkz = (old_position.2 / 16.0).floor() as i32;
        if oldchunkx != chunkx || oldchunkz != chunkz {
            let view_distance = player.view_distance as i32;
            let xdisp = chunkx - oldchunkx;
            let zdisp = chunkz - oldchunkz;
            let max_view_size = 2 * view_distance + 1;
            inbox.submit(Response::CenterChunk(chunkx, chunkz));
            for x in 0..(max_view_size - xdisp.abs()) {
                let col = chunkx - view_distance + x;
                for i in 0..zdisp.abs() {
                    let newrow = chunkz + (view_distance - i) * zdisp.signum();
                    let oldrow = oldchunkz - (view_distance - i) * zdisp.signum();
                    self.load_chunk(pid, inbox.reborrow(), (col, newrow));
                    // inbox.submit(Response::LoadChunk(col, newrow));
                    // inbox.submit(Response::UnloadChunk(col, oldrow));
                }
            }
            for x in 0..xdisp.abs().min(max_view_size) {
                for z in -view_distance..=view_distance {
                    let newcol = chunkx + (view_distance - x) * xdisp.signum();
                    let newrow = chunkz - z;
                    let oldcol = oldchunkx - (view_distance - x) * xdisp.signum();
                    let oldrow = oldchunkz - z;
                    self.load_chunk(pid, inbox.reborrow(), (newcol, newrow));
                    // inbox.submit(Response::LoadChunk(newcol, newrow));
                    // inbox.submit(Response::UnloadChunk(oldcol, oldrow));
                }
            }
        }
        let blockx = x.floor() as i32;
        let blockz = z.floor() as i32;
        let oldblockx = old_position.0.floor() as i32;
        let oldblockz = old_position.2.floor() as i32;
struct Line {
    current: (i32, i32),
    end: (i32, i32),
    diff: (i32, i32),
    inc: (i32, i32),

    error: i32,
}
impl Iterator for Line {
    type Item = (i32, i32);
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            return None;
        }
        let error = self.error;
        if error > -self.diff.0 {
            self.error -= self.diff.1;
            self.current.0 += self.inc.0;
        }
        if error < self.diff.1 {
            self.error += self.diff.0;
            self.current.1 += self.inc.1;
        }
        Some(self.current)
    }
}
impl Line {
    fn new(start: (i32, i32), end: (i32, i32)) -> Self {
        let diff = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
        Self {
            current: start,
            end,
            diff,
            inc: ([-1, 1][(start.0 < end.0) as usize], [-1, 1][(start.1 < end.1) as usize]),

            error: if diff.0 > diff.1 { diff.0 } else { -diff.1 } / 2
        }
    }
}
        for (x, z) in Line::new((oldblockx, oldblockz), (blockx, blockz)) {
            if (0.0..10.0).contains(&y) {
                self.set_block(V3(x, -1, z), Some(Block::ANDESITE), inboxes.reborrow());
                // inbox.submit(Response::SetBlock(V3(x, -1, z), BlockId(4)));
            }
        }
    }
    pub(crate) fn break_at(&mut self, inboxes: Inboxes, pos: V3<i32>) {
        self.set_block(pos, None, inboxes);
    }
    pub(crate) fn run_command(&self, pid: usize, mut inboxes: Inboxes, cmd: &[u8]) {
        if cmd == b"kill" {
            inboxes.get(pid).unwrap().submit(Response::SetHealth(0.0, 0, 0.0));
        }
    }
    pub(crate) fn chat_message(&self, mut inboxes: Inboxes, msg: &str) {
        inboxes.retain(|_, mut inbox| {
            inbox.submit(Response::Chat(msg.to_owned()));
            true
        });
    }
    pub(crate) fn use_item_at_block(&mut self, pid: usize, inboxes: Inboxes, pos: V3<i32>, hand: Hand, face: BlockFace) {
        let player = &self.players[pid];
        let inventory_slot = match hand {
            Hand::Main => player.selected_item,
            Hand::Secondary => 9,
        };
        let item = player.hotbar[inventory_slot as usize];
        if let Some(block) = item.and_then(|i| i.block()) {
            self.try_set_block(match face {
                BlockFace::Top => V3(pos.x, pos.y + 1, pos.z),
                BlockFace::Bottom => V3(pos.x, pos.y - 1, pos.z),
                BlockFace::North => V3(pos.x, pos.y, pos.z - 1),
                BlockFace::South => V3(pos.x, pos.y, pos.z + 1),
                BlockFace::West => V3(pos.x - 1, pos.y, pos.z),
                BlockFace::East => V3(pos.x + 1, pos.y, pos.z),
            }, block, inboxes);
        }
    }
    fn try_set_block(&mut self, pos: V3<i32>, block: Block, inboxes: Inboxes) {
        println!("placing at {pos:?}");
        if !self.does_entity_collide(pos) {
            self.set_block(pos, Some(block), inboxes);
        }
    }
fn does_entity_collide(&self, pos: V3<i32>) -> bool {
    for player in &self.players {
        if player.position.0 - 0.3 < (pos.x + 1) as f64 && player.position.0 + 0.3 > pos.x as f64
        && player.position.1 < (pos.y + 1) as f64       && player.position.1 + 1.8 > pos.y as f64
        && player.position.2 - 0.3 < (pos.z + 1) as f64 && player.position.2 + 0.3 > pos.z as f64
        {
            return true;
        }
    }
    false
}
    pub(crate) fn set_held_item(&mut self, pid: usize, inboxes: Inboxes, hotbar_idx: u8) {
        self.players[pid].selected_item = hotbar_idx;
    }
    pub(crate) fn set_block(&mut self, pos: V3<i32>, block: Option<Block>, mut inboxes: Inboxes) {
        let x = (pos.x as f32 / 16.0).floor() as i32;
        let z = (pos.z as f32 / 16.0).floor() as i32;
        let chunk = &mut self.chunks[self.index[&(x, z)] as usize];
        // let id = BlockId(8);
        match &mut chunk.content {
            ChunkContent::OneToOne {
                nonaircounts,
                blocks
            } => {
                let id = block.map_or(0, |b| b.net_id());
                let idx = (pos.y + 64) * 16 * 16 + pos.z.rem_euclid(16) * 16 + pos.x.rem_euclid(16);
                let long = &mut blocks[(idx / 4) as usize];
                let subidx = idx % 4;
                let mask = 0b11111_11111_11111;
                let old = (*long >> (subidx * 15)) & mask;
                *long = (*long & !(mask << (subidx * 15))) | (id as u64) << (subidx * 15);
                let y = (pos.y as f32 / 16.0).floor() as i32 + 4;
                if old == 0 && id != 0 {
                    nonaircounts[y as usize] += 1;
                } else if old != 0 && id == 0 {
                    nonaircounts[y as usize] -= 1;
                }
            }
        }
        inboxes.retain(|i, mut inbox| {
            if chunk.visible_to & 1 << i != 0 {
                inbox.submit(Response::SetBlock(pos, block));
            }
            true
        });
    }
    fn logout(&self, pid: usize) {

    }
    fn tick(&mut self, inboxes: Inboxes) {
        if self.tick % (5 * 20) == 0 {
            inboxes.retain(|pid, mut inbox| {
                if self.tick - self.players[pid].last_ping_ack > (20 * 5) {
                    log::warn!("{} timed out", self.players[pid].name.as_str());
                    self.logout(pid);
                    false
                } else {
                    inbox.submit(Response::Ping());
                    true
                }
            })
        }
    }
    pub(crate) fn set_creative_slot(&mut self, pid: usize, mut inboxes: Inboxes, slot: i16, item: Option<(Item, u8)>) {
        if (36..=45).contains(&slot) {
            self.players[pid].hotbar[(slot - 36) as usize] = item.map(|it| it.0);
        }
    }
    pub(crate) fn closed_inventory(&mut self, pid: usize, window: u8) {

    }
    pub(crate) fn load_chunk(&mut self, pid: usize, mut inbox: Inbox, pos: (i32, i32)) {
        if let Some(chunk) = self.index.get(&pos).and_then(|idx| self.chunks.get_mut(*idx as usize)) {
            // println!("revisiting chunk {},{}", pos.0, pos.1);
            // match &chunk.content {
            //     ChunkContent::OneToOne { nonaircounts, blocks } => {
            //         for z in 0..16 {
            //             for x in 0..16 {
            //                 let idx = 63 * 16 * 16 +  z * 16 + x;
            //                 print!("{:02X} ", (blocks[idx / 4] >> (idx % 4) * 15) & 0b11111_11111_11111);
            //             }
            //             println!("");
            //         }
            //         println!("");
            //     }
            // }
            chunk.visible_to |= 1 << pid;
        } else {
            let mut blocks = vec![0u64; (16 * 16 * 384) / 4];
            blocks[..16 * 16 * 16 * 4  / 4].iter_mut().for_each(|s| *s = 0b00000_00000_00001__00000_00000_00001__00000_00000_00001__00000_00000_00001);
            blocks[16 * 16 * 16 - 1] = 0b00000_00000_00011__00000_00000_00011__00000_00000_00011__00000_00000_00011;
            let mut nonaircounts = [0; 24];
            nonaircounts[..4].iter_mut().for_each(|v| *v = 4096);
            let content = ChunkContent::OneToOne {
                nonaircounts,
                blocks: blocks.into_boxed_slice().try_into().unwrap()
            };
            let mut idx = self.first_free_chunk;
            loop {
                if idx == u32::MAX {
                    idx = self.chunks.len() as u32;
                    self.chunks.push(Chunk {
                        next_free_chunk: u32::MAX,
                        visible_to: 1 << pid,
    
                        content,
                    });
                    break;
                }
                let chunk = self.chunks.get_mut(idx as usize).unwrap();
                if chunk.visible_to == 0 {
                    chunk.content = content;
                    chunk.visible_to |= 1 << pid;
                    break;
                }
                idx = chunk.next_free_chunk;
            }
            self.index.insert(pos, idx);
        }
        inbox.submit(Response::LoadChunk(pos.0, pos.1));
    }
    pub(crate) fn request_view_distance(&mut self, pid: usize, mut inbox: Inbox, view_distance: u8) {
        // FIXME: I'm fairly sure the chunkloading logic is overworking the server. (loads seem to be doubled up on connect)
        // The client is behaving right now though, so I'm not fiddling anymore
        inbox.submit(Response::SetRenderDistance(view_distance - 1));

        let player = &mut self.players[pid];
        let old_view_distance = core::mem::replace(&mut player.view_distance, view_distance);
        let chunkx = (player.position.0 / 16.0).floor() as i32;
        let chunkz = (player.position.2 / 16.0).floor() as i32;
        if view_distance < old_view_distance {
            todo!()
        } else {
            for layer in old_view_distance..=view_distance {
                let layer = layer as i32;
                for x in 0..=layer {
                    self.load_chunk(pid, inbox.reborrow(), (chunkx + x, chunkz + layer));
                    self.load_chunk(pid, inbox.reborrow(), (chunkx - x, chunkz + layer));
                    self.load_chunk(pid, inbox.reborrow(), (chunkx + x, chunkz - layer));
                    self.load_chunk(pid, inbox.reborrow(), (chunkx - x, chunkz - layer));
                }
                for z in 0..layer {
                    self.load_chunk(pid, inbox.reborrow(), (chunkx + layer, chunkz + z));
                    self.load_chunk(pid, inbox.reborrow(), (chunkx - layer, chunkz + z));
                    self.load_chunk(pid, inbox.reborrow(), (chunkx + layer, chunkz - z));
                    self.load_chunk(pid, inbox.reborrow(), (chunkx - layer, chunkz - z));

                }
            }
        }
    }
}