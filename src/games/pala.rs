/*
Game: Pala
Designer: Jeffrey Allers
BoardGameGeek: https://boardgamegeek.com/boardgame/37441/pala
*/

use enum_iterator::{all, Sequence};
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Default,
    Serialize,
    Sequence,
    Deserialize,
    PartialEq,
    Eq,
    Copy,
    Hash,
    PartialOrd,
    Ord,
)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
    #[default]
    Blue = 0,
    Red = 1,
    Yellow = 2,
    Green = 3,
    Purple = 4,
    Orange = 5,
}

impl Suit {
    pub fn is_primary(&self) -> bool {
        match self {
            Suit::Blue | Suit::Red | Suit::Yellow => true,
            _ => false,
        }
    }

    pub fn is_secondary(&self) -> bool {
        !self.is_primary()
    }

    pub fn composed_of(&self) -> [Suit; 2] {
        match self {
            Suit::Green => [Suit::Blue, Suit::Yellow],
            Suit::Purple => [Suit::Red, Suit::Blue],
            Suit::Orange => [Suit::Yellow, Suit::Red],
            _ => panic!("primary colors are not composed of other colors"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    pub suit: Suit,
    value: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    // Select a card from hand to play to bid (or pass)
    BidSelectBidCard,
    // Select a bid location
    BidSelectBidLocation,
    // Select a card to play (as a smear, mix, follow, or junk)
    SelectCardToPlay,
    // Select location to play card
    SelectLocationToPlay,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    TricksTaken,
    Score,
    ReorderHand,
    Message,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Faceup,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    Reorder,
    Message,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    object_id: i32,
    dest: Location,
    startscore: i32,
    end_score: i32,
    offset: usize,
    player: usize,
    length: usize,
    highlight: bool,
    message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PalaGame {
    // Current game state
    pub state: State,
    // Which player is making a move now
    pub current_player: usize, // 0 - 3
    // Player who led the current trick
    pub lead_player: usize,
    // Cards each player has played in the current trick
    pub current_trick: [Option<Card>; 4],
    // Cards in each player's hand
    pub hands: [Vec<Card>; 4],
    // Voids revealed when a player couldn't follow a lead card (used during determination)
    pub voids: [Vec<Suit>; 4],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    pub changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    pub no_changes: bool,
    // Current score of the game
    pub scores: [i32; 4],
    // Game winner
    pub winner: Option<usize>,
    // Use experimental reward function for comparison
    pub experiment: bool,
    // Which player is the human player
    pub human_player: Option<usize>,
    // Cards selected as the torch card
    pub bids: [Option<Card>; 4],
}

impl PalaGame {
    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        for suit in all::<Suit>() {
            let values = if suit.is_primary() {
                [1, 1, 2, 2, 3, 3, 4, 5]
            } else {
                [2, 3, 4, 5, 6, 7, 8, 9]
            };
            for value in values {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use enum_iterator::all;
    use std::collections::HashMap;

    #[test]
    fn test_deck_composition() {
        let deck = PalaGame::deck();
        // Total card count
        assert_eq!(deck.len(), 48);

        // Group values by suit
        let mut map: HashMap<Suit, Vec<i32>> = HashMap::new();
        for card in deck {
            map.entry(card.suit).or_default().push(card.value);
        }

        for suit in all::<Suit>() {
            let values = map
                .get(&suit)
                .expect(&format!("No cards for suit {:?}", suit));
            // 8 cards per suit
            assert_eq!(values.len(), 8, "wrong count for {:?}", suit);

            // Sort and compare against expected multiset
            let mut got = values.clone();
            got.sort_unstable();
            let mut want = if suit.is_primary() {
                vec![1, 1, 2, 2, 3, 3, 4, 5]
            } else {
                vec![2, 3, 4, 5, 6, 7, 8, 9]
            };
            want.sort_unstable();
            assert_eq!(got, want, "bad values for {:?}", suit);
        }
    }
}
