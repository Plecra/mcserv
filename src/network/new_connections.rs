use super::{Disconnection, wire, client::{State, Inbox}};

pub(super) fn recv_handshaking(pkt: &[u8]) -> Result<State, Disconnection> {
    let (_, pkt) = wire::varint(pkt).filter(|p| p.0 == 0).ok_or(Disconnection::new())?;
    let (protocol_version, pkt) = wire::varint(pkt).ok_or(Disconnection::new())?;
    let _connected_with_address = ();
    let _connected_with_port = ();
    let next_state = pkt[pkt.len() - 1];

    match (protocol_version, next_state) {
        (_, 1) => Ok(State::Status),
        (759, 2) => Ok(State::Login),
        (760, 2) => Ok(State::Login), // FIXME: problem?
        _ => {
            log::debug!("client connected with unknown version {protocol_version}");
            Err(Disconnection::new())
        }
    }
}
pub(super) fn recv_status(pkt: &[u8], mut inbox: Inbox) -> Result<(), Disconnection> {
    let (id, pkt) = wire::varint(pkt).ok_or(Disconnection::new())?;
    match id {
        0 if pkt.is_empty() => inbox.submit(super::Response::Status()),
        1 => inbox.submit(super::Response::Pong(wire::u64(pkt).ok_or(Disconnection::new())?.0)),
        _ => return Err(Disconnection::new()),
    }
    Ok(())
}