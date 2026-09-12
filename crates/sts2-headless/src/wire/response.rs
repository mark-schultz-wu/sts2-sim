//! What the game sends back, as an envelope.
//!
//! The protocol's own shape: a sequence number, the command it answers, the
//! game's own digest, and a payload. None of that is a fact about the run, so
//! none of it survives into [`sts2_core::Applied`]. That is what
//! [`Response::applied`] produces.
//!
//! `state` stays a [`serde_json::Value`] here and is read into
//! [`sts2_core::Observation`] on the way out. A `&Value` is a `Deserializer`,
//! so the mirror is built out of the state where it sits. The relay in
//! `sts2-cli` hands the same value on without reading it.
//!
//! This crate is strict about the envelope for a reason the crate README
//! records. Envelope drift is what the protocol version in the handshake is
//! for, and [`crate::Oracle`] checks it.

use serde::Deserialize;
use serde_json::Value;
use sts2_core::Applied;

use super::observation::{Error as ObservationError, observation};
use super::question::WireQuestion;

/// A request that the game accepted.
///
/// Exactly one reply arrives per request, though not always the kind the command
/// implies. Anything can be answered with [`Response::Pending`].
///
/// Two messages are deliberately absent. A [`sts2_core::Refusal`] is the inner
/// `Err` of a [`crate::Reply`], since it is the one reply that means nothing
/// happened. The greeting arrives once, before any request, and is consumed
/// during the handshake.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// The command advanced the game, and it is now idle.
    Observation {
        /// Increments per observation or pending, not per request.
        seq: u64,
        cmd: String,
        /// False when combat ended mid-cascade and the decision was abandoned
        /// rather than completed. Not an error. But a simulator that resolved
        /// it fully has diverged, even if the resulting state looks agreeable.
        resolved: bool,
        digest: String,
        /// The whole state the digest was taken over.
        state: Value,
        /// Errors the game logged while running the command. Never suppressed.
        /// The command can have half-happened, and only the caller decides
        /// whether to trust the state.
        #[serde(rename = "gameErrors", default)]
        game_errors: Option<Vec<String>>,
    },

    /// The game stopped mid-action to ask something.
    ///
    /// Until it is answered the game accepts only [`crate::Command::Answer`],
    /// `Command::Observe` and `Command::Quit`.
    Pending {
        seq: u64,
        cmd: String,
        /// A parked game still reports where it is: `digest` and `state` are
        /// attached to a pending exactly as they are to an observation.
        digest: String,
        state: Value,
        /// Flattened, because the mod writes a question's fields beside the
        /// envelope's rather than under a key.
        ///
        /// Which is also why `WireQuestion` is not `deny_unknown_fields`.
        /// A flattened deserializer sees `seq`, `cmd`, `digest` and `state`
        /// too, so refusing what it does not know refuses every pending.
        /// Every struct *inside* a question is strict, since none of them is
        /// flattened.
        #[serde(flatten)]
        question: Asked,
    },

    /// The reply to [`crate::Command::Quit`], and the last line the game sends.
    Bye,
}

impl Response {
    /// The state this reply carries, if it carries one.
    ///
    /// An observation and a pending both report where the run is standing, and
    /// a `Bye` does not. Untyped, because this is the value a relay passes on
    /// and [`Response::applied`] reads.
    pub fn state(&self) -> Option<&Value> {
        match self {
            Self::Observation { state, .. } | Self::Pending { state, .. } => Some(state),
            Self::Bye => None,
        }
    }

    /// What the game logged while running the command.
    ///
    /// Never suppressed and never an error on its own. The command can have
    /// half-happened, and only the caller decides whether to trust the state
    /// that came back. Empty for a pending and for a `Bye`, neither of which
    /// reports one.
    pub fn game_errors(&self) -> &[String] {
        match self {
            Self::Observation { game_errors, .. } => game_errors.as_deref().unwrap_or(&[]),
            Self::Pending { .. } | Self::Bye => &[],
        }
    }

