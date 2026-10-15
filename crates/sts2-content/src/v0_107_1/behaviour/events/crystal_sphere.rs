//! Crystal Sphere: gold or a Debt for a grid to divine, and rewards for what
//! the divinations uncover.

use nonmax::NonMaxU8;
use smallvec::SmallVec;
use sts2_core::rng::{GameRng, RngInteger as _, Span};
use sts2_core::{
    Answer, Cell, CustomReward, EventBehavior, EventView, FoundItem, HookEffect, HookEffects,
    Question, RewardRarity, Room, Tool,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};

type Number = <V as sts2_core::ContentVersion>::Number;

/// `UncoverFutureCost`'s base, which `CalculateVars` adds a draw to.
const COST: Number = 50;

/// The width and the height of the grid.
const SIDE: u8 = 11;

/// One thing the grid hides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    /// `CrystalSphereRelic`.
    Relic,
    /// `CrystalSpherePotion`, of a rarity.
    Potion(RewardRarity),
    /// `CrystalSphereCardReward`, of a rarity.
    Cards(RewardRarity),
    /// `CrystalSphereCurse`.
    Curse,
    /// `CrystalSphereGold`.
    Gold { big: bool },
}

/// `PopulateItems`, in the order it places them.
const ITEMS: [Item; 15] = [
    Item::Relic,
    Item::Potion(RewardRarity::Common),
    Item::Potion(RewardRarity::Common),
    Item::Potion(RewardRarity::Rare),
    Item::Cards(RewardRarity::Common),
    Item::Cards(RewardRarity::Uncommon),
    Item::Cards(RewardRarity::Rare),
    Item::Curse,
    Item::Gold { big: false },
    Item::Gold { big: false },
    Item::Gold { big: false },
    Item::Gold { big: false },
    Item::Gold { big: false },
    Item::Gold { big: true },
    Item::Gold { big: true },
];

impl Item {
    /// `Size`: how many cells it covers across, then down.
    fn size(self) -> (u8, u8) {
        match self {
            Item::Relic => (4, 4),
            Item::Potion(RewardRarity::Rare) => (2, 2),
            Item::Potion(_) => (1, 3),
            Item::Cards(_) | Item::Curse => (2, 2),
            Item::Gold { big: false } => (1, 1),
            Item::Gold { big: true } => (2, 1),
        }
    }

    /// The class the game names it by.
    fn class(self) -> &'static str {
        match self {
            Item::Relic => "CrystalSphereRelic",
            Item::Potion(_) => "CrystalSpherePotion",
            Item::Cards(_) => "CrystalSphereCardReward",
            Item::Curse => "CrystalSphereCurse",
            Item::Gold { .. } => "CrystalSphereGold",
        }
    }

    /// `ToReward`, which a curse has none of.
    fn reward(self) -> Option<CustomReward<V>> {
        Some(match self {
            Item::Relic => CustomReward::RolledRelic,
            Item::Potion(rarity) => CustomReward::PotionOf(rarity),
            // `CardRarityOddsType.Uniform` over the character's pool, which
            // rolls each card's upgrade, and which the pool's hooks reach.
            Item::Cards(rarity) => CustomReward::CardsOf {
                pool: None,
                rarity,
                count: 3,
                upgrade_roll: true,
                no_pool_modifications: false,
            },
            Item::Curse => return None,
            Item::Gold { big: false } => CustomReward::Gold(10),
            Item::Gold { big: true } => CustomReward::Gold(30),
        })
    }
}

/// `CrystalSphereMinigame`: the grid, where each item sits, and what the
/// divinations have uncovered.
#[derive(Debug, Clone)]
pub struct Grid {
    /// `IsHidden` for each cell, as the bit at `x * SIDE + y`.
    hidden: u128,
    /// Each item's top-left corner, in [`ITEMS`] order.
    at: [(u8, u8); ITEMS.len()],
    /// `DivinationCount`.
    left: u8,
    /// `CrystalSphereTool`, the one the last divination used.
    tool: Tool,
    /// `_revealed`: the items uncovered, in the order they were.
    revealed: SmallVec<[u8; ITEMS.len()]>,
}

fn bit(x: u8, y: u8) -> u128 {
    1 << (u32::from(x) * u32::from(SIDE) + u32::from(y))
}

