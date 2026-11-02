//! Holding a game open, and asking it one thing at a time.
//!
//! Where the game is ([`config`]), the process and socket while it runs
//! ([`game`]), and what can go wrong between the two ([`error`]). What is said
//! over that connection is [`crate::wire`].
//!
//! Two ways to hold one. [`Oracle`](game::Oracle) owns a child process and dies
//! with it. [`Session`](held::Session) reaches one a [`daemon`] is holding, so a
//! command can be a whole process and still land in a run forty decisions deep.
//! Both are driven the same way, and each has a run table, which is what lets a
//! front end not care.

pub(crate) mod cached;
pub(crate) mod config;
pub(crate) mod daemon;
pub(crate) mod error;
pub(crate) mod game;
pub(crate) mod held;
pub(crate) mod seat;
pub(crate) mod table;

use error::Error;
use sts2_core::Answered;

/// Turns one reply into the answer both implementations give.
///
/// Where the envelope is set aside and the state is read. Shared by the two
/// holders because it is the same reply either way: only how the bytes arrived
/// differs, and by this point they have.
///
/// A `Bye` here is a fault rather than an answer: nothing the typed API sends
/// can produce one.
pub(crate) fn receive<V: sts2_content_wire::WireContentVersion>(
    reply: crate::Reply<V>,
    last: &mut Option<crate::Response>,
) -> Result<Answered<V>, Error> {
    let response = match reply {
        Ok(response) => response,
        // A refusal means the game did not move, so the envelope from the
        // command before it is still what the run is standing in.
        Err(refusal) => return Ok(Err(refusal)),
    };

    let applied = response.applied::<V>()?;
    *last = Some(response);

    match applied {
        Some(applied) => Ok(Ok(applied)),
        None => Err(Error::SaidGoodbye),
    }
}

/// Where the run stands, from the envelope the last answer left behind.
///
/// What [`crate::Cached`] answers a look with. Looking spends no transcript
/// entry, because nothing moved.
pub(crate) fn snapshot_of<V: sts2_content_wire::WireContentVersion>(
    last: Option<&crate::Response>,
) -> Result<sts2_core::Snapshot<V>, Error> {
    let response = last.ok_or_else(|| {
        Error::NotObserved("nothing has been played, so nothing has been reported".to_string())
    })?;
    match response.applied::<V>()? {
        Some(applied) => Ok(applied.snapshot()),
        None => Err(Error::SaidGoodbye),
    }
}
