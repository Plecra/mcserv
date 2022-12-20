use crate::prelude::*;
use std::io::{Read, Write};
use std::collections::VecDeque;

#[derive(Debug, Copy, Clone)]
pub(super) enum State {
    Handshaking,
    Status,
    Login,
    Play,
}
#[derive(Debug)]
pub struct Inbox<'a>(&'a mut Client);
impl Inbox<'_> {
    pub fn submit(&mut self, response: Response) {
        self.0.queue.push_back(response);
        self.0.waiting_for_write = true;
    }
    pub fn reborrow(&mut self) -> Inbox<'_> {
        Inbox(&mut self.0)
    }
}

pub struct Client {
    conn: TcpStream,
    state: State,
    queue: VecDeque<Response>,
    pending_bytes: Vec<u8>,
    pending_byte_cursor: usize,
    last_pending_byte: usize,

    pub(super) waiting_for_write: bool,
}
impl Client {
    pub(super) fn is_playing(&self) -> bool {
        matches!(self.state, State::Play)
    }
    pub(super) fn conn(&self) -> &TcpStream {
        &self.conn
    }
    pub(super) fn inbox(&mut self) -> Inbox<'_> {
        Inbox(self)
    }
    pub fn accept(conn: TcpStream) -> io::Result<Self> {
        conn.set_nonblocking(true)?;
        Ok(Self {
            conn,
            queue: Default::default(),
            state: State::Handshaking,
            pending_bytes: vec![],
            pending_byte_cursor: 0,
            last_pending_byte: 0,
            waiting_for_write: false,
        })
    }
    pub fn write(&mut self, world: &crate::World, pid: usize, buf: &mut Vec<u8>) {
        // flush buffer of any half-sent packets
        while self.pending_byte_cursor < self.last_pending_byte {
            match self.conn.write(&self.pending_bytes[self.pending_byte_cursor..self.last_pending_byte]) {
                Ok(0) => todo!("whaaa? they just hung on on mee!!!! >:("),
                Ok(e) => self.pending_byte_cursor += e,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.waiting_for_write = true;
                    return;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => unimplemented!("idk what do do with error {e:?}"),
            }
        }
        let mut low_priority_count = 0;
        while let Some(response) = self.queue.pop_front() {
            if matches!(response, Response::LoadChunk(..)) && self.queue.len() > low_priority_count {
                self.queue.push_back(response);
                low_priority_count += 1;
                continue;
            }
            let mut sending = response.write(world, pid, buf);
            log::trace!("Forwarding {response:?} {}kb", sending.len() as f64 / 1024.0);
            while !sending.is_empty() {
                match self.conn.write(sending) {
                    Ok(0) => todo!("whaaa? they just hung on on mee!!!! >:("),
                    Ok(e) => sending = &sending[e..],
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        let bytes_remaining = sending.len();
                        self.pending_byte_cursor = sending.as_ptr() as usize - buf.as_ptr() as usize;
                        self.last_pending_byte = self.pending_byte_cursor + bytes_remaining;
                        self.pending_bytes = core::mem::replace(buf, vec![0; 2 * 1024 * 1024]);
                        self.waiting_for_write = true;
                        return;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                    Err(e) => unimplemented!("idk what do do with error {e:?}"),
                }
            }
        }
    }
    fn continue_login(&mut self, pkt: &[u8]) -> Result<Option<Name>, Disconnection> {
        let (id, pkt) = super::wire::varint(pkt).ok_or(Disconnection::new())?;
        match id {
            0 => {
                let mut name = [0xFF; 16];
                let given_name = super::wire::str(pkt)
                    .filter(|s| core::str::from_utf8(s.0).is_ok())
                    .ok_or(Disconnection::new())?.0;
                name.get_mut(..given_name.len()).ok_or(Disconnection::new())?.copy_from_slice(given_name);
                Ok(Some(Name::from_utf8(name)))
            }
            _ => Err(Disconnection::new())
        }

    }
    pub fn read(network: &mut super::Network, world: &mut crate::World, id: usize) -> Result<(), ()> {
        let mut write = 0;
        let mut read = 0;
        let mut scratch = core::mem::take(&mut network.scratch_buffer);
        let res = 'ret: loop {
            let client = network.clients.get(id).unwrap();
            match client.conn.read(&mut scratch[write..]) {
                Ok(0) => break Err(()),
                Ok(n) => write += n,
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => break Err(()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    assert_eq!(read, write); // FIXME: this aint guaranteed
                    break Ok(());
                }
                Err(e) => unimplemented!("unexpected error while receiving from player: {e:?}"),
            }
            let mut buf = &scratch[read..write];
            while let Some((pkt, rem)) = super::wire::str(buf) {
                let client = network.clients.get(id).unwrap();
                buf = rem;
                let result = match client.state {
                    State::Handshaking => super::new_connections::recv_handshaking(pkt),
                    State::Status => super::new_connections::recv_status(pkt, Inbox(client)).map(|_| State::Status),
                    State::Login => client.continue_login(pkt).map(|start_playing| {
                        if let Some(name) = start_playing {
                            world.login(id, name, Inbox(client));
                            State::Play
                        } else {
                            State::Login
                        }
                    }),
                    State::Play => world.play_request(id, super::Inboxes(network), pkt).map(|()| State::Play)
                };
                match result {
                    Ok(state) => network.clients.get(id).unwrap().state = state,
                    Err(Disconnection(())) => break 'ret Err(()),
                }
            }
            if buf.is_empty() {
                read = 0;
                write = 0;
            } else {
                read = write - buf.len();
            }
        };
        network.scratch_buffer = scratch;
        res
    }
}
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fields = f.debug_struct("Client");
        if let Ok(addr) = self.conn.peer_addr() {
            fields.field("conn", &addr);
        }
        fields.field("state", &self.state);
        fields.finish()
    }
}