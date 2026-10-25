//! A co-op seat played on the real game: one game process for each seat.
//!
//! The seats talk to each other over ENet, as the game's own co-op does. Seat
//! 0 hosts on [`SeatHost::port`] and the others join it there. Each game
//! answers for its own player only.

use sts2_core::{Decision, Lobby, Seat, SeatAnswered, SeatIndex, Seated};

use crate::session::error::Error;
use crate::wire::peers::{net_id, peers};
use crate::{Config, Oracle, Reply, Response, parse_reply};

/// Where a seat's game is, and the port the lobby's host listens on.
#[derive(Debug, Clone)]
pub struct SeatHost {
    /// The seat's own game. Every seat needs a
    /// [`Config::data_home`] of its own, or the games race on the profile.
    pub config: Config,
    /// The UDP port seat 0 hosts on, the same for every seat.
    pub port: u16,
}

/// One seat of a co-op run, played by a game of its own.
#[derive(Debug)]
pub struct GameSeat<V: sts2_content_wire::WireContentVersion> {
    game: Oracle<V>,
    /// How many seats the lobby has.
    seats: usize,
    /// What the game logged in every reply since [`GameSeat::take_logged`].
    logged: Vec<String>,
}

impl<V: sts2_content_wire::WireContentVersion> GameSeat<V> {
    /// What the game logged in every reply since this was last asked, and
    /// then nothing until it logs again.
    ///
    /// Every reply, and not only the last as [`Oracle::logged`] reports. The
    /// seats act in turn, and a game can log while another seat's move
    /// plays. Looking at it is what reports that.
    pub fn take_logged(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logged)
    }

    /// The seat's game. See [`Oracle`].
    pub fn game(&self) -> &Oracle<V> {
        &self.game
    }

    /// Where the seat stands after a reply, or the game's refusal.
    fn seated(&mut self, reply: Reply<V>) -> Result<SeatAnswered<V>, Error> {
        let applied = match self.game.receive(reply)? {
            Ok(applied) => applied,
            Err(refusal) => return Ok(Err(refusal)),
        };
        self.logged.extend_from_slice(self.game.logged());
        let state = self
            .game
            .reported()
            .and_then(Response::state)
            .expect("a reply with a state is kept");
        Ok(Ok(Seated {
            snapshot: applied.snapshot(),
            peers: peers(state, self.seats)?,
        }))
    }
}

impl<V: sts2_content_wire::WireContentVersion> Seat<V> for GameSeat<V> {
    type Error = Error;
    type Host = SeatHost;

    /// Launches the seat's game, which hosts or joins, and returns once the
    /// run has started.
    fn begin(host: SeatHost, lobby: &Lobby<V>, me: SeatIndex) -> Result<Self, Error> {
        let character = serde_json::to_value(lobby.characters[me.at()])
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .expect("a character is spelled as its id");
        let role = match me.at() {
            0 => "host",
            _ => "client",
        };
        let game = Oracle::launch_as(
            host.config,
            "coop",
            &[
                ("STS2_COOP_ROLE", role.to_owned()),
                ("STS2_COOP_PORT", host.port.to_string()),
                ("STS2_COOP_NETID", net_id(me).to_string()),
                ("STS2_COOP_PLAYERS", lobby.characters.len().to_string()),
                ("STS2_COOP_CHARACTER", character),
                ("STS2_COOP_SEED", lobby.seed.as_str().to_owned()),
            ],
        )?;
        Ok(Self {
            game,
            seats: lobby.characters.len(),
            logged: Vec::new(),
        })
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<SeatAnswered<V>, Error> {
        let reply = self.game.dispatch(decision)?;
        self.seated(reply)
    }

    fn observe(&mut self) -> Result<Seated<V>, Error> {
        let reply = parse_reply::<V>(&self.game.send_line(r#"{"cmd":"observe"}"#)?)?;
        match self.seated(reply)? {
            Ok(seated) => Ok(seated),
            Err(refusal) => Err(Error::NotObserved(refusal.to_string())),
        }
    }
}
