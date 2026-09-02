//! Holding a game open, and asking it one thing at a time.
//! Where the game is ([`config`]), the process and socket while it runs ([`game`]), and what can go wrong between the two ([`error`]). What is said over that connection is [`crate::wire`].

pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod game;
