/*
Game: Hotdog
Designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/365349/hotdog
*/

use rand::Rng;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
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
    fn description(&self) -> String {
        match self {
            Bid::Ketchup => "Ketchup (↑) (9/12)".to_string(),
            Bid::Mustard => "Mustard (↓) (9/12)".to_string(),
            Bid::TheWorks => "The Works (↓↑↓...) (9/12)".to_string(),

            Bid::KetchupFootlong => "Ketchup Footlong (↑) (12)".to_string(),
            Bid::MustardFootlong => "Mustard Footlong (↓) (12)".to_string(),
            Bid::TheWorksFootlong => "The Works Footlong (↓↑↓...) (12)".to_string(),
            Bid::Pass => "Pass".to_string(),
        }
    }

    fn required_tricks(&self) -> i32 {
        match self {
            // The Picker must capture at least 9 tricks.
            Bid::Ketchup | Bid::Mustard | Bid::TheWorks => 9,
            // (Footlong option) The Picker must capture at least 12 tricks.
            Bid::KetchupFootlong | Bid::MustardFootlong | Bid::TheWorksFootlong => 12,
            Bid::Pass => unreachable!(),
        }
    }

    fn ranking(&self) -> Ranking {
        match self {
            Bid::Ketchup | Bid::KetchupFootlong => Ranking::HighStrong,
            Bid::Mustard | Bid::MustardFootlong => Ranking::LowStrong,
            Bid::TheWorks | Bid::TheWorksFootlong => Ranking::Alternating,
            Bid::Pass => unreachable!(),
        }
    }

    /// Higher bids can be made on top of lower bids
    fn next_bids(&self) -> Vec<Bid> {
        match self {
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
    // When playing with the works, the player who leads the first trick decides whether it is played with Ketchup or Mustard. From there, card ranking alternates with each trick.
    WorksSelectFirstTrickType,
    // Trick play
    Play,
}

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
    Red = 0,
    Green = 1,
    Blue = 2,
    Yellow = 3,
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
    // Throw card offscreen
    Burn,
    // Trump display
    Trump,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
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
    BidOptions,    // system sends bid options to be displayed in a dialog
    HighOrLowWins, // display if high cards or low cards win this trick
    BidSummary,    // display last bid or named relish for each player
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
    bid_options: Option<Vec<BidOption>>,
    high_wins: Option<bool>,
    bid_title: Option<String>,
    bid_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
struct BidOption {
    id: i32,
    description: String,
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
    pub hands: [Vec<Card>; 2],
    // 5 cards that are face up covering the straw bottom at the start of a hand
    pub straw_top: [[Option<Card>; 5]; 2],
    // 5 cards that are face down covered by the straw top at the start of a hand
    pub straw_bottom: [[Option<Card>; 5]; 2],
    // Voids revealed when a player couldn't follow a lead card - only applies
    // to hand - not to straw piles - used to determine possible hands
    pub voids: [Vec<Suit>; 2],
    // Total number of tricks taken for the current hand
    pub tricks_taken: [i32; 2],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    pub changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    pub no_changes: bool,
    // Each player's latest bid
    pub bids: [Option<Bid>; 2],
    // The bid the round is played with
    pub winning_bid: Bid,
    // The player who secured the bid
    pub picker: Option<usize>,
    // The special suit rank
    pub relish: i32,
    // Current trump suit
    pub trump: Option<Suit>,
    // Whether or not high wins the current trick
    pub high_wins: Option<bool>,
    // Current score of the game
    pub scores: [i32; 2],
    // Game winner
    pub winner: Option<usize>,
    // Use experimental reward function for comparison
    pub experiment: bool,
    // Undealt cards
    pub cards: Vec<Card>,
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
        self.picker = None;
        self.tricks_taken = [0, 0];
        self.high_wins = None;
        self.hands = [vec![], vec![]];
        self.state = State::Bid;
        self.current_player = self.dealer;
        self.current_trick = [None; 2];
        self.dealer = (self.dealer + 1) % 2;
        self.voids = [vec![], vec![]];
        let mut cards = HotdogGame::deck();
        let shuffle_index = self.new_change();
        let deal_index = self.new_change();
        let straw_top_index = self.new_change();
        self.straw_bottom = [[CARD_NONE; 5], [CARD_NONE; 5]];
        self.winning_bid = Bid::Pass;
        self.picker = None;
        self.bids = [None, None];
        self.relish = 0;
        self.trump = None;
        // Clear bid summaries
        self.add_bid_summary(0, "".to_string());
        self.add_bid_summary(1, "".to_string());
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::HighOrLowWins,
                high_wins: None,
                ..Default::default()
            },
        );
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Shuffle,
                object_id: 0,
                dest: Location::Deck,
                ..Default::default()
            },
        );
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
        for hand_index in 0..7 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.add_change(
                    straw_top_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id as usize,
                        dest: Location::Hand,
                        player,
                        offset: hand_index,
                        length: 7,
                        ..Default::default()
                    },
                );
                self.hands[player].push(card);
            }
        }
        for card in cards.iter_mut() {
            self.add_change(
                straw_top_index,
                Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id as usize,
                    dest: Location::Burn,
                    ..Default::default()
                },
            );
        }
        self.hands[0].sort_by(card_sorter);
        self.reorder_hand(0, true);
        self.bid_phase_changes();
        self.cards = cards;
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

        // Both players played the same suit - player who played highest or lowest card wins
        if cards.0.suit == cards.1.suit {
            if self.high_wins.unwrap() {
                return if cards.0.value > cards.1.value { 0 } else { 1 };
            }
            return if cards.0.value < cards.1.value { 0 } else { 1 };
        }
        // Only one player played trump - that player wins the trick
        if self.trump.is_some() {
            let trump = self.trump.unwrap();
            if cards.0.suit == trump {
                return 0;
            }
            if cards.1.suit == trump {
                return 1;
            }
        }
        // Both players played a different suit, no one played trump or relish, lead player
        // takes the trick
        return self.lead_player;
    }

    pub fn moves_to_string(&self) -> BTreeMap<i32, String> {
        let mut moves_strings = BTreeMap::new();

        match self.state {
            State::NameRelish => {
                for i in 0..=9 {
                    let relish = if i == NO_RELISH {
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
                for move_id in self.get_moves() {
                    let bid: Bid = ID_TO_BID[&move_id];
                    moves_strings.insert(move_id, bid.description());
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

                let mut bids: Vec<i32> = other_player_bid
                    .next_bids()
                    .iter()
                    .map(|f| *f as i32)
                    .collect();

                if false {
                    //self.no_changes {
                    let mut rnd = thread_rng();
                    if rnd.gen_range(0..100) > 20 {
                        bids.retain(|b| b < &4)
                    }
                }

                bids
            }
            State::WorksSelectFirstTrickType => (0..2).collect(), // 0 - Ketchup, 1 - Mustard
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

    pub fn playable_cards(&self) -> [Vec<Card>; 2] {
        let mut cards = [self.visible_straw(0), self.visible_straw(1)];

        for player in 0..2 {
            cards[player].extend(self.hands[player].iter().cloned());
        }

        cards
    }

    pub fn playable_card_ids(&self) -> Vec<i32> {
        // Must follow
        let playable_cards = &self.playable_cards()[self.current_player];
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

    pub fn bid_phase_changes(&mut self) {
        if self.no_changes || self.current_player == 1 {
            return;
        }
        let proceed: bool = match self.state {
            State::Bid
            | State::WorksSelectFirstTrickType
            | State::NameRelish
            | State::NameTrump => true,
            _ => false,
        };
        if !proceed {
            return;
        }
        let bid_title = Some(match self.state {
            State::Bid => "Select a bid".to_string(),
            State::WorksSelectFirstTrickType => "Select start ".to_string(),
            State::NameRelish => "Select relish (special rank)".to_string(),
            State::NameTrump => "Select trump".to_string(),
            _ => unreachable!(),
        });
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::BidOptions,
                player: self.current_player,
                bid_title,
                bid_options: Some(self.bid_options()),
                ..Default::default()
            },
        );
    }

    fn apply_move_internal(&mut self, action: i32) {
        match self.state {
            State::NameTrump => {
                let suit = ID_TO_SUIT[&action];
                let index = self.new_change();
                self.add_change(
                    index,
                    Change {
                        change_type: ChangeType::Trump,
                        object_id: (action + 1000) as usize,
                        ..Default::default()
                    },
                );
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
                self.add_bid_summary(self.current_player, bid.description());
                if bid == Bid::TheWorksFootlong {
                    if !self.no_changes {
                        // println!(
                        //     ">> Bid::TheWorksFootlong << player: {}",
                        //     self.current_player
                        // );
                    }
                    self.trump = None;
                    self.hide_trump();
                    self.current_player = (self.current_player + 1) % 2;
                    self.state = State::NameRelish;

                    return;
                }
                if self.bids == [Some(Bid::Pass), Some(Bid::Pass)] {
                    // If both players pass, there is no Picker.
                    // The round is still played with The Works.
                    self.winning_bid = Bid::TheWorks;
                    self.add_bid_summary(0, "The Works (no picker)".to_string());
                    self.add_bid_summary(1, "The Works (no picker)".to_string());
                    // The dealer may select some Relish
                    self.current_player = self.dealer;
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
                    self.hide_trump();
                    // Relish selection goes to the non-picker
                    self.current_player = (self.current_player + 1) % 2;
                }
            }
            State::NameRelish => {
                let next_player = (self.current_player + 1) % 2;
                self.picker = Some(next_player);
                self.current_player = next_player;
                self.lead_player = next_player;

                // Check to see if we are in the state where there both players passed
                if self.bids == [Some(Bid::Pass), Some(Bid::Pass)] {
                    self.winning_bid = Bid::TheWorks;
                } else {
                    if !self.no_changes {
                        // println!("self.bids: {:?} self.picker: {:?}", self.bids, self.picker);
                    }
                    self.winning_bid = self.bids[self.picker.unwrap()].unwrap();
                    self.high_wins = match self.winning_bid.ranking() {
                        Ranking::HighStrong => Some(true),
                        Ranking::LowStrong => Some(false),
                        Ranking::Alternating => None,
                    };
                    self.add_change(
                        self.changes.len() - 1,
                        Change {
                            change_type: ChangeType::HighOrLowWins,
                            high_wins: self.high_wins,
                            ..Default::default()
                        },
                    );
                }
                self.relish = action;
                if self.relish == 0 {
                    self.add_bid_summary((self.current_player + 1) % 2, "No relish".to_string());
                } else {
                    self.add_bid_summary(
                        (self.current_player + 1) % 2,
                        format!("Named {} as relish", &action),
                    );
                }
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
                self.high_wins = Some(action == 0);
                self.add_change(
                    self.changes.len() - 1,
                    Change {
                        change_type: ChangeType::HighOrLowWins,
                        high_wins: self.high_wins,
                        ..Default::default()
                    },
                );
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

                self.reorder_hand(self.current_player, false);

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
                self.hide_playable();

                if self.current_trick.iter().flatten().count() == 2 {
                    // End trick

                    let trick_winner = self.trick_winner();
                    self.lead_player = trick_winner;
                    self.current_player = trick_winner;
                    self.tricks_taken[trick_winner] += 1;

                    self.reveal_straw_bottoms(0);
                    self.reveal_straw_bottoms(1);

                    let index = self.new_change();
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::ShowWinningCard,
                            object_id: self.current_trick[trick_winner].unwrap().id as usize,
                            dest: Location::Play,
                            ..Default::default()
                        },
                    );
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::OptionalPause,
                            object_id: 0,
                            dest: Location::Play,
                            ..Default::default()
                        },
                    );

                    if self.winning_bid.ranking() == Ranking::Alternating {
                        self.high_wins = Some(!self.high_wins.unwrap());
                        self.add_change(
                            self.changes.len() - 1,
                            Change {
                                change_type: ChangeType::HighOrLowWins,
                                high_wins: self.high_wins,
                                ..Default::default()
                            },
                        );
                    }

                    // Animate tricks to winner
                    let change_index = self.new_change();
                    for card in self.current_trick {
                        self.add_change(
                            change_index,
                            Change {
                                change_type: ChangeType::TricksToWinner,
                                object_id: card.unwrap().id as usize,
                                dest: Location::TricksTaken,
                                player: trick_winner,
                                tricks_taken: self.tricks_taken[trick_winner],
                                ..Default::default()
                            },
                        );
                    }

                    // Clear trick
                    self.current_trick = [None; 2];

                    if self.playable_cards().iter().all(|x| x.is_empty()) {
                        // The hand is over

                        let picker = if self.picker.is_some() {
                            self.picker.unwrap()
                        } else {
                            if self.tricks_taken[0] > self.tricks_taken[1] {
                                0
                            } else {
                                1
                            }
                        };

                        if !self.no_changes {
                            // println!(
                            //     "tricks taken: {:?} bid: {:?} picker: {:?}",
                            //     self.tricks_taken, self.winning_bid, picker,
                            // );
                        }

                        let tricks_taken_by_picker = self.tricks_taken[picker];
                        if tricks_taken_by_picker >= self.winning_bid.required_tricks() {
                            self.scores[picker] += self
                                .winning_bid
                                .points_for_picker_success(tricks_taken_by_picker);

                            let change_index = self.new_change();
                            self.add_change(
                                change_index,
                                Change {
                                    change_type: ChangeType::Score,
                                    dest: Location::Score,
                                    player: picker,
                                    end_score: self.scores[picker],
                                    ..Default::default()
                                },
                            );
                        } else {
                            let setter = (picker + 1) % 2;
                            self.scores[setter] += self
                                .winning_bid
                                .points_for_setter(self.tricks_taken[setter]);
                            let change_index = self.new_change();
                            self.add_change(
                                change_index,
                                Change {
                                    change_type: ChangeType::Score,
                                    dest: Location::Score,
                                    player: setter,
                                    end_score: self.scores[setter],
                                    ..Default::default()
                                },
                            );
                        }

                        // Check if the game is over
                        for player in 0..2 {
                            if self.scores[player] >= 5 {
                                self.winner = Some(player);
                                let change_index = self.new_change();
                                self.add_change(
                                    change_index,
                                    Change {
                                        change_type: ChangeType::GameOver,
                                        object_id: 0,
                                        dest: Location::Deck,
                                        ..Default::default()
                                    },
                                );
                                return;
                            }
                        }
                        self.deal();
                        return;
                    }
                }
            }
        }
    }

    pub fn apply_move(&mut self, action: i32) {
        self.changes = vec![vec![]]; // card from player to table
        if !self.get_moves().contains(&action) {
            // return the same game with no animations when an invalid move is made
            return;
        }
        self.apply_move_internal(action);
        self.bid_phase_changes();
        self.show_playable();
    }

    #[inline]
    fn new_change(&mut self) -> usize {
        self.changes.push(vec![]);
        self.changes.len() - 1
    }

    #[inline]
    fn add_change(&mut self, index: usize, change: Change) {
        if self.no_changes {
            return;
        }
        self.changes[index].push(change);
    }

    #[inline]
    fn add_bid_summary(&mut self, player: usize, bid_summary: String) {
        if self.no_changes {
            return;
        }
        self.add_change(
            self.changes.len() - 1,
            Change {
                change_type: ChangeType::BidSummary,
                player,
                bid_summary: Some(bid_summary),
                ..Default::default()
            },
        );
    }

    #[inline]
    pub fn reorder_hand(&mut self, player: usize, force_new_animation: bool) {
        if self.no_changes {
            return;
        }
        if self.changes.is_empty() || force_new_animation {
            self.new_change();
        }
        let length = self.hands[player].len();
        let index = self.changes.len() - 1;
        self.changes[index].extend(self.hands[player].iter().enumerate().map(|(offset, card)| {
            Change {
                change_type: ChangeType::Reorder,
                dest: Location::ReorderHand,
                object_id: card.id as usize,
                player,
                offset,
                length,
                ..Default::default()
            }
        }));
    }

    fn hide_trump(&mut self) {
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Trump,
                object_id: 999 as usize,
                ..Default::default()
            },
        )
    }

    fn show_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.new_change();
        if self.current_player == 0 && self.state == State::Play {
            let moves = self.get_moves();
            for id in moves {
                self.add_change(
                    change_index,
                    Change {
                        object_id: id as usize,
                        change_type: ChangeType::ShowPlayable,
                        dest: Location::Hand,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
            }
        } else {
            self.hide_playable();
        }
    }

    fn hide_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.changes.len() - 1;
        let mut cards = self.hands[0].clone();
        cards.extend(self.exposed_straw_bottoms(0));
        cards.extend(self.straw_top[0].iter().flatten());
        for card in cards {
            self.add_change(
                change_index,
                Change {
                    object_id: card.id as usize,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    player: self.current_player,
                    ..Default::default()
                },
            );
        }
    }

    pub fn reveal_straw_bottoms(&mut self, player: usize) {
        if self.no_changes {
            return;
        }
        let index = self.new_change();
        let exposed_straw_bottoms = self.exposed_straw_bottoms(player);
        let sorted_straw_bottoms: Vec<Card> = exposed_straw_bottoms.iter().cloned().collect();
        //sorted_straw_bottoms.sort_by_key(|c| c.id); - needed to verify against Dart engine
        self.changes[index].extend(sorted_straw_bottoms.iter().map(|c| Change {
            change_type: ChangeType::RevealCard,
            dest: Location::Hand,
            object_id: c.id as usize,
            ..Default::default()
        }));
    }

    fn bid_options(&self) -> Vec<BidOption> {
        self.moves_to_string()
            .into_iter()
            .map(|bid_option| BidOption {
                id: bid_option.0,
                description: bid_option.1,
            })
            .collect()
    }
}

