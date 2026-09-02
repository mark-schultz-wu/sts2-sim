//! What travels over the socket, in both directions.
//!
//! One reply, one greeting. Nothing here launches anything, holds a connection,
//! or decides how any of it looks. Those are [`crate::session`] and the front
//! end's business respectively.
//!
//! Requests are not here any more. A decision is `sts2_core::Decision`, which
//! belongs to the vocabulary two implementations share rather than to the one
//! that happens to speak it over a socket.

pub(crate) mod command;
pub(crate) mod greeting;
pub(crate) mod ident;
pub(crate) mod observation;
pub(crate) mod question;
pub(crate) mod refusal;
pub(crate) mod response;
