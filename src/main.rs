use std::time::*;

struct Announcer {
    next_due: Instant,
    socket: std::net::UdpSocket,
}
impl Announcer {
    fn new() -> std::io::Result<Self> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            next_due: Instant::now(),
            socket
        })
    }
    fn announce(&mut self) {
        let now = Instant::now();
        if self.next_due < now {
            self.next_due = now + Duration::from_secs(2);
            let announcement = concat!("[MOTD]", "Pria SMP", "[/MOTD]", "[AD]", 25565, "[/AD]");
            match self.socket.send_to(announcement.as_bytes(), "224.0.2.60:4445") {
                Ok(n) if n == announcement.len() => {}
                Ok(_) => log::warn!("network too busy to announce on LAN"),
                Err(e) => unimplemented!("{e}"),
            }
        }
    }
}


fn main() -> std::io::Result<()> {
    env_logger::init();
    let mut args = std::env::args_os();
        
    // let mut level = if let Some(path) = args.nth(1) {
    //     mcserv::world::Level::from_path(path)?
    // } else {
    //     mcserv::world::Level::empty()
    // };
    let mut world = mcserv::World::new();
    let mut network = mcserv::Network::new()?;
    
    let starttime = Instant::now();
    let mut announcer = Announcer::new()?;
    loop {
        announcer.announce();

        let next_tick_due = starttime + world.next_tick() * Duration::from_millis(50);
        network.process_packets_until(next_tick_due, &mut world);

        let time_passed_in_ticks = (Instant::now() - starttime).as_millis() / 50;
        world.tick_until(time_passed_in_ticks as u32, network.inboxes());
    }
}
