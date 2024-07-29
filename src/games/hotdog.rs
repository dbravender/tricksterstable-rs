/*
Game: Hotdog
Designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/365349/hotdog
*/

use std::collections::{BTreeMap, HashMap, HashSet};

use enum_iterator::{all, Sequence};
use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

const CARD_NONE: std::option::Option<Card> = None;
const NO_RELISH: i32 = 0;

/// All the possible bids in the game
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Sequence, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Bid {
    #[default]
    Pass = 0,
    Ketchup = 1,
    Mustard = 2,
    TheWorks = 3,
    KetchupFootlong = 4,
    MustardFootlong = 5,
    TheWorksFootlong = 6,
    NoPicker = 7,
}

/// Reader-friendly ranking of cards (Mustard -> LowStrong, Ketchup -> HighStrong, Works -> Alternating)
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
            Bid::Pass => unreachable!(),
        }
    }

    fn ranking(&self) -> Ranking {
        match self {
            Bid::NoPicker => Ranking::Alternating,
            Bid::Ketchup | Bid::KetchupFootlong => Ranking::HighStrong,
            Bid::Mustard | Bid::MustardFootlong => Ranking::LowStrong,
            Bid::TheWorks | Bid::TheWorksFootlong => Ranking::Alternating,
            Bid::Pass => unreachable!(),
        }
    }

    /// Higher bids can be made on top of lower bids
    fn next_bids(&self) -> Vec<Bid> {
        match self {
            Bid::NoPicker => unreachable!(),
            Bid::Pass => vec![
                Bid::Pass,
                Bid::Ketchup,
                Bid::Mustard,
                Bid::TheWorks,
                Bid::KetchupFootlong,
                Bid::MustardFootlong,
                Bid::TheWorksFootlong,
            ],
            Bid::Ketchup => vec![
                Bid::Pass,
                Bid::TheWorks,
                Bid::KetchupFootlong,
                Bid::MustardFootlong,
                Bid::TheWorksFootlong,
            ],
            Bid::Mustard => vec![
                Bid::Pass,
                Bid::TheWorks,
                Bid::KetchupFootlong,
                Bid::MustardFootlong,
                Bid::TheWorksFootlong,
            ],
            Bid::TheWorks => vec![
                Bid::Pass,
                Bid::KetchupFootlong,
                Bid::MustardFootlong,
                Bid::TheWorksFootlong,
            ],
            Bid::KetchupFootlong => vec![Bid::Pass, Bid::TheWorksFootlong],
            Bid::MustardFootlong => vec![Bid::Pass, Bid::TheWorksFootlong],
            Bid::TheWorksFootlong => vec![],
        }
    }

    fn next_state(&self) -> State {
        match self {
            Bid::Ketchup => State::NameTrump,
            Bid::Mustard => State::NameTrump,
            Bid::TheWorks => State::Bid,
            Bid::KetchupFootlong => State::NameTrump,
            Bid::MustardFootlong => State::NameTrump,
            Bid::TheWorksFootlong => State::Bid,
            Bid::Pass => State::Bid,
            Bid::NoPicker => unreachable!(),
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

static ID_TO_BID: Lazy<HashMap<i32, Bid>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for bid in all::<Bid>() {
        m.insert(bid as i32, bid);
    }
    m
});

static ID_TO_CARD: Lazy<HashMap<i32, Card>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for card in HotdogGame::deck() {
        m.insert(card.id as i32, card);
    }
    m
});

static ID_TO_SUIT: Lazy<HashMap<i32, Suit>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for suit in all::<Suit>() {
        m.insert(suit as i32, suit);
    }
    m
});

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    // Players are bidding to name rank, trump, and a special rank
    Bid,
    // Players select trump for the current bid (Mustard and Ketchup only)
    NameTrump,
    // Select a special rank which wins if both cards played in a trick are different suits
    NameRelish,
    // TODO: When playing with the works, the player who leads the first trick decides whether it is played with Ketchup or Mustard. From there, card ranking alternates with each trick.
    WorksSelectFirstTrickType,
    // Trick play
    Play,
}

