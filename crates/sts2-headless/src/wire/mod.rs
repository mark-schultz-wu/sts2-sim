//! What travels over the socket, in both directions.
//! One request, one reply, one greeting. Nothing here launches anything, holds a connection, or decides how any of it must look, those are [`crate::session`] and the front end's business respectively.
//! The split is worth keeping because these types are the protocol's shape and change with it. While the session's types are this crate's own and do not.

pub(crate) mod command;
pub(crate) mod greeting;
pub(crate) mod response;