    /// This reply as the vocabulary two implementations share.
    ///
    /// `None` for a `Bye`, which reports no run. Everything the envelope adds
    /// is dropped here: the sequence number, the command name, and the game's
    /// own digest. None of it is carried into a type a simulator also fills
    /// in.
    pub fn applied<V: sts2_content_wire::WireContentVersion>(
        &self,
    ) -> Result<Option<Applied<V>>, ObservationError> {
        let (state, resolved, asking) = match self {
            Self::Observation {
                state, resolved, ..
            } => (state, *resolved, None),

            // A parked game has not finished the decision, so it is not
            // resolved; `asking` is what tells that apart from a decision the
            // game abandoned.
            Self::Pending {
                state, question, ..
            } => (state, false, Some(question.clone())),

            Self::Bye => return Ok(None),
        };

        Ok(Some(Applied {
            observation: observation::<V>(state)?,
            asking: asking.map(|q| q.0.into_domain::<V>()).transpose()?,
            resolved,
        }))
    }
}

/// The question a pending carries, unread.
///
/// Opaque on purpose. A caller reads it through [`Response::applied`], which
/// produces the [`sts2_core::Question`] both implementations share. The wire's
/// own spelling of one has no business outside this crate. A public field on a
/// public enum variant cannot be made private any other way.
#[derive(Debug, Clone, Deserialize)]
pub struct Asked(WireQuestion);

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Response {
        serde_json::from_str(line).expect("sample should parse")
    }

    /// The client always asks for full observations, so one arriving without
    /// state means the game disagreed about that. Refusing it is the point of
    /// `state` not being an `Option`.
    #[test]
    fn an_observation_without_state_is_refused() {
        let err = serde_json::from_str::<Response>(
            r#"{"type":"observation","seq":3,"cmd":"play_card",
                "resolved":true,"digest":"A21E"}"#,
        )
        .expect_err("should refuse");
        assert!(
            err.to_string().contains("state"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn an_observation_carries_its_state_untouched() {
        let r = parse(
            r#"{"type":"observation","seq":4,"cmd":"observe","resolved":true,
                "digest":"B0","state":{"gold":99,"nested":{"hp":[1,2]}}}"#,
        );
        let Response::Observation {
            seq,
            resolved,
            state,
            game_errors,
            ..
        } = r
        else {
            panic!("expected observation");
        };
        assert_eq!(seq, 4);
        assert!(resolved);
        assert!(game_errors.is_none());
        assert_eq!(state["gold"], 99);
        assert_eq!(state["nested"]["hp"][1], 2);
    }

    #[test]
    fn game_errors_survive() {
        let r = parse(
            r#"{"type":"observation","seq":5,"cmd":"end_turn","resolved":false,
                "digest":"C1","state":{},"gameErrors":["boom"]}"#,
        );
        let Response::Observation {
            resolved,
            game_errors,
            ..
        } = r
        else {
            panic!("expected observation");
        };
        assert!(!resolved);
        assert_eq!(game_errors.as_deref(), Some(&["boom".to_string()][..]));
    }

    /// Regression. `Pending` had no fields for `digest` or `state`, and serde
    /// drops what it has nowhere to put. So every pending silently threw away
    /// the state of a parked game, which is when you most want it.
    #[test]
    fn a_pending_reports_where_the_game_is() {
        let r = parse(
            r#"{"type":"pending","seq":7,"cmd":"play_card","kind":"select_cards",
                "digest":"DEADBEEF","state":{"gold":99},
                "min":1,"max":1,"asker":null,"options":[]}"#,
        );
        let Response::Pending { digest, state, .. } = r else {
            panic!("expected pending");
        };
        assert_eq!(digest, "DEADBEEF");
        assert_eq!(state["gold"], 99);
    }

    /// The envelope and the question arrive flat in one object, so this pins
    /// that `flatten` plus an internally tagged enum works inside a variant.
    ///
    /// What the question *says* is `wire::question`'s to test. This is only
    /// that the two halves of a pending come off one object.
    #[test]
    fn a_question_flattens_into_the_envelope() {
        let r = parse(
            r#"{"type":"pending","seq":7,"cmd":"play_card","kind":"select_cards",
                "digest":"D1","state":{},"min":1,"max":2,"asker":"DiscardAction",
                "options":[{"index":0,"card":"STRIKE"},{"index":1,"card":"BASH+1"}]}"#,
        );
        let Response::Pending { seq, cmd, .. } = r else {
            panic!("expected pending");
        };
        assert_eq!(seq, 7);
        assert_eq!(cmd, "play_card");
    }

    #[test]
    fn bye_needs_nothing_else() {
        assert!(matches!(parse(r#"{"type":"bye"}"#), Response::Bye));
    }
}
