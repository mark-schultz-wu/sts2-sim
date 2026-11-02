//! A co-op seat played on the real game: one game process for each seat.
//!
//! The seats talk to each other over ENet, as the game's own co-op does. Seat
//! 0 hosts on [`SeatHost::port`] and the others join it there. Each game
//! answers for its own player only.

use sts2_core::{AscensionKinds as _, Decision, SeatAnswered, SeatIndex, Seated, Start};

use crate::session::error::Error;
use crate::wire::peers::{net_id, peers};
use crate::{Config, Oracle, Reply, Response, parse_reply};

/// The files a client's game waits for and writes, so the clients join the
/// lobby in seat order.
#[derive(Debug, Clone, Default)]
pub struct JoinOrder {
    /// What the client before this one writes once it has joined.
    pub after: Option<std::path::PathBuf>,
    /// What this client writes once it has joined.
    pub joined: Option<std::path::PathBuf>,
}

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
    /// How many seats the run has.
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

    /// Puts the seat's game into a position, as
    /// [`Oracle::setup`] does.
    /// What the game logged is kept for [`GameSeat::take_logged`].
    pub fn setup(&mut self, step: &sts2_core::Setup<V>) -> Result<sts2_core::Answered<V>, Error> {
        let answered = self.game.setup(step)?;
        self.logged.extend_from_slice(self.game.logged());
        Ok(answered)
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

impl<V: sts2_content_wire::WireContentVersion> GameSeat<V> {
    /// Launches the seat's game, which hosts or joins, and returns once the
    /// run has started. A run starts only once every seat has joined, so a
    /// lobby begins its seats at once.
    ///
    /// # Panics
    ///
    /// On an ascension or a modifier, which the mod's co-op lobby does not
    /// set yet.
    pub fn begin(
        host: SeatHost,
        start: &Start<V>,
        me: SeatIndex,
        order: JoinOrder,
    ) -> Result<Self, Error> {
        if start.ascension != V::AscensionLevel::NONE || !start.modifiers.is_empty() {
            unimplemented!("a co-op run with an ascension or modifiers");
        }
        let spelled = |character: V::Character| {
            serde_json::to_value(character)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .expect("a character is spelled as its id")
        };
        let character = spelled(start.players[me.at()].character);
        // Every player's wins, for the profile each seat's game reads them
        // off. A player's copy of The Architect reads the local profile's
        // count for that player's character.
        let wins: Vec<String> = (start.players.iter())
            .map(|player| format!("{}={}", spelled(player.character), player.wins))
            .collect();
        let role = match me.at() {
            0 => "host",
            _ => "client",
        };
        let mut env = vec![
            ("STS2_COOP_ROLE", role.to_owned()),
            ("STS2_COOP_PORT", host.port.to_string()),
            ("STS2_COOP_NETID", net_id(me).to_string()),
            ("STS2_COOP_PLAYERS", start.players.len().to_string()),
            ("STS2_COOP_CHARACTER", character),
            ("STS2_COOP_WINS", wins.join(",")),
            ("STS2_COOP_SEED", start.seed.as_str().to_owned()),
        ];
        if let Some(after) = order.after {
            env.push(("STS2_COOP_JOIN_AFTER", after.display().to_string()));
        }
        if let Some(joined) = order.joined {
            env.push(("STS2_COOP_JOINED", joined.display().to_string()));
        }
        let game = Oracle::launch_as(host.config, "coop", &env)?;
        Ok(Self {
            game,
            seats: start.players.len(),
            logged: Vec::new(),
        })
    }

    /// Submits one of this seat's moves, and returns once it is submitted.
    /// What the move does can wait on the other seats.
    pub fn play(&mut self, decision: &Decision<V>) -> Result<SeatAnswered<V>, Error> {
        let reply = self.game.dispatch(decision)?;
        self.seated(reply)
    }

    /// Where this seat stands, without moving it.
    pub fn observe(&mut self) -> Result<Seated<V>, Error> {
        let reply = parse_reply::<V>(&self.game.send_line(r#"{"cmd":"observe"}"#)?)?;
        match self.seated(reply)? {
            Ok(seated) => Ok(seated),
            Err(refusal) => Err(Error::NotObserved(refusal.to_string())),
        }
    }
}