impl ismcts::Game for HotdogGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();
        let mut remaining_cards: Vec<Card> = self.cards.clone();
        let mut hidden_straw_bottoms: [HashSet<Card>; 2] = [HashSet::new(), HashSet::new()];

        for player in 0..2 {
            if player != self.current_player {
                remaining_cards.extend(self.hands[player].iter());
            }

            hidden_straw_bottoms[player] =
                HashSet::from_iter(self.straw_bottom[player].iter().filter_map(|&x| x))
                    .difference(&self.exposed_straw_bottoms(player))
                    .cloned()
                    .collect();

            remaining_cards.extend(hidden_straw_bottoms[player].iter());
        }

        remaining_cards.shuffle(rng);

        for player in 0..2 {
            let original_hand_length: usize = self.hands[player].len();
            if player != self.current_player {
                let mut pc = extract_short_suited_cards(&remaining_cards, &self.voids[player]);
                self.hands[player] = vec![];
                pc.cards.shuffle(rng);
                for _ in 0..original_hand_length {
                    let card = pc.cards.pop().unwrap();
                    self.hands[player].push(card);
                }
                remaining_cards = pc.leftovers;
                remaining_cards.extend(pc.cards);
            }
            assert!(original_hand_length == self.hands[player].len());
        }

        remaining_cards.shuffle(rng);
        for player in 0..2 {
            for i in 0..self.straw_bottom[player].len() {
                let card = self.straw_bottom[player][i];
                if !card.is_none() && hidden_straw_bottoms[player].contains(&card.unwrap()) {
                    self.straw_bottom[player][i] = remaining_cards.pop();
                }
            }
        }
        assert!(remaining_cards.len() == 2);
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player + 1) % 2
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.apply_move(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if let Some(winner) = self.winner {
            // someone won the game
            if winner == player {
                Some(1.0)
            } else {
                Some(0.0)
            }
        } else {
            if self.scores == [0, 0] {
                // the hand is not over
                None
            } else {
                let current_player_score = self.scores[player] as f64;
                let other_player_score = self.scores[(player + 1) % 2] as f64;
                if current_player_score > other_player_score {
                    Some(0.8 + ((current_player_score / 5.0) * 0.2))
                } else {
                    Some(0.2 - ((other_player_score / 5.0) * 0.2))
                }
            }
        }
    }
}
pub struct PossibleCards {
    cards: Vec<Card>,
    leftovers: Vec<Card>,
}

