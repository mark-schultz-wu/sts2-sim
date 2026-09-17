//! The names a rule reaches a card keyword by.
//!
//! Hand-written beside the generated enum, the way `rest_site` is. The table
//! says which keywords exist. This says which of them the rules name.

use sts2_core::KeywordKinds;

use super::V0_107_1;
use super::generated::cards::CardKeyword;

impl KeywordKinds<V0_107_1> for CardKeyword {
    const INNATE: CardKeyword = CardKeyword::Innate;
}
