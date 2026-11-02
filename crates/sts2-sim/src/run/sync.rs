//! `CombatStateSynchronizer`, which a run of several players runs as it
//! enters a room.
//!
//! `StartSync` has each machine send its own player as `ToSerializable`
//! makes it, and the host send the run's streams and the shared relic bag.
//! `WaitForSync` then has every machine take each other player as their
//! own machine sent them (`SyncWithSerializedPlayer`), and each client take
//! the host's streams and bag. So what ran on one machine alone, as a
//! purchase does, reaches the others' copies there.

use std::sync::Arc;

use sts2_core::{Rules, SeatIndex};

use super::State;

impl<V: Rules> State<V> {
    /// Hand every machine as it stood before the decision, which a room the
    /// decision enters syncs from. Nothing clears what the decision did not
    /// use.
    pub(crate) fn sync_from(&mut self, machines: Option<Arc<[State<V>]>>) {
        self.sync = machines;
    }

    /// `WaitForSync`, as the run enters a room, before it is built.
    pub(super) fn room_synced(&mut self) {
        let Some(machines) = self.sync.take() else {
            return;
        };
        for at in 0..machines.len() {
            let seat = SeatIndex(u8::try_from(at).expect("a lobby's seats fit a u8"));
            if seat == self.local {
                continue;
            }
            let theirs = &machines[at];
            self.players[seat].synced_with(&theirs.players[seat]);
            self.belts[seat] = theirs.belts[seat].clone();
            self.streams.load_player(at, &theirs.streams);
        }
        let host = SeatIndex(0);
        if self.local != host {
            let hosts = &machines[host.at()];
            self.streams.load_run(&hosts.streams);
            self.shared_dealt = Arc::clone(&hosts.shared_dealt);
            self.shared_refills = Arc::clone(&hosts.shared_refills);
        }
    }
}
