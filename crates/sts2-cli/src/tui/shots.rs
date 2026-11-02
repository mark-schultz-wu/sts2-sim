//! Screens, rendered off a fixture and read back as text.
//!
//! A terminal front end is the one thing in this repo that cannot be checked by
//! reading its return value. So it is checked by drawing it. Every screen here
//! is painted into a buffer at a stated size and asserted against what a person
//! sees. That is the enemy's damage on the screen, an unplayable card saying
//! why, and eighty columns still fitting.
//!
//! It doubles as the way the screens are worked on. `cargo test -- --nocapture
//! shots` prints them. The game is a twenty-second boot and a Steam install
//! away, and a layout needs neither.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use sts2_core::Applied;

use super::app::App;
use super::state::Options;

type V = sts2_content::v0_107_1::V0_107_1;

/// Draws one frame and gives back the buffer, for a claim about colour.
///
/// Returns the cells so tests can inspect properties such as colour.
fn painted(
    applied: Applied<V>,
    w: u16,
    h: u16,
    before: impl FnOnce(&mut App<V>),
) -> ratatui::buffer::Buffer {
    let mut app = App::new(Options::default());
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("a test terminal");
    terminal
        .draw(|frame| super::screen::draw(frame, &app))
        .expect("a first frame");
    app.show(applied.snapshot());
    before(&mut app);
    terminal
        .draw(|frame| super::screen::draw(frame, &app))
        .expect("a frame");
    terminal.backend().buffer().clone()
}

