//! The Architect: the dialogue a won run ends on, then the win.

use sts2_core::rng::{Bound, GameRng};
use sts2_core::{
    EventBehavior, EventOption, EventOptionKey, EventView, HookEffect, HookEffects, Room,
};

use super::super::super::V0_107_1 as V;
use super::super::super::event_options::EventOptionId;
use super::super::super::generated::characters::Character;
use super::super::super::generated::events::Event;

/// One of `DefineDialogues`' dialogues, shown on one of the character's
/// wins. A repeating one joins the pool shown on every later win. The
/// localisation table's keys mark those with an `r`.
struct Dialogue {
    visit: u32,
    lines: u8,
    repeating: bool,
}

const fn said(visit: u32, lines: u8, repeating: bool) -> Dialogue {
    Dialogue {
        visit,
        lines,
        repeating,
    }
}

const IRONCLAD: [Dialogue; 3] = [said(0, 2, true), said(1, 3, true), said(2, 3, true)];
const SILENT: [Dialogue; 4] = [
    said(0, 1, true),
    said(1, 1, true),
    said(2, 1, true),
    said(3, 1, true),
];
const DEFECT: [Dialogue; 3] = [said(0, 3, true), said(1, 3, true), said(2, 3, true)];
const NECROBINDER: [Dialogue; 4] = [
    said(0, 2, false),
    said(1, 2, true),
    said(2, 2, true),
    said(3, 3, true),
];
const REGENT: [Dialogue; 3] = [said(0, 3, false), said(1, 3, true), said(2, 3, true)];

/// `CharacterDialogues`. A character with none offers the win alone.
fn dialogues(character: Character) -> &'static [Dialogue] {
    match character {
        Character::Ironclad => &IRONCLAD,
        Character::Silent => &SILENT,
        Character::Defect => &DEFECT,
        Character::Necrobinder => &NECROBINDER,
        Character::Regent => &REGENT,
        _ => &[],
    }
}

#[derive(Debug, Clone)]
pub struct State {
    /// How many lines the dialogue drawn has. None where no dialogue applies.
    lines: u8,
    /// `CurrentLineIndex`.
    line: u8,
}

pub struct TheArchitect;

impl EventBehavior<V> for TheArchitect {
    type State = State;

    /// `EventModel.IsAllowed`, which it keeps. No act offers it, and
    /// `RunManager.EnterNextAct` enters it past the last act's boss.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `LoadDialogue`. `GetValidDialogues` takes the character's dialogue for
    /// its wins so far, or else every repeating one shown on an earlier win.
    /// The Architect has no first-ever dialogue, so the profile's other wins
    /// count for nothing. Then `Rng.NextItem` of those, which draws nothing
    /// from an empty list.
    ///
    /// Its `Combat` layout builds the Architect as it opens, which the run
    /// does.
    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let all = dialogues(run.character());
        let wins = run.wins();
        let mut valid: Vec<&Dialogue> = all.iter().filter(|d| d.visit == wins).collect();
        if valid.is_empty() {
            valid = all
                .iter()
                .filter(|d| d.repeating && d.visit <= wins)
                .collect();
        }
        let lines = if valid.is_empty() {
            0
        } else {
            let count = num_traits::cast(valid.len()).expect("a few dialogues");
            let at: usize =
                num_traits::cast(rng.next_int(Bound::try_new(count).expect("a dialogue applies")))
                    .expect("a draw below the count is an index");
            valid[at].lines
        };
        State { lines, line: 0 }
    }

    /// One option. A line before the last moves the dialogue on, and the last
    /// line, or no dialogue, offers the bare `PROCEED`.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let option = if state.line + 1 < state.lines {
            EventOptionId::Dialogue(state.line)
        } else {
            EventOptionId::Proceed
        };
        Room::Event {
            event: Event::TheArchitect,
            finished: false,
            title: None,
            text: None,
            options: vec![EventOption {
                index: 0,
                key: EventOptionKey {
                    event: Event::TheArchitect,
                    option,
                },
                title: None,
                text: None,
                locked: false,
                proceed: false,
                chosen: false,
                relic: None,
            }],
        }
    }

    /// `AdvanceDialogue` on a line before the last. `WinRun` on `PROCEED`:
    /// the attack the screen plays, whose draws off the event's own stream
    /// leave with the room, then `ActChangeSynchronizer.SetLocalPlayerReady`,
    /// which enters the next act.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        _option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        if state.line + 1 < state.lines {
            state.line += 1;
            return HookEffects::new();
        }
        smallvec::smallvec![HookEffect::EnterNextAct]
    }
}