/// One step either way along a row or a column, where that stays on the grid.
fn steps(at: u8) -> impl Iterator<Item = u8> {
    [at.checked_sub(1), at.checked_add(1)]
        .into_iter()
        .flatten()
        .filter(|&at| at < SIDE)
}

/// `GetHorizontalCells`: left of a cell, then right of it.
fn horizontal(x: u8, y: u8) -> impl Iterator<Item = (u8, u8)> {
    steps(x).map(move |x| (x, y))
}

/// `GetVerticalCells`: above a cell, then below it.
fn vertical(x: u8, y: u8) -> impl Iterator<Item = (u8, u8)> {
    steps(y).map(move |y| (x, y))
}

impl Grid {
    /// `new CrystalSphereMinigame`: the corners cleared, then every item
    /// placed off the event's stream.
    fn new<R: GameRng>(rng: &mut R, divinations: u8) -> Self {
        let mut grid = Grid {
            hidden: (1 << (u32::from(SIDE) * u32::from(SIDE))) - 1,
            at: [(0, 0); ITEMS.len()],
            left: divinations,
            tool: Tool::Big,
            revealed: SmallVec::new(),
        };
        // Each corner, and what lies beside it twice over. Nothing is placed
        // yet, so clearing uncovers nothing.
        let mut cleared: Vec<(u8, u8)> =
            vec![(0, 0), (SIDE - 1, 0), (SIDE - 1, SIDE - 1), (0, SIDE - 1)];
        for _ in 0..2 {
            let across: Vec<_> = cleared
                .iter()
                .flat_map(|&(x, y)| horizontal(x, y))
                .collect();
            let down: Vec<_> = cleared.iter().flat_map(|&(x, y)| vertical(x, y)).collect();
            cleared.extend(across);
            cleared.extend(down);
        }
        for (x, y) in cleared {
            grid.hidden &= !bit(x, y);
        }
        for (index, item) in ITEMS.iter().enumerate() {
            let (width, height) = item.size();
            let free: Vec<(u8, u8)> = (0..SIDE)
                .flat_map(|x| (0..SIDE).map(move |y| (x, y)))
                .filter(|&(x, y)| {
                    x + width <= SIDE
                        && y + height <= SIDE
                        && (x..x + width).all(|x| {
                            (y..y + height).all(|y| {
                                grid.hidden & bit(x, y) != 0 && grid.cover(x, y, index).is_none()
                            })
                        })
                })
                .collect();
            grid.at[index] = *rng
                .choose(&free)
                .unwrap_or_else(|_| unimplemented!("a Crystal Sphere with no room for {item:?}"));
        }
        grid
    }

    /// The item among the first `placed` that covers a cell.
    fn cover(&self, x: u8, y: u8, placed: usize) -> Option<usize> {
        (0..placed).find(|&index| {
            let (left, top) = self.at[index];
            let (width, height) = ITEMS[index].size();
            (left..left + width).contains(&x) && (top..top + height).contains(&y)
        })
    }

    /// Whether nothing hides any cell an item covers.
    fn uncovered(&self, index: usize) -> bool {
        let (left, top) = self.at[index];
        let (width, height) = ITEMS[index].size();
        (left..left + width).all(|x| (top..top + height).all(|y| self.hidden & bit(x, y) == 0))
    }

    /// `ClearCell`, and the item it uncovers the last of.
    fn clear(&mut self, x: u8, y: u8) -> Option<Item> {
        if self.hidden & bit(x, y) == 0 {
            return None;
        }
        self.hidden &= !bit(x, y);
        let index = self.cover(x, y, ITEMS.len())?;
        if !self.uncovered(index) {
            return None;
        }
        self.revealed
            .push(u8::try_from(index).expect("fifteen items fit a byte"));
        Some(ITEMS[index])
    }