/// Draws one frame at `w` by `h` and gives back what is on it.
fn shot(applied: Applied<V>, w: u16, h: u16, before: impl FnOnce(&mut App<V>)) -> String {
    let mut app = App::new(Options::default());
    // Drawn once with nothing shown, then showing the fixture.
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("a test terminal");
    terminal
        .draw(|frame| {
            super::screen::draw(frame, &app);
        })
        .expect("a first frame");

    app.show(applied.snapshot());
    before(&mut app);

    terminal
        .draw(|frame| super::screen::draw(frame, &app))
        .expect("a frame");

    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use serde_json::json;

    fn a_fight() -> Applied<V> {
        fixture::applied(json!({
            "floor": 6, "hp": 61, "gold": 138, "inCombat": true,
            "relics": ["BURNING_BLOOD"], "potions": ["FIRE_POTION"],
            "deck": ["STRIKE_IRONCLAD", "BASH", "DEFEND_IRONCLAD"],
            "combat": fixture::combat(json!({
                "turn": 3, "energy": 2,
                "hand": ["STRIKE_IRONCLAD", "BASH", "ARMAMENTS", "DEFEND_IRONCLAD", "INFLAME"],
                "draw": ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD"],
                "discard": ["STRIKE_IRONCLAD"],
                "creatures": [
                    // `CHARACTER/IRONCLAD`, which is what the game reports —
                    // this fixture said `None` and so agreed with a bug that
                    // drew the player as a third enemy.
                    fixture::creature(0, "Player", Some("CHARACTER/IRONCLAD"),
                                      &[("STRENGTH_POWER", 2)]),
                    fixture::creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"),
                                      &[("VULNERABLE_POWER", 3)]),
                    fixture::creature(2, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                ],
                "legal": [
                    fixture::playable("STRIKE_IRONCLAD", &[1, 2]),
                    fixture::playable("BASH", &[1, 2]),
                    fixture::unplayable("ARMAMENTS", "EnergyCostTooHigh"),
                    fixture::playable("DEFEND_IRONCLAD", &[]),
                    fixture::playable("INFLAME", &[]),
                ],
            })),
        }))
    }

    #[test]
    fn a_fight_at_a_hundred_columns() {
        let out = shot(a_fight(), 100, 34, |_| {});
        println!("\n{out}\n");

        // The four things a player has to be able to see.
        assert!(out.contains("61/80"), "the player's hit points");
        assert!(out.contains("Bowlbug Rock"), "who is being fought");
        assert!(out.contains("energy 2"), "the energy left");
        assert!(out.contains("Strike"), "the hand, by name");

        // The player belongs under `you`, not among the creatures being
        // fought. Split on the two headings and check which side it lands.
        let (them, you) = out.split_once(" you ").expect("the player has a panel");
        assert!(
            !them.contains("Ironclad\n") && !them.contains("Ironclad "),
            "the player is drawn as an enemy:\n{them}"
        );
        assert!(you.contains("energy 2"), "the player's own row:\n{you}");
    }

    /// The tallest run of blank rows in the fight itself, from the top of the
    /// screen down to the hand.
    ///
    /// Scoped there rather than over the whole screen, because an empty log is
    /// not a hole. It is a panel with nothing in it yet, and it fills as the
    /// run is played.
    fn widest_gap_in_the_fight(out: &str) -> usize {
        out.lines()
            .take_while(|line| !line.contains("hand ("))
            .fold((0usize, 0usize), |(run, worst), line| {
                match line.trim().is_empty() {
                    true => (run + 1, worst.max(run + 1)),
                    false => (0, worst),
                }
            })
            .1
    }

    /// A tall terminal must not put the two halves of a turn a screen apart.
    ///
    /// The fight's spacer used to be `Min(0)`, so it took every spare row. At
    /// 200x55 that was twenty-eight blank rows between the enemies and the
    /// hand. The enemy a card was aimed at was nowhere near the card. The gap
    /// is bounded now, and the surplus goes to the log.
    #[test]
    fn a_tall_terminal_does_not_open_a_canyon() {
        let out = shot(a_fight(), 200, 55, |_| {});
        println!("\n{out}\n");

        let gap = widest_gap_in_the_fight(&out);
        assert!(gap <= 8, "{gap} blank rows inside the fight:\n{out}");

        // Bounded, not merely smaller. The first attempt left two flexible
        // constraints to share the surplus, which halved the gap and left it
        // still growing with the screen — so this asserts at a height where
        // half of it would be far too much.
        let taller = shot(a_fight(), 200, 90, |_| {});
        let gap = widest_gap_in_the_fight(&taller);
        assert!(gap <= 8, "{gap} blank rows at 90 rows:\n{taller}");

        // And the whole fight is still on it, which is the thing the gap was
        // pushing off the bottom.
        assert!(out.contains("Bowlbug Rock"), "{out}");
        assert!(out.contains("Strike"), "{out}");
        assert!(out.contains("draw 2"), "{out}");
    }

    /// The log grows into the space the fight gave up, rather than the space
    /// going nowhere.
    #[test]
    fn a_tall_terminal_gives_the_surplus_to_the_log() {
        assert_eq!(super::super::screen::log_rows(24), 3);
        assert_eq!(super::super::screen::log_rows(34), 5);
        assert!(super::super::screen::log_rows(55) > 5);
        // Capped well short of the screen: past a dozen lines the extra rows
        // are empty most of the time, and a rule with twenty blank rows under
        // it reads as a broken panel.
        assert!(super::super::screen::log_rows(200) <= 12);
    }

    /// A pet fights for you and belongs on your side of the screen.
    ///
    /// `BYRDPIP` is a relic with `spawnsPets`, and its bird is a monster model
    /// on the player's side. Splitting the fight by "is this the player" put it
    /// among the enemies. That meant nine thousand hit points, no intent, and a
    /// card that can be aimed at it.
    #[test]
    fn a_pet_is_drawn_on_your_side_not_theirs() {
        let applied = fixture::applied(json!({
            "floor": 6, "hp": 61, "inCombat": true,
            "combat": fixture::combat(json!({
                "hand": ["STRIKE_IRONCLAD"],
                "creatures": [
                    fixture::creature(0, "Player", Some("CHARACTER/IRONCLAD"), &[]),
                    // The pet: a monster model, on the player's side.
                    fixture::creature(3, "Player", Some("MONSTER/BYRDPIP"), &[]),
                    fixture::creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                ],
                "legal": [fixture::playable("STRIKE_IRONCLAD", &[1])],
            })),
        }));
        let out = shot(applied, 100, 40, |_| {});
        println!("\n{out}\n");

        let (them, yours) = out.split_once(" yours ").expect("an ally row");
        assert!(
            !them.contains("Byrdpip"),
            "the pet is drawn as an enemy:\n{them}"
        );
        assert!(yours.contains("Byrdpip"), "the pet is nowhere:\n{out}");
        assert!(them.contains("Bowlbug Rock"), "the enemy moved:\n{them}");

        // And the player still has its own row, not a panel among the pets.
        let (_, you) = out.split_once(" you ").expect("the player has a row");
        assert!(you.contains("energy"), "{you}");
    }

    /// No pet, no row. A fight with nothing on your side but you does not pay
    /// seven rows for an empty rule.
    #[test]
    fn a_fight_with_no_pet_has_no_ally_row() {
        let out = shot(a_fight(), 100, 34, |_| {});
        assert!(!out.contains(" yours "), "{out}");
    }

    /// The smallest terminal anyone plays in. Every panel has a floor, so this
    /// has to be a worse screen and not a broken one.
    #[test]
    fn a_fight_at_eighty_by_twenty_four() {
        let out = shot(a_fight(), 80, 24, |_| {});
        println!("\n{out}\n");

        assert!(out.contains("61/80"));
        assert!(out.contains("energy 2"));
        for line in out.lines() {
            assert!(
                line.chars().count() <= 80,
                "{line:?} is wider than the screen"
            );
        }
    }

    /// A card that cannot be played says so on its face, so the player is not
    /// left pressing enter at it.
    #[test]
    fn an_unplayable_card_says_why() {
        let out = shot(a_fight(), 120, 34, |_| {});
        assert!(out.contains("Energy Cost"), "{out}");
    }

    #[test]
    fn the_help_panel_lists_the_keys() {
        let out = shot(a_fight(), 100, 34, |app| {
            app.press(super::super::keys::Action::Help);
        });
        println!("\n{out}\n");
        assert!(out.contains("end the turn"), "{out}");
    }

    /// Aiming shows which creature the card is pointed at, and does not send
    /// anything until it is confirmed.
    #[test]
    fn a_card_with_two_targets_asks_which() {
        let out = shot(a_fight(), 100, 34, |app| {
            app.press(super::super::keys::Action::Confirm);
        });
        println!("\n{out}\n");
        assert!(
            out.contains("aim"),
            "the key bar changes while aiming: {out}"
        );
    }

    fn a_fight_with_a_belt() -> Applied<V> {
        fixture::applied(json!({
            "floor": 6, "hp": 61, "inCombat": true,
            "relics": [
                {"id": "BURNING_BLOOD", "title": "Burning Blood",
                 "text": "At the end of combat, heal [green]6[/green] HP."},
            ],
            "potions": [
                {"id": "FIRE_POTION", "title": "Fire Potion",
                 "text": "Deal [green]20[/green] damage.",
                 "usable": true, "reason": null, "targets": [1, 2]},
                {"id": "BLOOD_POTION", "title": "Blood Potion",
                 "text": "Heal 10 HP.",
                 "usable": false, "reason": "ItsOwnCheck", "targets": []},
            ],
            "combat": fixture::combat(json!({
                "turn": 3, "energy": 2,
                "hand": ["STRIKE_IRONCLAD"],
                "creatures": [
                    fixture::creature(0, "Player", Some("CHARACTER/IRONCLAD"), &[]),
                    fixture::creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                    fixture::creature(2, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                ],
                "legal": [fixture::playable("STRIKE_IRONCLAD", &[1, 2])],
            })),
        }))
    }

    /// The belt says what each potion does and which cannot be drunk, in the
    /// game's own words rather than in a de-cased id.
    #[test]
    fn the_belt_says_what_each_potion_does() {
        let out = shot(a_fight_with_a_belt(), 100, 34, |app| {
            app.press(super::super::keys::Action::ShowPotions);
        });
        println!("\n{out}\n");

        assert!(out.contains("Fire Potion"), "the game's own name: {out}");
        assert!(out.contains("Deal 20 damage."), "and its words: {out}");
        assert!(!out.contains("[green]"), "with the markup gone: {out}");
        // The reason, in words a player can act on rather than a variant name.
        assert!(out.contains("not right now"), "{out}");
        assert!(!out.contains("ItsOwnCheck"), "{out}");
    }

    /// A potion that cannot be drunk keeps its row and loses its digit, the way
    /// a taken reward does. So a digit never takes something else.
    #[test]
    fn an_undrinkable_potion_keeps_its_row_and_loses_its_digit() {
        let out = shot(a_fight_with_a_belt(), 100, 34, |app| {
            app.press(super::super::keys::Action::ShowPotions);
        });

        assert!(out.contains("Blood Potion"), "still listed: {out}");
        assert!(out.contains("[1]"), "the drinkable one has a digit: {out}");
        assert!(!out.contains("[2]"), "the other one kept a digit: {out}");
    }

    /// Confirming a potion with two targets aims it rather than drinking it,
    /// and the bar says what confirming does now.
    #[test]
    fn a_potion_with_two_targets_asks_which() {
        let out = shot(a_fight_with_a_belt(), 100, 34, |app| {
            app.press(super::super::keys::Action::ShowPotions);
            app.press(super::super::keys::Action::Confirm);
        });
        println!("\n{out}\n");

        assert!(out.contains("aim"), "the bar changes while aiming: {out}");
        // Named, because the belt closed to get out of the way and this line is
        // the only thing still saying what is in flight.
        assert!(
            out.contains("drink Fire Potion"),
            "and says what it would drink, not `play`: {out}"
        );
        // The belt is out of the way, so the creature being pointed at is on
        // the screen. Its title rather than a potion's name: the run strip
        // lists the belt as well and would answer either way.
        assert!(
            !out.contains("potions (2/3)"),
            "the belt is still up: {out}"
        );
    }

    #[test]
    fn the_relic_panel_says_what_a_relic_does() {
        let out = shot(a_fight_with_a_belt(), 100, 34, |app| {
            app.press(super::super::keys::Action::ShowRelics);
        });
        println!("\n{out}\n");

        assert!(out.contains("Burning Blood"), "{out}");
        assert!(out.contains("At the end of combat, heal 6 HP."), "{out}");
    }

    /// The simulator's run carries no words. A relic still says what it does,
    /// from the game's text table with the dumped number filled in.
    #[test]
    fn a_relic_with_no_words_reads_from_the_text_table() {
        let out = shot(
            fixture::applied(json!({"relics": ["BURNING_BLOOD"]})),
            100,
            34,
            |app| {
                app.press(super::super::keys::Action::ShowRelics);
            },
        );
        assert!(out.contains("Burning Blood"), "{out}");
        assert!(out.contains("At the end of combat, heal 6 HP."), "{out}");
    }

    /// The run strip names relics and potions the way the game does, so the
    /// name on the strip is the name in the panel.
    #[test]
    fn the_run_strip_uses_the_games_own_names() {
        let out = shot(a_fight_with_a_belt(), 100, 34, |_| {});
        assert!(out.contains("Burning Blood"), "{out}");
        assert!(!out.contains("BURNING_BLOOD"), "{out}");
    }

    /// Which *column* a character is in, which is not where it is in the
    /// string. Every box-drawing glyph is three bytes, so a byte offset
    /// compares two lines by how much box they happen to contain.
    fn column(line: &str, want: char) -> Option<usize> {
        line.chars().position(|c| c == want)
    }

    fn a_map() -> Applied<V> {
        fixture::applied(json!({
            "floor": 3, "hp": 54, "gold": 210,
            "map": {
                "at": "1,2",
                "visited": ["0,0", "1,1", "1,2"],
                "travelable": [
                    {"at": "0,3", "type": "Monster"},
                    {"at": "2,3", "type": "RestSite"},
                ],
                "points": [
                    {"at": "0,0", "type": "Monster", "to": ["1,1"]},
                    {"at": "1,1", "type": "Unknown", "to": ["1,2"]},
                    {"at": "1,2", "type": "Shop", "to": ["0,3", "2,3"]},
                    {"at": "0,3", "type": "Monster", "to": ["1,4"]},
                    {"at": "2,3", "type": "RestSite", "to": ["1,4"]},
                    {"at": "1,4", "type": "Elite", "to": []},
                ],
                "proceed": false
            },
            "moves": [
                {"cmd": "travel", "col": 0, "row": 3},
                {"cmd": "travel", "col": 2, "row": 3}
            ]
        }))
    }

    #[test]
    fn the_map_marks_where_the_run_can_go() {
        let out = shot(a_map(), 90, 28, |_| {});
        println!("\n{out}\n");

        // The digit beside a room is the key that goes there, so the picture
        // and the menu are the same thing.
        assert!(
            out.contains("M1"),
            "the first destination is numbered: {out}"
        );
        assert!(out.contains("R2"), "and so is the second: {out}");
        assert!(
            out.contains("a fire"),
            "spelled out under the picture: {out}"
        );
    }

    /// The screen needs no terminal: keys of its own go in, and it draws into
    /// whatever backend it is handed. An ssh session or a browser drives it
    /// this way.
    #[test]
    fn the_screen_plays_on_any_backend_with_its_own_keys() {
        use crate::tui::input::{Key, KeyCode};

        let mut app = App::new(Options::default());
        app.show(a_fight().snapshot());
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("a test terminal");
        app.key(Key::from(KeyCode::Char('d')));
        terminal.draw(|frame| app.draw(frame)).expect("a frame");
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
            .map(|at| buffer[at].symbol())
            .collect();
        assert!(text.contains("Deck (3)"), "the deck panel opened: {text}");
    }

    /// A card's frame says what kind it is: an attack is red, a skill blue,
    /// and a power yellow.
    #[test]
    fn a_cards_frame_is_coloured_by_its_kind() {
        use ratatui::style::Color;

        let buffer = painted(a_fight(), 100, 30, |_| {});
        let row = (0..buffer.area.height)
            .find(|&y| {
                let line: String = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                line.contains("╭(1)")
            })
            .expect("a hand of cards");
        // The hand in the fixture: Strike, Bash, Armaments, Defend, Inflame.
        // The cursor is on Strike and Armaments cannot be played, so those
        // two frames say that instead.
        let frames: Vec<Option<Color>> = (0..buffer.area.width)
            .filter(|&x| buffer[(x, row)].symbol() == "╭")
            .map(|x| buffer[(x, row)].fg.into())
            .collect();
        assert_eq!(
            [frames[1], frames[3], frames[4]],
            [Color::Red, Color::Blue, Color::Yellow].map(Some),
            "{frames:?}"
        );
    }

    /// A refusal pops up over the screen, even one too short for a log.
    #[test]
    fn a_refusal_pops_up_over_a_short_screen() {
        let out = shot(a_fight(), 80, 22, |app| {
            app.ui
                .say(super::super::state::Tone::Refused, "not enough energy");
        });
        let line = (out.lines())
            .find(|line| line.contains("not enough energy"))
            .unwrap_or_else(|| panic!("no toast: {out}"));
        assert!(line.trim_end().ends_with('│'), "boxed at the right: {line}");
    }

    /// Between two rows of rooms, the paths: `╲` and `╱` to the next column
    /// over, `│` straight on.
    #[test]
    fn the_map_draws_the_paths_between_rooms() {
        let out = shot(a_map(), 90, 40, |_| {});
        let lines: Vec<&str> = out.lines().collect();
        let row = |n: &str| {
            (lines.iter())
                .position(|line| line.trim_start().starts_with(n))
                .unwrap_or_else(|| panic!("no row {n}: {out}"))
        };
        // From the shop at 1,2 out to both rooms of row 3, and from both of
        // those into the elite at 1,4.
        assert_eq!(lines[row("3 ") + 1].trim_end(), "      ╲   ╱", "{out}");
        assert_eq!(lines[row("4 ") + 1].trim_end(), "      ╱   ╲", "{out}");
        assert_eq!(lines[row("2 ") + 1].trim_end(), "        │", "{out}");
    }

    /// An option the game gave no title reads as the last part of its key,
    /// and the relic it names is not said twice.
    #[test]
    fn an_untitled_option_reads_as_its_name() {
        let out = shot(
            fixture::applied(json!({
                "room": {
                    "type": "Event", "id": "NEOW", "title": null, "text": null,
                    "options": [{"index": 0, "key": "NEOW.pages.INITIAL.options.WINGED_BOOTS",
                                 "title": null, "relic": "WINGED_BOOTS"}]
                },
                "moves": [{"cmd": "choose_option", "index": 0}]
            })),
            90,
            20,
            |_| {},
        );
        assert!(out.contains(" 1  Winged Boots\n"), "{out}");
        assert!(!out.contains("pages"), "{out}");
    }

    fn an_event() -> Applied<V> {
        fixture::applied(json!({
            "floor": 4, "hp": 48,
            "room": {
                "type": "Event",
                "id": "WELCOME_TO_WONGOS",
                "title": "Welcome to Wongo's",
                "text": "A spring of [gold]clear water[/gold] runs from the rock.",
                "options": [
                    {"index": 0, "key": "WELCOME_TO_WONGOS.pages.INITIAL.options.BARGAIN_BIN",
                     "title": "Drink", "text": "Heal [green]12[/green] HP."},
                    {"index": 1, "key": "WELCOME_TO_WONGOS.pages.INITIAL.options.FEATURED_ITEM",
                     "title": "Fill a flask", "text": "Gain a potion."},
                    {"index": 2, "key": "WELCOME_TO_WONGOS.pages.INITIAL.options.LEAVE",
                     "title": "Leave", "proceed": true},
                ]
            },
            "moves": [{"cmd": "choose_option", "index": 0}, {"cmd": "choose_option", "index": 1}, {"cmd": "choose_option", "index": 2}]
        }))
    }

    #[test]
    fn an_event_reads_as_prose_and_a_list() {
        let out = shot(an_event(), 90, 28, |_| {});
        println!("\n{out}\n");

        assert!(
            out.contains("clear water"),
            "the game's words, markup gone: {out}"
        );
        assert!(
            !out.contains("[gold]"),
            "and the markup really is gone: {out}"
        );
        assert!(out.contains("Drink"), "{out}");
        // Only the option under the cursor spells itself out.
        assert!(out.contains("Heal 12 HP."), "{out}");
        assert!(!out.contains("Gain a potion."), "{out}");
    }

    #[test]
    fn a_shop_prices_what_it_cannot_sell_you() {
        let applied = fixture::applied(json!({
            "gold": 40,
            "room": {
                "type": "Shop",
                "entries": [
                    {"index": 0, "kind": "MerchantCardEntry", "id": "BASH",
                     "cost": 50, "stocked": true, "affordable": false},
                    {"index": 1, "kind": "MerchantRelicEntry", "id": "AKABEKO",
                     "cost": 30, "stocked": true, "affordable": true},
                ]
            },
            "moves": [{"cmd": "buy", "index": 1}]
        }));
        let out = shot(applied, 90, 24, |_| {});
        println!("\n{out}\n");
        assert!(out.contains("50 gold"), "{out}");
        assert!(out.contains("short"), "unaffordable says so: {out}");
    }

    /// Neow: an event whose options each hand over a relic, with the deck
    /// panel open over it. Reported from a real run as an unreadable screen.
    fn neow_with_the_deck_open() -> Applied<V> {
        fixture::applied(json!({
            "floor": 0, "hp": 80, "gold": 99,
            "deck": [
                "STRIKE_IRONCLAD", "STRIKE_IRONCLAD", "STRIKE_IRONCLAD",
                "STRIKE_IRONCLAD", "STRIKE_IRONCLAD",
                "DEFEND_IRONCLAD", "DEFEND_IRONCLAD", "DEFEND_IRONCLAD",
                "DEFEND_IRONCLAD", "BASH"
            ],
            "room": {
                "type": "Event",
                "id": "WELCOME_TO_WONGOS",
                "title": null,
                "text": null,
                "options": [
                    {"index": 0,
                     "key": "WELCOME_TO_WONGOS.pages.INITIAL.options.LEAD_PAPERWEIGHT",
                     "title": "Lead Paperweight", "relic": "LEAD_PAPERWEIGHT"},
                    {"index": 1, "key": "WELCOME_TO_WONGOS.pages.INITIAL.options.KALEIDOSCOPE",
                     "title": "Kaleidoscope",
                     "text": "Obtain 2 card rewards from other characters.",
                     "relic": "KALEIDOSCOPE"},
                    {"index": 2, "key": "WELCOME_TO_WONGOS.pages.INITIAL.options.HEFTY_TABLET",
                     "title": "Hefty Tablet", "relic": "HEFTY_TABLET"},
                ]
            }
        }))
    }

    #[test]
    fn the_deck_panel_sits_over_the_room_without_tearing_it() {
        let out = shot(neow_with_the_deck_open(), 124, 30, |app| {
            app.press(super::super::keys::Action::ShowDeck);
        });
        println!("\n{out}\n");

        // The panel is one rectangle: its title, its contents and its right
        // edge all begin at the same column.
        let top = out
            .lines()
            .find(|l| l.contains("Deck (10)"))
            .expect("the panel has a title");
        let left = column(top, '╭').expect("and a top-left corner");
        let listed = out
            .lines()
            .find(|l| l.contains("Strike"))
            .expect("the deck is listed");
        assert!(
            column(listed, '│').is_some_and(|at| at == left),
            "the contents are drawn outside the panel:\n{top}\n{listed}"
        );

        // An option whose title already names the relic must not name it twice.
        for line in out.lines() {
            assert!(
                !line.contains("Lead Paperweight   Lead Paperweight"),
                "the relic repeats the title: {line:?}"
            );
        }
    }

    /// A reward set with one already taken. The digits and the list the keys
    /// act on are built in two places and must agree. When they did not, the
    /// number under a card took the reward beside it.
    #[test]
    fn a_taken_reward_keeps_its_box_and_loses_its_digit() {
        let applied = fixture::answered(json!({
            "type": "pending", "seq": 9, "cmd": "end_turn", "digest": "AA",
            "kind": "rewards", "skippable": true,
            "rewards": [
                {"index": 0, "type": "GoldReward", "populated": true, "taken": true,
                 "amount": 13},
                {"index": 1, "type": "CardReward", "populated": true, "taken": false,
                 "cards": [
                     {"card": "PERFECTED_STRIKE", "text": "Deal 6 damage."},
                     {"card": "BODY_SLAM", "text": "Deal damage equal to your Block."}
                 ]},
            ],
            "state": {}
        }));
        let out = shot(applied, 100, 24, |_| {});
        println!("\n{out}\n");

        assert!(
            out.contains("13 gold"),
            "the taken reward is still shown: {out}"
        );
        assert!(out.contains("[1]") && out.contains("[2]"), "{out}");
        // Two digits, not three: the gold is no longer answerable.
        assert!(!out.contains("[3]"), "a taken reward kept a digit: {out}");
    }

    /// A card that cannot be played is refused here, off the game's own reason,
    /// rather than sent to be told again.
    #[test]
    fn an_unplayable_card_is_not_sent() {
        let out = shot(a_fight(), 100, 34, |app| {
            // The third card is the unplayable one.
            app.press(super::super::keys::Action::Nth(2));
            app.press(super::super::keys::Action::Confirm);
        });
        println!("\n{out}\n");
        assert!(
            out.contains("cannot be played"),
            "the log should say so, without a round trip: {out}"
        );
    }

    /// The map is colour coded, and the code is the room type.
    ///
    /// Asserted on the painted cells rather than on the text. A map drawn
    /// entirely in white still prints `E` where the elite is, so nothing about
    /// the letters catches this going wrong.
    #[test]
    fn the_map_is_coloured_by_what_the_room_is() {
        use ratatui::style::Color;

        let buffer = painted(a_map(), 90, 28, |_| {});
        let mut seen: Vec<(char, Option<Color>)> = Vec::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                let ch = cell.symbol().chars().next().unwrap_or(' ');
                if matches!(ch, 'E' | 'R' | 'M' | '?' | '@') {
                    seen.push((ch, cell.fg.into()));
                }
            }
        }

        let colour = |want: char| {
            seen.iter()
                .find(|(ch, _)| *ch == want)
                .map(|(_, fg)| *fg)
                .unwrap_or_else(|| panic!("no {want} on the map"))
        };

        assert_eq!(colour('E'), Some(Color::Red), "an elite is a warning");
        assert_eq!(colour('R'), Some(Color::Green), "a fire is a kindness");
        assert_eq!(colour('?'), Some(Color::Cyan));
        // The common case stays quiet, so the rest can stand out against it.
        assert_ne!(colour('M'), Some(Color::Red));
        assert_ne!(colour('M'), Some(Color::Green));
    }

    /// The screen over the simulator's table, as `sts2 sim` plays it. A run
    /// opens, and pressing on takes it past Neow, with every frame drawn.
    #[test]
    fn the_simulator_plays_on_the_screen() {
        use super::super::keys::Action;

        let start = sts2_core::Start::<V> {
            seed: sts2_core::Seed::new("C000000042").expect("a seed"),
            players: vec![sts2_core::Player {
                character: sts2_content::v0_107_1::characters::Character::Ironclad,
                wins: 0,
            }],
            ascension: crate::cli::ascension::<V>("None").expect("no ascension"),
            modifiers: Vec::new(),
        };
        let table = <sts2_sim::SimTable<V> as sts2_core::RunTable<V>>::begin((), &start)
            .expect("the run starts");
        let (mut app, _) = crate::tui::check::Driven::new(table);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("a test terminal");
        let mut frame = |app: &App<V>| {
            terminal
                .draw(|frame| super::super::screen::draw(frame, app))
                .expect("a frame");
            format!("{:?}", terminal.backend().buffer())
        };
        let opening = frame(&app);
        assert!(opening.contains("Neow"), "{opening}");

        let floor = |app: &App<V>| app.view().map(|seen| seen.floor);
        let before = floor(&app);
        for _ in 0..12 {
            if app.moves().is_empty() {
                break;
            }
            app.press(Action::Confirm);
            frame(&app);
        }
        assert_ne!(floor(&app), before, "the run left Neow's floor");
    }

    /// Every kind of screen a run reaches, drawn off the simulator and
    /// printed. A tool for working on the screens rather than a check:
    ///
    /// ```text
    /// cargo test -p sts2-cli screens_of_a_run -- --ignored --nocapture
    /// SHOTS_SEED=C000000042 SHOTS_SIZE=80x24 cargo test ...
    /// ```
    ///
    /// The run takes legal moves at random, from a fixed generator, and the
    /// first two of each kind of screen are printed.
    #[test]
    #[ignore = "prints screens to look at; asserts nothing"]
    fn screens_of_a_run() {
        let seed = std::env::var("SHOTS_SEED").unwrap_or_else(|_| "C000000077".into());
        let (w, h) = std::env::var("SHOTS_SIZE")
            .ok()
            .and_then(|size| {
                let (w, h) = size.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((100, 30));
        let start = sts2_core::Start::<V> {
            seed: sts2_core::Seed::new(&seed).expect("a seed"),
            players: vec![sts2_core::Player {
                character: sts2_content::v0_107_1::characters::Character::Silent,
                wins: 0,
            }],
            ascension: crate::cli::ascension::<V>("None").expect("no ascension"),
            modifiers: Vec::new(),
        };
        let mut sim = sts2_sim::Sim::<V>::begin(&start).expect("the run starts");
        let mut printed: std::collections::BTreeMap<String, usize> = Default::default();
        // xorshift, so the run is the same each time without a dependency.
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for step in 0..2000 {
            let standing = sim.observe().expect("the simulator answers");
            let seen = standing.view();
            let kind = match (&seen.asking, &seen.room) {
                (Some(question), _) => format!("{:?}", question.kind()),
                _ if seen.run_over => "over".into(),
                _ if seen.fight().is_some() => "fight".into(),
                (None, Some(room)) => format!("{room:?}")
                    .split([' ', '{'])
                    .next()
                    .unwrap_or("room")
                    .into(),
                (None, None) => "map".into(),
            };
            let count = printed.entry(kind.clone()).or_default();
            *count += 1;
            if *count <= 2 {
                let mut app = App::new(Options::default());
                let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("a test terminal");
                terminal
                    .draw(|frame| super::super::screen::draw(frame, &app))
                    .expect("a frame");
                app.show(standing.clone());
                terminal
                    .draw(|frame| super::super::screen::draw(frame, &app))
                    .expect("a frame");
                let buffer = terminal.backend().buffer();
                let text = (0..buffer.area.height)
                    .map(|y| {
                        (0..buffer.area.width)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>()
                            .trim_end()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                println!("===== step {step}: {kind} #{count}\n{text}\n");
            }
            if seen.run_over || seen.moves.is_empty() {
                break;
            }
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let pick = &seen.moves[(x % seen.moves.len() as u64) as usize];
            let answered = sim.play(pick).expect("the simulator answers");
            assert!(answered.is_ok(), "the simulator refused a move it listed");
        }
    }
}
