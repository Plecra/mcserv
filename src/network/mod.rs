use crate::prelude::*;

mod new_connections;
mod response;
mod request;
mod inboxes;
pub mod wire;
pub mod client;

pub use response::Response;
pub use inboxes::Inboxes;

#[derive(Debug)]
pub struct Network {
    pub(super) listener: std::net::TcpListener,
    pub(super) scratch_buffer: Vec<u8>,
    
    pub(super) poller: polling::Poller,
    pub(super) events: Vec<polling::Event>,
    
    pub(super) clients: SlotMap<client::Client>,
}

const LISTENER: usize = usize::MAX - 1;
impl Network {
    pub fn new() -> io::Result<Self> {
        let poller = polling::Poller::new()?;
    
        let listener = std::net::TcpListener::bind("0.0.0.0:25565")?;
        listener.set_nonblocking(true)?;
        poller.add(&listener, polling::Event::readable(LISTENER))?;
        Ok(Self {
            listener,
            scratch_buffer: vec![0; 2 * 1024 * 1024],

            poller,
            events: vec![],

            clients: SlotMap::new(),
        })
    }


    pub fn process_packets_until(&mut self, deadline: time::Instant, world: &mut crate::World) {
        loop {
            match self.poller.wait(&mut self.events, match deadline.checked_duration_since(time::Instant::now()) {
                Some(v) => Some(v),
                None => return,
            }) {
                Ok(0) => return,
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => unimplemented!("unexpected error while waiting for player input: {e:?}"),
            }
            while let Some(event) = self.events.pop() {
                if event.key == LISTENER {
                    self.poller.modify(&self.listener, polling::Event::readable(LISTENER))
                        .expect("unable to listen for connecting players. network down?");
                    self.accept_players();
                    continue;
                }
                
                let client = self.clients.get(event.key).expect("received message from dead client");
                let was_waiting_for_write = core::mem::take(&mut client.waiting_for_write);
                
                if event.readable {
                    // may set the waiting_for_write flag
                    if client::Client::read(self, world, event.key).is_err() {
                        let client = self.clients.get(event.key).expect("received message from dead client");
                        log::debug!("client disconnected {}", event.key);
                        self.poller.delete(client.conn()).unwrap();
                        self.clients.release(event.key);
                        continue;
                    }
                }
                let client = self.clients.get(event.key).expect("received message from dead client");
                if event.writable {
                    client.write(world, event.key, &mut self.scratch_buffer);
                }
                self.poller.modify(client.conn(), polling::Event {
                    key: event.key,
                    readable: true,
                    writable: client.waiting_for_write || (was_waiting_for_write && !event.writable)
                }).unwrap();
            }
        }
    }
    fn accept_players(&mut self) {
        loop {
            match self.listener.accept() {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => unimplemented!("unexpected error during player connection {e:?}"),
                Ok((conn, _)) => {
                    self.poller.add(&conn, polling::Event::readable(self.clients.next_idx()))
                      .expect("unexpected error connecting to player input");
                    let idx = self.clients.insert(client::Client::accept(conn)
                      .expect("unexpected error connecting to player input"));
                    log::debug!("new player connected as {idx}");
                },
            };
        }
    }
}