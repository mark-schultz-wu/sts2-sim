//! What a co-op seat's observation says about the other seats, and how the
//! protocol names a player.
//!
//! The mod names a player by its net id. A seat names it by its place in the
//! lobby. The net id each seat's game is given ties the two. The game's ENet
//! host is always 1, and a client picks its own, `1000 + seat`.

use serde::Deserialize;
use sts2_core::{Peer, Peers, SeatIndex};

use super::ident;
use super::observation::{Error, Why};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WirePeers {
    players: Vec<WirePlayer>,
    awaiting: Vec<u64>,
    in_flight: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WirePlayer {
    net_id: u64,
    ended_turn: bool,
    vote: Option<String>,
}

/// The net id seat `seat`'s game plays under.
pub(crate) fn net_id(seat: SeatIndex) -> u64 {
    match seat.0 {
        0 => 1,
        at => 1000 + u64::from(at),
    }
}

/// The seat whose game plays under `net_id`, if any does.
pub(crate) fn seat_of(net_id: u64) -> Option<SeatIndex> {
    match net_id {
        1 => Some(SeatIndex(0)),
        1001..=1255 => u8::try_from(net_id - 1000).ok().map(SeatIndex),
        _ => None,
    }
}

/// A target player on the wire: the seat, spelled as its net id.
pub(crate) mod target {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use sts2_core::SeatIndex;

    pub(crate) fn serialize<S: Serializer>(
        target: &Option<SeatIndex>,
        to: S,
    ) -> Result<S::Ok, S::Error> {
        target.map(super::net_id).serialize(to)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        from: D,
    ) -> Result<Option<SeatIndex>, D::Error> {
        Option::<u64>::deserialize(from)?
            .map(|net_id| {
                super::seat_of(net_id).ok_or_else(|| {
                    serde::de::Error::custom(format!("{net_id} is no seat's net id"))
                })
            })
            .transpose()
    }
}

/// The `peers` of a co-op seat's state, for a lobby of `seats`.
pub(crate) fn peers(state: &serde_json::Value, seats: usize) -> Result<Peers, Error> {
    let fail = |why| Error {
        at: "peers".into(),
        why,
    };
    let raw = state
        .get("peers")
        .ok_or_else(|| fail(Why::Absent("peers")))?;
    let wire = WirePeers::deserialize(raw).map_err(|e| {
        fail(Why::Unknown {
            what: "peers",
            value: e.to_string(),
        })
    })?;
    let place = |net_id: u64| {
        seat_of(net_id)
            .filter(|seat| seat.at() < seats)
            .ok_or_else(|| {
                fail(Why::Unknown {
                    what: "player's net id",
                    value: net_id.to_string(),
                })
            })
    };
    let mut seated = vec![Peer::default(); seats];
    for player in &wire.players {
        let vote = player
            .vote
            .as_deref()
            .map(ident::coord)
            .transpose()
            .map_err(|e| fail(Why::Ident(e)))?;
        seated[place(player.net_id)?.at()] = Peer {
            ended_turn: player.ended_turn,
            vote,
        };
    }
    let awaiting = wire
        .awaiting
        .iter()
        .map(|&net_id| place(net_id))
        .collect::<Result<_, _>>()?;
    Ok(Peers {
        seats: seated,
        awaiting,
        in_flight: wire.in_flight,
    })
}

#[cfg(test)]
mod tests {
    use super::peers;
    use serde_json::json;
    use sts2_core::{Coord, Peer, SeatIndex};

    /// The protocol's own example, with the host at seat 0 and the client at
    /// seat 1.
    #[test]
    fn each_player_lands_at_the_seat_of_its_net_id() {
        let state = json!({"peers": {
            "players": [{"netId": 1001, "endedTurn": false, "vote": "2,3"},
                        {"netId": 1, "endedTurn": true, "vote": null}],
            "awaiting": [1001],
            "inFlight": false,
        }});
        let read = peers(&state, 2).expect("the peers read");
        assert_eq!(
            read.seats,
            [
                Peer {
                    ended_turn: true,
                    vote: None
                },
                Peer {
                    ended_turn: false,
                    vote: Some(Coord::new(2, 3))
                },
            ]
        );
        assert_eq!(read.awaiting, [SeatIndex(1)]);
        assert!(!read.in_flight);
    }

    #[test]
    fn a_net_id_no_seat_has_is_refused() {
        let state = json!({"peers": {
            "players": [{"netId": 7, "endedTurn": false, "vote": null}],
            "awaiting": [],
            "inFlight": false,
        }});
        assert!(peers(&state, 2).is_err());
    }

    #[test]
    fn a_run_of_one_has_no_peers_to_read() {
        assert!(peers(&json!({}), 1).is_err());
    }
}
