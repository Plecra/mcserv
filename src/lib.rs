mod collections;
pub mod types;
pub mod world;
pub mod network;

pub use world::World;
pub use network::Network;

mod prelude {
    pub(crate) use crate::collections::*;
    pub(crate) use crate::types::*;
    pub(crate) use crate::network::{Response, Inboxes, client::Inbox};
    pub(crate) use std::{io, time};
    pub(crate) use std::net::TcpStream;
}

/// What would it take to enable resource limits?
/// 
/// To keep a maximum memory usage, I need everything to gracefully handle allocation failure.
/// At which point, it should start trying to free up space to keep running.
/// 
/// This'll behave much like a GC pause, and mustnt happen during normal gameplay.
/// Keeping to these limits should instead be progressive.
/// 
/// What needs to allocate?
/// When a player joins, they will need
///   - A connected socket
///   - A torn send buffer (use should be rare, but one may be needed for all clients up to 2MB)
///   - game state
///     - inventory
///     - health
///     - position/facing/held item/running/crouching
///   - encryptor
///   - compressor
/// When a chunk loads, it needs to store
///   - World representation
///   - sub-tick changes (if I choose to sync these at falling edge)
///   - spatial indexing info
///   - dynamic list of entities
/// An entity is relatively simple
///   - position
///   - ai state
///   - potentially small inventory
/// The level will need memory for all inflight chunk loads
///   Each thread could be loading a chunk, containing metadata, worlddata and entitydata.
/// 
/// chunkloading can allocate buffers ahead of time - it'll allow a fixed number of chunks loading at once,
///   but it's bottlenecked by file io, really.
///   DOWNSCALING: it'd be possible to teardown chunkloaders to buy back some memory.
///                this is a small win and should be used last.
/// 
/// player joins can't really be predicted. With a lowish player limit, they *could* be preallocated,
/// but I think I want them unbounded.
/// In which case, what do I do when a player joins and I'm at the memory limit?
/// Seems like default behaviour should be setting the player cap to memory cap / max player memory
/// which would be max-view-distance * max-chunk-size + player-info + transmit buffer
/// 
/// Problem: there is no "max-chunk-size". Players can create items with infinitely nested NBT
/// 
/// Unless I say no, ig? but any limit on a single item still makes max chunk size **huge**
/// 
/// No, this is intractable. Players are able to join as long as there's a couple mbs spare.
/// I can handle consequences further down the chain.
/// 
/// The server MUST always keep room to allow connections to be rejected & the ability to implement
/// 2b2t's wait queue. This means there'll always at least be a message sent when I'm OOM.
/// - If an admin wants to, it should always be possible to lower max-view-distance in the console,
///   likely raising the player cap.
/// - In fact, this is another option for keeping player spaces open. When there's not enough memory
///   for a player to join, it could aggressively whittle down memory usage elsewhere.
///   - The server could keep upper- and lower-bounds on each of these figures.
///     It'll shutdown if maintaining them isn't possible
///   - example: view-distance: 7.. max-players: 8.. prefer: players
///     - if the server is ever OOM with <=8 players at view-distance 7, it'll shutdown.
///       - though ig it should assume a bad actor, since these were lower bounds set by an admin
///       - how reliably can I judge the memory use of a player?
///       - 
///     - this also implies I need to reclaim memory for saving worldstate to disk.
/// 
/// 
/// 
/// 
struct allocations;