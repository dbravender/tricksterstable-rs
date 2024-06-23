/*
Game: Hotdog
Designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/365349/hotdog
*/

use enum_iterator::{all, Sequence};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

const CARD_NONE: std::option::Option<Card> = None;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Bid {
    #[default]
    NoPicker,
    Ketchup,
    Mustard,
    TheWorks,
    KetchupFootlong,
    MustardFootlong,
    TheWorksFootlong,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Ranking {
    #[default]
    HighStrong,
    LowStrong,
    Alternating,
}

impl Bid {
    fn required_tricks(&self) -> i32 {
        match self {
            // If both players passed on picking the toppings (i.e., there was no Picker),
            // whichever player wins 9 tricks or more earns 1 game point.
            Bid::NoPicker => 9,
            // The Picker must capture at least 9 tricks.
            Bid::Ketchup | Bid::Mustard | Bid::TheWorks => 9,
            // (Footlong option) The Picker must capture at least 12 tricks.
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => 12,
        }
    }

    fn ranking(&self) -> Ranking {
        match self {
            Bid::NoPicker => Ranking::Alternating,
            Bid::Ketchup | Bid::KetchupFootlong => Ranking::HighStrong,
            Bid::Mustard | Bid::MustardFootlong => Ranking::LowStrong,
            Bid::TheWorks | Bid::TheWorksFootlong => Ranking::Alternating,
        }
    }

    fn order(&self) -> i32 {
        match self {
            Bid::NoPicker => -1,
            Bid::Ketchup => 0,
            Bid::Mustard => 1,
            Bid::TheWorks => 2,
            Bid::KetchupFootlong => 3,
            Bid::MustardFootlong => 4,
            Bid::TheWorksFootlong => 5,
        }
    }

    fn points_for_setter(&self, tricks_taken: i32) -> i32 {
        match self {
            // Footlong Option
            // However, if the Picker fails to capture at least 12 tricks, the opponent automatically wins the game.
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => 5,
            _ => {
                if tricks_taken >= 12 {
                    5
                } else {
                    2
                }
            }
        }
    }

    ///
    fn points_for_picker_success(&self, tricks_taken: i32) -> i32 {
        match self {
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => {
                if tricks_taken >= 15 {
                    5
                } else {
                    3
                }
            }
            _ => {
                if tricks_taken >= 15 {
                    5
                } else if tricks_taken >= 12 {
                    2
                } else {
                    1
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum State {
    #[default]
    // Players are bidding to name rank, trump, and a special rank
    Bid,
    // Trick play
    Play,
}

#[derive(Debug, Clone, Default, Serialize, Sequence, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "camelCase")]
enum Suit {
    #[default]
    Red,
    Green,
    Blue,
    Yellow,
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
    StrawTop,
    StrawBottom,
    ReorderHand,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Card {
    id: usize,
    suit: Suit,
    value: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Faceup,
    Trump,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    RevealCard,
    Discard,
    Reorder,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    #[serde(rename(serialize = "id", deserialize = "id"))]
    object_id: usize,
    dest: Location,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    offset: usize,
    player: usize,
    length: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HotdogGame {
    // What current actions are allowed - see State
    state: State,
    // Which player is making a move now
    current_player: usize, // 0 - 1
    // Cards each player has played in the current trick
    current_trick: [Option<Card>; 2],
    // Cards in each player's hand
    hands: [Vec<Card>; 2],
    // 5 cards that are face up covering the straw bottom at the start of a hand
    straw_top: [[Option<Card>; 5]; 2],
    // 5 cards that are face down covered by the straw top at the start of a hand
    straw_bottom: [[Option<Card>; 5]; 2],
    lead_suit: Option<Card>,
    // Voids revealed when a player couldn't follow a lead card - only applies
    // to hand - not to straw piles - used to determine possible hands
    voids: [Vec<Suit>; 2],
    // Total number of tricks taken for the current hand
    tricks_taken: [i32; 2],
    // Player who starts the next hand
    lead_player: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    no_changes: bool,
}

impl HotdogGame {
    pub fn new() -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        game.deal();
        game
    }

    // Called at the start of a game and when a new hand is dealt
    pub fn deal(&mut self) {
        self.lead_suit = None;
        self.tricks_taken = [0, 0];
        self.hands = [vec![], vec![]];
        self.state = State::Bid;
        self.current_player = self.lead_player;
        self.lead_player = (self.lead_player + 1) % 2;
        self.voids = [vec![], vec![]];
        let mut cards = self.deck();
        let deal_index = self.new_change();
        let straw_top_index = self.new_change();
        self.straw_bottom = [[CARD_NONE; 5], [CARD_NONE; 5]];
        for straw_index in 0..5 {
            for player in 0..2 as usize {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::StrawBottom,
                        player,
                        offset: straw_index,
                        length: 5,
                        ..Default::default()
                    },
                );
                self.straw_bottom[player][straw_index] = Some(card);
            }
        }
        // End dealing with sevens
        self.straw_top = [[CARD_NONE; 5], [CARD_NONE; 5]];
        for straw_index in 0..5 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.add_change(
                    straw_top_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::StrawTop,
                        player,
                        offset: straw_index,
                        length: 5,
                        ..Default::default()
                    },
                );
                self.straw_top[player][straw_index] = Some(card);
            }
        }
    }

    pub fn deck(&self) -> Vec<Card> {
        let mut deck: Vec<Card> = vec![];
        let mut id = 0;
        for suit in all::<Suit>() {
            for value in 1..=9 {
                deck.push(Card {
                    id,
                    value: value,
                    suit,
                });
                id += 1;
            }
        }
        deck.shuffle(&mut thread_rng());
        deck
    }

    fn start_hand() {
        // The Picker leads to the first trick.
        // If both players pass, the non-dealer leads the first trick.
    }

    fn trick_winner() {
        // In general, the highest-ranking card in the trump suit wins the trick
        // or, if no trumps were played, the highest-ranking card in the suit that was led. However, if the trick includes two different suits, and one of the cards has the special rank, the card with the special rank wins.
        // If two special rank cards are played, the second card wins.
    }

    #[inline]
    fn new_change(&mut self) -> usize {
        self.changes.push(vec![]);
        self.changes.len() - 1
    }

    #[inline]
    fn add_change(&mut self, index: usize, change: Change) {
        if !self.no_changes {
            self.changes[index].push(change);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck() {
        let g = HotdogGame::new();
        let d = g.deck();
        println!("{:?}", d);
        assert_eq!(d.len(), 36);
    }
}