#[derive(Debug, Clone, Default, Serialize, Sequence, Deserialize, PartialEq, Eq, Copy, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
struct Card {
    id: i32,
    suit: Suit,
    value: i32,
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
pub struct HotdogGame {
    // What current actions are allowed - see State
    pub state: State,
    // Which player is making a move now
    pub current_player: usize, // 0 - 1
    // Player who led the current trick
    pub lead_player: usize,
    // Cards each player has played in the current trick
    current_trick: [Option<Card>; 2],
    // Cards in each player's hand
    hands: [Vec<Card>; 2],
    // 5 cards that are face up covering the straw bottom at the start of a hand
    straw_top: [[Option<Card>; 5]; 2],
    // 5 cards that are face down covered by the straw top at the start of a hand
    straw_bottom: [[Option<Card>; 5]; 2],
    // Voids revealed when a player couldn't follow a lead card - only applies
    // to hand - not to straw piles - used to determine possible hands
    voids: [Vec<Suit>; 2],
    // Total number of tricks taken for the current hand
    tricks_taken: [i32; 2],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    no_changes: bool,
    // Each player's latest bid
    pub bids: [Option<Bid>; 2],
    // The bid the round is played with
    pub winning_bid: Bid,
    // The player who secured the bid
    pub picker: usize,
    // The special suit rank
    pub relish: i32,
    // Current trump suit
    pub trump: Option<Suit>,
    // Whether or not high wins the current trick
    pub high_wins: bool,
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
        self.tricks_taken = [0, 0];
        self.hands = [vec![], vec![]];
        self.state = State::Bid;
        self.current_player = self.dealer;
        self.dealer = (self.dealer + 1) % 2;
        self.voids = [vec![], vec![]];
        let mut cards = HotdogGame::deck();
        let deal_index = self.new_change();
        let straw_top_index = self.new_change();
        self.straw_bottom = [[CARD_NONE; 5], [CARD_NONE; 5]];
        self.winning_bid = Bid::NoPicker;
        self.picker = self.dealer;
        self.bids = [None, None];
        self.relish = 0;
        self.trump = None;
        for straw_index in 0..5 {
            for player in 0..2 as usize {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id as usize,
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
                        object_id: card.id as usize,
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

    pub fn deck() -> Vec<Card> {
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

    pub fn trick_winner(&self) -> usize {
        assert!(self.current_trick[0].is_some());
        assert!(self.current_trick[1].is_some());
        let cards = (
            self.current_trick[0].clone().unwrap(),
            self.current_trick[1].clone().unwrap(),
        );
        if cards.0.suit != cards.1.suit
            && (cards.0.value == self.relish || cards.1.value == self.relish)
        {
            if cards.0.value == self.relish && cards.1.value == self.relish {
                // If two special rank cards are played, the second card wins.
                return (self.lead_player + 1) % 2;
            }
            // If the trick includes two different suits, and one of the cards has the special rank,
            // the card with the special rank wins.
            return if cards.0.value == self.relish { 0 } else { 1 };
        }
        // In general, the highest-ranking card in the trump suit wins the trick
        // or, if no trumps were played, the highest-ranking card in the suit that was led.
        return if self.card_value(cards.0) > self.card_value(cards.1) {
            0
        } else {
            1
        };
    }

    fn card_value(&self, card: Card) -> i32 {
        let multiplier = if self.high_wins { 1 } else { -1 };
        if card.suit == self.current_trick[self.lead_player].as_ref().unwrap().suit {
            return (card.value + 50) * multiplier;
        }
        if self.trump.is_some() && card.suit == self.trump.unwrap() {
            return (card.value + 100) * multiplier;
        }
        return card.value * multiplier;
    }

    pub fn moves_to_string(&self) -> BTreeMap<i32, String> {
        let mut moves_strings = BTreeMap::new();

        match self.state {
            State::NameRelish => {
                for i in 0..=9 {
                    let relish = if i == 0 {
                        "no relish".to_string()
                    } else {
                        format!("{} as relish", i)
                    };
                    moves_strings.insert(i, format!("Select {}", relish));
                }
            }
            State::NameTrump => {
                for i in 0..=3 {
                    moves_strings
                        .insert(i, format!("Name Trump {:?}", ID_TO_SUIT.get(&i).unwrap()));
                }
            }
            State::Bid => {
                let other_player_bid = match self.bids[(self.current_player + 1) % 2] {
                    None => Bid::Pass,
                    Some(x) => x,
                };

                for bid in other_player_bid.next_bids() {
                    let bid_value = bid as i32;
                    moves_strings.insert(bid_value, format!("{:?}", bid));
                }
            }
            State::WorksSelectFirstTrickType => {
                moves_strings.insert(0, "Ketchup (high card wins)".to_string());
                moves_strings.insert(1, "Mustard (low card wins)".to_string());
            }
            State::Play => {
                let moves = self.get_moves();
                for action in moves {
                    moves_strings.insert(action, format!("{:?}", ID_TO_CARD[&action]));
                }
            }
        }
        moves_strings
    }

    pub fn get_moves(self: &HotdogGame) -> Vec<i32> {
        match self.state {
            State::NameRelish => (0..=9).collect(), // NO_RELISH is 0, 1-9 are card values
            State::NameTrump => (0..=3).collect(),  // 0-3 correspond to ID_TO_SUIT
            State::Bid => {
                let other_player_bid = match self.bids[(self.current_player + 1) % 2] {
                    None => Bid::Pass, // Pass in next_bids maps to opening bids
                    Some(x) => x,
                };

                return other_player_bid
                    .next_bids()
                    .iter()
                    .map(|f| *f as i32)
                    .collect();
            }
            State::WorksSelectFirstTrickType => (0..=2).collect(), // 0 - Ketchup, 1 - Mustard
            State::Play => self.playable_card_ids(),
        }
    }

    fn exposed_straw_bottoms(&self, player: usize) -> HashSet<Card> {
        let mut exposed_cards: HashSet<Card> = HashSet::new();
        for (i, card) in self.straw_bottom[player].iter().enumerate() {
            if card.is_none() {
                continue;
            }
            if self.straw_top[player][i].is_none() {
                exposed_cards.insert(card.clone().unwrap());
            }
        }
        return exposed_cards;
    }

    fn visible_straw(&self, player: usize) -> Vec<Card> {
        let mut visible: Vec<Card> = self.straw_top[player].iter().filter_map(|x| *x).collect();
        visible.extend(self.exposed_straw_bottoms(player));
        return visible;
    }

    pub fn playable_card_ids(&self) -> Vec<i32> {
        // Must follow
        let mut playable_cards = self.visible_straw(self.current_player);
        playable_cards.extend(self.hands[self.current_player].clone());

        if self.current_trick[self.lead_player].is_some() {
            let lead_suit = self.current_trick[self.lead_player].clone().unwrap().suit;
            let moves: Vec<i32> = playable_cards
                .iter()
                .filter(|c| c.suit == lead_suit)
                .map(|c| c.id)
                .collect();
            if !moves.is_empty() {
                return moves;
            }
        }
        return playable_cards.iter().map(|c| c.id).collect();
    }

    pub fn apply_move(&mut self, action: i32) {
        match self.state {
            State::NameTrump => {
                let suit = ID_TO_SUIT[&action];
                self.trump = Some(suit);
                self.current_player = (self.current_player + 1) % 2;
                // Whenever State::NameTrump is an option there's always
                // another bidding round - trump isn't named for the
                // TheWorksFootlong terminal bid state
                self.state = State::Bid;
            }
            State::Bid => {
                let other_player_bid = self.bids[(self.current_player + 1) % 2];
                let bid = ID_TO_BID[&action];
                self.bids[self.current_player] = Some(bid);
                if bid == Bid::TheWorksFootlong {
                    self.trump = None;
                    self.current_player = (self.current_player + 1) % 2;
                    self.state = State::NameRelish;
                    return;
                }
                if self.bids == [Some(Bid::Pass), Some(Bid::Pass)] {
                    // If both players pass, there is no Picker.
                    // The round is still played with The Works.
                    self.winning_bid = Bid::TheWorks;
                    self.picker = (self.current_player + 1) % 2;
                    // The dealer may select some Relish
                    self.current_player = self.picker;
                    self.state = State::NameRelish;
                    return;
                }
                if other_player_bid.is_some() && bid == Bid::Pass {
                    // Other player bid, current player passed
                    // Bidding is over, non-picker can name relish
                    self.state = State::NameRelish;
                    return;
                }
                self.state = bid.next_state();
                if self.state == State::Bid {
                    // Next player doesn't get to name trump (works)
                    // Works has no trump
                    self.trump = None;
                    // Relish selection goes to the non-picker
                    self.current_player = (self.current_player + 1) % 2;
                }
            }
            State::NameRelish => {
                self.picker = (self.current_player + 1) % 2;
                let next_player = self.picker;
                self.current_player = next_player;
                self.lead_player = next_player;

                // Check to see if we are in the state where there both players passed
                if self.bids == [Some(Bid::Pass), Some(Bid::Pass)] {
                    self.winning_bid = Bid::TheWorks;
                } else {
                    self.winning_bid = self.bids[self.picker].unwrap();
                }
                self.relish = action;
                if self.winning_bid.ranking() == Ranking::Alternating {
                    self.state = State::WorksSelectFirstTrickType;
                } else {
                    // After relish is selected trick play starts
                    self.state = State::Play;
                }
            }
            State::WorksSelectFirstTrickType => {
                // 0 - Ketchup
                // 1 - Mustard
                self.high_wins = action == 0;
                self.state = State::Play;
            }
            State::Play => {
                let card = ID_TO_CARD[&action];
                let lead_suit = match self.current_trick[self.lead_player] {
                    Some(lead_card) => Some(lead_card.suit),
                    None => None,
                };
                if let Some(index) =
                    self.straw_bottom[self.current_player]
                        .iter()
                        .position(|c| match c {
                            Some(c_inner) => c_inner.id == card.id,
                            None => false,
                        })
                {
                    // Card played was from straw_bottom
                    self.straw_bottom[self.current_player][index] = None;
                } else if let Some(index) =
                    self.straw_top[self.current_player]
                        .iter()
                        .position(|c| match c {
                            Some(c_inner) => c_inner.id == card.id,
                            None => false,
                        })
                {
                    // Card played was from straw_top
                    self.straw_top[self.current_player][index] = None;
                } else {
                    // Card played was from hand
                    self.hands[self.current_player].retain(|c| c.id != card.id);
                }
                self.add_change(
                    0,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: action as usize,
                        dest: Location::Play,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
                // TODO self.reorder_hand(self.current_player);
                self.current_trick[self.current_player] = Some(card);

                if lead_suit.is_some() {
                    if Some(card.suit) != lead_suit
                        && !self.voids[self.current_player].contains(&lead_suit.unwrap())
                    {
                        // Player has revealed a void
                        self.voids[self.current_player].push(lead_suit.unwrap());
                    }
                }

                self.current_player = (self.current_player + 1) % 2;
                // TODO self.hide_playable();

                if self.current_trick.iter().flatten().count() == 2 {
                    // end trick

                    let trick_winner = self.trick_winner();
                    self.lead_player = trick_winner;
                    self.current_player = trick_winner;

                    if self.winning_bid.ranking() == Ranking::Alternating {
                        self.high_wins = !self.high_wins;
                    }
                    // TODO animate tricks to winner

                    // TODO check if hand is over
                    // TODO check if game is over
                }
            }
        }
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
        let d = HotdogGame::deck();
        println!("{:?}", d);
        assert_eq!(d.len(), 36);
    }

    struct TrickWinnerTestCase {
        relish: i32,
        current_trick: [Option<Card>; 2],
        lead_player: usize,
        expected_winner: usize,
        high_wins: bool,
    }

    #[test]
    fn test_trick_winner() {
        let test_cases = [
            TrickWinnerTestCase {
                relish: 1,
                lead_player: 0,
                current_trick: [
                    Some(Card {
                        id: 0,
                        value: 1,
                        suit: Suit::Red,
                    }),
                    Some(Card {
                        id: 1,
                        value: 2,
                        suit: Suit::Red,
                    }),
                ],
                expected_winner: 1,
                high_wins: true,
            },
            TrickWinnerTestCase {
                relish: 1,
                lead_player: 0,
                current_trick: [
                    Some(Card {
                        id: 0,
                        value: 1,
                        suit: Suit::Red,
                    }),
                    Some(Card {
                        id: 1,
                        value: 2,
                        suit: Suit::Red,
                    }),
                ],
                high_wins: true,
                expected_winner: 1,
            },
        ];
        for test_case in test_cases {
            let mut game = HotdogGame::new();
            game.relish = test_case.relish;
            game.lead_player = test_case.lead_player;
            game.current_trick = test_case.current_trick;
            game.high_wins = test_case.high_wins;
            assert_eq!(game.trick_winner(), test_case.expected_winner);
        }
    }
}