    /// `CellClicked`: one divination, over the cell and, with the big tool,
    /// everything around it. The items it uncovers come back, in order.
    fn divine(&mut self, x: u8, y: u8, tool: Tool) -> SmallVec<[Item; 4]> {
        if x >= SIDE || y >= SIDE || self.hidden & bit(x, y) == 0 {
            unimplemented!("divining ({x}, {y}), which is off the grid or already clear");
        }
        self.tool = tool;
        self.left -= 1;
        let cells: SmallVec<[(u8, u8); 9]> = match tool {
            Tool::Small => smallvec::smallvec![(x, y)],
            // `GetAdjacentCells`: beside it, then its diagonals, then itself.
            Tool::Big => horizontal(x, y)
                .chain(vertical(x, y))
                .chain(steps(x).flat_map(|x| steps(y).map(move |y| (x, y))))
                .chain([(x, y)])
                .collect(),
        };
        cells
            .into_iter()
            .filter_map(|(x, y)| self.clear(x, y))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct State {
    /// `UncoverFutureCost`, with its draw added.
    cost: Number,
    /// The option taken.
    chosen: Option<NonMaxU8>,
    /// The minigame, once an option starts it.
    grid: Option<Grid>,
}

pub struct CrystalSphere;

impl EventBehavior<V> for CrystalSphere {
    type State = State;

    /// A hundred gold or more, past the first act, which the table of
    /// allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: one to forty-nine more on the cost, off the event's
    /// stream.
    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let span = Span::new(R::Int::ONE, num_traits::cast(COST).expect("fifty fits"))
            .expect("one below fifty");
        let drawn: Number = num_traits::cast(rng.next_int_range(span)).expect("a cost fits");
        State {
            cost: COST + drawn,
            chosen: None,
            grid: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::CrystalSphere,
            &[
                (Said::CrystalSphereUncoverFuture, false),
                (Said::CrystalSpherePaymentPlan, false),
            ],
            state.chosen,
        )
    }

    /// `UncoverFuture`: the cost, then three divinations. `PaymentPlan`: a
    /// Debt, then six. Either builds the grid off the event's stream.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let (paid, divinations) = match option {
            0 => (HookEffect::LoseGold(state.cost), 3),
            _ => (HookEffect::AddToDeck(Cards::Curse(CurseCard::Debt)), 6),
        };
        state.grid = Some(Grid::new(rng, divinations));
        smallvec::smallvec![paid, HookEffect::AskEvent]
    }

    fn question(&self, state: &State) -> Option<Question<V>> {
        let grid = state.grid.as_ref()?;
        Some(Question::Divine {
            left: u32::from(grid.left),
            width: SIDE,
            height: SIDE,
            tool: grid.tool,
            cleared: (0..SIDE)
                .flat_map(|x| (0..SIDE).map(move |y| (x, y)))
                .filter(|&(x, y)| grid.hidden & bit(x, y) == 0)
                .map(|(x, y)| Cell { x, y })
                .collect(),
            revealed: (0..ITEMS.len())
                .filter(|&index| grid.uncovered(index))
                .map(|index| {
                    let item = ITEMS[index];
                    let (x, y) = grid.at[index];
                    let (width, height) = item.size();
                    FoundItem {
                        item: item.class().into(),
                        good: item != Item::Curse,
                        x,
                        y,
                        width,
                        height,
                    }
                })
                .collect(),
        })
    }

    /// One divination. A curse it uncovers joins the deck as it does, which
    /// is `CrystalSphereCurse.RevealItem`. The last one offers what was
    /// found, and the event finishes once that screen closes.
    fn answer(&self, state: &mut State, answer: &Answer) -> HookEffects<V> {
        let Answer::Divine { x, y, tool } = *answer else {
            unimplemented!("a Crystal Sphere answered with {answer:?}")
        };
        let grid = state
            .grid
            .as_mut()
            .unwrap_or_else(|| unreachable!("a divination with no grid"));
        let mut effects: HookEffects<V> = grid
            .divine(x, y, tool)
            .into_iter()
            .filter(|&item| item == Item::Curse)
            .map(|_| HookEffect::AddToDeck(Cards::Curse(CurseCard::Doubt)))
            .collect();
        match grid.left {
            0 => effects.extend([HookEffect::OfferEventRewards, HookEffect::FinishEvent]),
            _ => effects.push(HookEffect::AskEvent),
        }
        effects
    }

    /// `ToReward` for each item uncovered, in the order it was.
    fn rewards(&self, state: &State) -> Vec<CustomReward<V>> {
        let grid = state
            .grid
            .as_ref()
            .unwrap_or_else(|| unreachable!("rewards with no grid"));
        grid.revealed
            .iter()
            .filter_map(|&index| ITEMS[usize::from(index)].reward())
            .collect()
    }
}
