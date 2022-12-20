# A Minecraft Server Implementation

This is a server designed to support a private multiplayer world running on low-end hardware.

## High-level architecture

To best handle connections with poor bandwidth, the entire server accounts for backpressure, assuming it may not be possible to forward messages to clients and gracefully waiting for the network to be available.

If the server's upload rate is maxed out, which is normally due to running on a personal network, then all players will start lagging and potentially timing out. The game needs to continue serving as many packets as it can as bandwidth varies.

- [ ] Maximise compression of packets.
    - This is especially useful for worlddata, which is sent in large batches (generally the size of chunks)
- [ ] Prioritize packets to maintain gameplay
    - Ping packets must be handled promptly
    - Everyone needs to see player movement the same way
    - For fair combat, players need to be able to interact with the world as they see it
        - anticheat should try not to penalise players for the servers' poor network.
        - ...and also can't trust that the client has poor network.
        - this might require some system for knowing that an enemy was in reach *when the player hit them*

### A potential design for backpressure

Each player may store a set of conflicts with the world state. The acts as a type of delta compression to represent the state of the client derived from the state of the server.

These conflicts can be stored in a priority queue ordered by how urgently they need to be resolved.

Sending updates to the player is then a process of resolving these conflicts.

There are different sets of players which individual conflicts apply to.
- Global
    - Global chat
    - Player list
    - Shutdowns
    - System notifications
- World
    - Time of day
- Chunk
    - Entity Movement
    - Entity Death
    - Entity Spawn
    - Block Change
    - Lighting Change
- Dynamic (teams)
    - Private chat

Ideally, work wont be duplicated when player sets are sent a message.
    
- Keeping a shared message queue makes dynamic prioritisation more awkward: 
    > Each player in range of a chunk will have different priorities for it. Someone inside it will want particle effects promptly, but entity movement is more urgent for players farther away. Maybe these are two different sets?
- Also, an entity entering a chunk will spawn it for only some players.
    > Handling "entity enter" can test if the previous chunk was loaded by the player.
    > 
    > I think this also requires a sequence identifier to check that the player was up to date with the previous state of the entity. If not, it should probably be respawned for them