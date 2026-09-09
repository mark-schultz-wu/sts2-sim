use sts2_core::rng::{GameRng, MegaRng};
use sts2_core::{Answer, Applied, CardIdent, CardOption, Carried, Question, Room, Rules};

use crate::engine::{Error, Sim};

impl<V: Rules> Sim<V> {
    pub(crate) fn choose(&mut self, index: u8) -> Result<Applied<V>, Error> {
        let current = &mut self.current;
        let Some(Room::Event { options, .. }) = current.observation.room.as_mut() else {
            return Err(Error::NotImplemented("choose_option"));
        };
        let option = options
            .get(index as usize)
            .ok_or(Error::NotImplemented("choose_option"))?;
        if option.relic != Some(V::lead_paperweight()) {
            return Err(Error::NotImplemented("this Neow option"));
        }

        options[index as usize].chosen = true;
        current.observation.relics.push(Carried {
            relic: V::lead_paperweight(),
            title: None,
            text: None,
        });

        let seed = self.reward_seed;
        let mut rng = MegaRng::named(seed, "rewards");
        let cards = crate::reward::card::regular::<V, _>(&mut rng, &V::colorless_cards(), 2);
        current
            .observation
            .rng
            .player
            .insert(V::rewards_rng(), rng.counter() as u32);
        current.asking = Some(Question::SelectCards {
            min: 0,
            max: 1,
            asker: None,
            options: cards
                .into_iter()
                .enumerate()
                .map(|(index, card)| CardOption {
                    index,
                    card: CardIdent::new(card),
                    text: None,
                })
                .collect(),
        });
        current.resolved = true;
        Ok(current.clone())
    }

    pub(crate) fn answer(&mut self, answer: &Answer) -> Result<Applied<V>, Error> {
        let current = &mut self.current;
        let Some(Question::SelectCards {
            min, max, options, ..
        }) = current.asking.as_ref()
        else {
            return Err(Error::NotImplemented("this answer"));
        };
        let Answer::Cards(indices) = answer else {
            return Err(Error::NotImplemented("this answer"));
        };
        if indices.len() < *min as usize || indices.len() > *max as usize {
            return Err(Error::NotImplemented("an invalid card selection"));
        }

        let cards: Option<Vec<_>> = indices
            .iter()
            .map(|&index| options.get(index).map(|option| option.card))
            .collect();
        let Some(cards) = cards else {
            return Err(Error::NotImplemented("an invalid card selection"));
        };
        current.observation.deck.extend(cards);
        current.asking = None;
        let Some(Room::Event {
            finished, options, ..
        }) = current.observation.room.as_mut()
        else {
            return Err(Error::NotImplemented("this card selection"));
        };
        *finished = true;
        options.clear();
        Ok(current.clone())
    }
}
