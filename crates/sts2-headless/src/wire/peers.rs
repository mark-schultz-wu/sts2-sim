//! What a co-op seat's observation says about the other seats.
//!
//! The mod names a player by its net id. A seat names it by its place in the
//! lobby, so reading `peers` takes the net id each place was given.

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

/// The `peers` of a co-op seat's state, with each player at the place whose
/// net id `net_ids` lists.
pub(crate) fn peers(state: &serde_json::Value, net_ids: &[u64]) -> Result<Peers, Error> {
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
        net_ids
            .iter()
            .position(|&id| id == net_id)
            .map(|at| SeatIndex(u8::try_from(at).expect("at most 256 seats")))
            .ok_or_else(|| {
                fail(Why::Unknown {
                    what: "player's net id",
                    value: net_id.to_string(),
                })
            })
    };
    let mut seats = vec![Peer::default(); net_ids.len()];
    for player in &wire.players {
        let vote = player
            .vote
            .as_deref()
            .map(ident::coord)
            .transpose()
            .map_err(|e| fail(Why::Ident(e)))?;
        seats[place(player.net_id)?.at()] = Peer {
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
        seats,
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
            "players": [{"netId": 1000, "endedTurn": false, "vote": "2,3"},
                        {"netId": 1, "endedTurn": true, "vote": null}],
            "awaiting": [1000],
            "inFlight": false,
        }});
        let read = peers(&state, &[1, 1000]).expect("the peers read");
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
        assert!(peers(&state, &[1, 1000]).is_err());
    }

    #[test]
    fn a_run_of_one_has_no_peers_to_read() {
        assert!(peers(&json!({}), &[1]).is_err());
    }
}