pub fn extract_short_suited_cards(remaining_cards: &Vec<Card>, voids: &Vec<Suit>) -> PossibleCards {
    let mut leftovers: Vec<Card> = vec![];

    let mut possible_cards = remaining_cards.clone();

    for suit in voids {
        possible_cards.retain(|card| {
            let belongs_to_suit = card.suit == *suit;
            if belongs_to_suit {
                leftovers.push(*card);
            }
            !belongs_to_suit
        });
    }
    return PossibleCards {
        cards: possible_cards,
        leftovers,
    };
}

pub fn get_mcts_move(game: &HotdogGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    // reset scores for the simulation
    new_game.scores = [0; 2];
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 8;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    if debug {
        // println!("-------");
        // ismcts.debug_children();
        // println!("-------");
    }
    ismcts.best_move().expect("should have a move to make")
}

fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck() {
        let d = HotdogGame::deck();
        assert_eq!(d.len(), 36);
    }

    #[derive(Debug)]
    struct TrickWinnerTestCase {
        relish: i32,
        current_trick: [Option<Card>; 2],
        lead_player: usize,
        expected_winner: usize,
        high_wins: bool,
        trump: Option<Suit>,
    }

    #[test]
    fn test_trick_winner() {
        let test_cases = [
            TrickWinnerTestCase {
                relish: 1,
                lead_player: 0,
                trump: None,
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
                expected_winner: 0,
                high_wins: false,
            },
            TrickWinnerTestCase {
                relish: 1,
                lead_player: 0,
                trump: None,
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
                trump: None,
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
            TrickWinnerTestCase {
                relish: 4,
                lead_player: 1,
                trump: Some(Suit::Red),
                current_trick: [
                    Some(Card {
                        id: 0,
                        value: 9,
                        suit: Suit::Green,
                    }),
                    Some(Card {
                        id: 1,
                        value: 3,
                        suit: Suit::Blue,
                    }),
                ],
                high_wins: false,
                expected_winner: 1,
            },
            TrickWinnerTestCase {
                high_wins: true,
                relish: 7,
                lead_player: 0,
                trump: Some(Suit::Blue),
                current_trick: [
                    Some(Card {
                        suit: Suit::Green,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Yellow,
                        value: 10,
                        id: 1,
                    }),
                ],
                expected_winner: 0,
            },
            TrickWinnerTestCase {
                relish: 4,
                lead_player: 0,
                trump: Some(Suit::Red),
                current_trick: [
                    Some(Card {
                        id: 0,
                        value: 10,
                        suit: Suit::Green,
                    }),
                    Some(Card {
                        id: 1,
                        value: 1,
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
            game.high_wins = Some(test_case.high_wins);
            game.trump = test_case.trump;
            assert_eq!(
                game.trick_winner(),
                test_case.expected_winner,
                "{:?}",
                test_case
            );
        }
    }
}
