use super::*;

pub struct Inboxes<'a>(pub(super) &'a mut Network);

impl Network {
    /// Used by the world to send updates to the network
    pub fn inboxes(&mut self) -> Inboxes<'_> {
        Inboxes(self)
    }
}

impl Inboxes<'_> {
    pub fn get(&mut self, idx: usize) -> Option<client::Inbox> {
        self.0.clients.get(idx).map(|c| c.inbox())
    }
    pub fn reborrow(&mut self) -> Inboxes {
        Inboxes(self.0)
    }
    pub fn retain(self, mut keep: impl FnMut(usize, client::Inbox) -> bool) {
        self.0.clients.retain(|idx, client| {
            let was_waiting_for_write = core::mem::take(&mut client.waiting_for_write);
            if !client.is_playing() {
                true
            } else if keep(idx, client.inbox()) {
                if client.waiting_for_write && !was_waiting_for_write {
                    self.0.poller.modify(client.conn(), polling::Event::all(idx)).unwrap();
                }
                client.waiting_for_write |= was_waiting_for_write;
                true
            } else {
                self.0.poller.delete(client.conn()).unwrap();
                false
            }
        });
    }
}