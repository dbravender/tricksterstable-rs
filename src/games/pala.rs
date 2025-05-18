/*
Game: Pala
Designer: Jeffrey Allers
BoardGameGeek: https://boardgamegeek.com/boardgame/37441/pala
*/

use once_cell::sync::Lazy;

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use enum_iterator::{all, Sequence};
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::{Deserialize, Serialize};

const HAND_SIZE: usize = 11;
const PASS_BID: i32 = -1;
const BID_OFFSET: i32 = -10; // -10 first bid slot, -11 second bid slot, etc.
const PLAYER_COUNT: usize = 4;
const POINT_THRESHOLD: i32 = 45;
const BID_CARDS: [BidSpace; PLAYER_COUNT] = [
    BidSpace::PlusFace,
    BidSpace::PlusOne,
    BidSpace::PlusOne,
    BidSpace::Cancel,
];

static ID_TO_CARD: Lazy<HashMap<i32, Card>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for card in PalaGame::deck().iter() {
        m.insert(card.id, *card);
    }
    m
});

#[derive(
    Debug, Clone, Serialize, Sequence, Deserialize, PartialEq, Eq, Copy, Hash, PartialOrd, Ord,
)]
pub enum BidSpace {
    PlusFace,
    PlusOne,
    Cancel,
    Missing,
}

impl BidSpace {
    pub fn score_for_card(&self, card: &Card) -> i32 {
        match self {
            BidSpace::PlusFace => card.value,
            BidSpace::PlusOne => 1,
            // During scoring, highest value cards will automatically be cancelled and remaining
            // cancel cards are worth -1
            BidSpace::Cancel => -1,
            // Suits which were not bid score as 0
            BidSpace::Missing => 0,
        }
    }
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

    pub fn mixed_with(&self, other: Suit) -> Suit {
        match (self, other) {
            (Suit::Blue, Suit::Yellow) => Suit::Green,
            (Suit::Yellow, Suit::Blue) => Suit::Green,
            (Suit::Red, Suit::Blue) => Suit::Purple,
            (Suit::Blue, Suit::Red) => Suit::Purple,
            (Suit::Yellow, Suit::Red) => Suit::Orange,
            (Suit::Red, Suit::Yellow) => Suit::Orange,
            _ => panic!("secondary colors cannot be mixed"),
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
    pub current_trick: [Option<Card>; PLAYER_COUNT],
    // Cards in each player's hand
    pub hands: [Vec<Card>; PLAYER_COUNT],
    // Voids revealed when a player couldn't follow a lead card (used during determination)
    pub voids: [Vec<Suit>; PLAYER_COUNT],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    pub changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    pub no_changes: bool,
    // Current score of the game
    pub scores: [i32; PLAYER_COUNT],
    // Game winner
    pub winner: Option<usize>,
    // Use experimental reward function for comparison
    pub experiment: bool,
    // Which player is the human player
    pub human_player: Option<usize>,
    // Cards played on the bid spaces
    pub bids: [Option<Suit>; PLAYER_COUNT],
    // Denormalized map of suit to BidSpace
    pub suit_to_bid: HashMap<Suit, BidSpace>,
    // Cards won by each player
    pub cards_won: [Vec<Card>; PLAYER_COUNT],
    // Card selected for moves that require multiple actions
    pub selected_card: Option<Card>,
}

impl PalaGame {
    pub fn new() -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..PLAYER_COUNT);
        game.deal();
        game
    }

    pub fn new_with_human_player(human_player: usize) -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..PLAYER_COUNT);
        game.human_player = Some(human_player);
        game.deal();
        game
    }

    // Called at the start of a game and when a new hand is dealt
    pub fn deal(&mut self) {
        self.state = State::BidSelectBidCard;
        self.hands = [vec![], vec![], vec![], vec![]];
        self.current_player = self.dealer;
        self.lead_player = self.current_player;
        self.current_trick = [None; PLAYER_COUNT];
        self.dealer = (self.dealer + 1) % PLAYER_COUNT;
        self.voids = [vec![], vec![], vec![], vec![]];
        let mut cards = PalaGame::deck();
        let shuffle_index = self.new_change();
        let deal_index = self.new_change();
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Shuffle,
                object_id: 0,
                dest: Location::Deck,
                ..Default::default()
            },
        );
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Message,
                message: Some("".to_string()),
                ..Default::default()
            },
        );
        for hand_index in 0..HAND_SIZE {
            for player in 0..PLAYER_COUNT {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        offset: hand_index,
                        length: HAND_SIZE,
                        ..Default::default()
                    },
                );
                self.hands[player].push(card);
            }
        }
        for player in 0..PLAYER_COUNT {
            self.sort_hand(player);
            self.reorder_hand(player, player == 0);
        }
        self.show_playable();
        self.show_message();
    }

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

    fn pop_card(&mut self, id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == id)
            .unwrap();
        let card = self.hands[self.current_player].remove(pos);
        return card;
    }

    // Intended to be called when all bids are finished
    pub fn set_suit_to_bid(&mut self) {
        self.suit_to_bid = HashMap::new();
        for i in 0..PLAYER_COUNT {
            let suit = self.bids[i].unwrap();
            self.suit_to_bid.insert(suit, BID_CARDS[i]);
        }
    }

    pub fn get_moves(&self) -> Vec<i32> {
        match self.state {
            State::BidSelectBidCard => self.get_moves_select_bid_card(),
            State::BidSelectBidLocation => self.get_moves_select_bid_location(),
            _ => todo!("Implement remaining states"),
        }
    }

    fn get_moves_select_bid_card(&self) -> Vec<i32> {
        let bid_suits: HashSet<Suit> = self.bids.iter().flat_map(|&s| s).collect();
        let mut options: Vec<i32> = self.hands[self.current_player]
            .iter()
            .filter(|c| !bid_suits.contains(&c.suit))
            .map(|c| c.id)
            .collect();

        options.push(PASS_BID);
        options
    }

    fn get_moves_select_bid_location(&self) -> Vec<i32> {
        self.bids
            .iter()
            .enumerate()
            .filter_map(|(i, bid)| bid.is_none().then(|| BID_OFFSET + i as i32))
            .collect()
    }

    pub fn apply_move(&mut self, action: i32) {
        if !self.get_moves().contains(&action) {
            panic!("Illegal move");
        }
        match self.state {
            State::BidSelectBidCard => self.apply_move_bid_card(action),
            State::BidSelectBidLocation => self.apply_move_bid_location(action),
            _ => todo!("Implement remaining states"),
        }
    }

    pub fn apply_move_bid_card(&mut self, action: i32) {
        if action == PASS_BID {
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
            return;
        }
        self.selected_card = Some(ID_TO_CARD.get(&action).unwrap().clone());
        self.state = State::BidSelectBidLocation;
    }

    pub fn apply_move_bid_location(&mut self, action: i32) {
        let card = self.pop_card(self.selected_card.unwrap().id);
        self.bids[(action - BID_OFFSET) as usize] = Some(card.suit);
        // TODO: Animate bid card to position
        // TODO: switch state, if bid board is filled start the hand, if not continue to the next player
        if self.bids.iter().all(|x| x.is_some()) {
            self.state = State::SelectCardToPlay;
            self.current_player = self.dealer;
        } else {
            self.state = State::BidSelectBidCard;
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
        }
    }

    pub fn score_player(&mut self, player: usize) -> i32 {
        let mut score: i32 = 0;
        for card in self.cards_won[player].iter() {
            score += self
                .suit_to_bid
                .get(&card.suit)
                .unwrap_or(&BidSpace::Missing)
                .score_for_card(&card);
        }
        return score;
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
    pub fn sort_hand(&mut self, player: usize) {
        if player != 0 {
            return;
        }
        self.hands[player].sort_by(human_card_sorter);
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
                object_id: card.id,
                player,
                offset,
                length,
                ..Default::default()
            }
        }));
    }

    fn show_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.new_change();

        if self.human_player.is_some() && self.current_player == self.human_player.unwrap() {
            let moves = self.get_moves();
            for id in moves {
                self.add_change(
                    change_index,
                    Change {
                        object_id: id,
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

    fn show_message(&mut self) {
        let player_name = self.player_name_string(self.current_player);
        let message = match self.state {
            State::BidSelectBidCard | State::BidSelectBidLocation => {
                Some(format!("{} may bid a card", player_name,))
            }
            _ => Some("".to_string()),
        };
        let index = self.new_change();
        self.set_message(message, index);
    }

    fn player_name_string(&self, player: usize) -> String {
        match player {
            0 => "You".to_string(),
            1 => "West".to_string(),
            2 => "North".to_string(),
            _ => "East".to_string(),
        }
    }

    fn set_message(&mut self, message: Option<String>, index: usize) {
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Message,
                message,
                object_id: -1,
                dest: Location::Message,
                ..Default::default()
            },
        );
    }

    fn hide_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.changes.len() - 1;
        let cards = self.hands[0].clone();
        for card in cards {
            self.add_change(
                change_index,
                Change {
                    object_id: card.id,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    player: self.current_player,
                    ..Default::default()
                },
            );
        }
    }
}

fn human_card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
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
    #[derive(Debug)]
    struct ScoreScenario {
        name: String,
        cards_won: Vec<Card>,
        expected_score: i32,
    }

    #[test]
    fn test_scoring() {
        let mut game = PalaGame::new();

        game.bids = [
            // BidSpace::PlusFace,
            Some(Suit::Orange),
            // BidSpace::PlusOne,
            Some(Suit::Purple),
            // BidSpace::PlusOne,
            Some(Suit::Red),
            // BidSpace::Cancel,
            Some(Suit::Green),
        ];
        game.set_suit_to_bid();

        let scenarios = vec![
            ScoreScenario {
                name: "unbid suits should not score".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Blue,
                    value: 5,
                }],
                expected_score: 0,
            },
            ScoreScenario {
                name: "face suits score face value".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Orange,
                    value: 7,
                }],
                expected_score: 7,
            },
            ScoreScenario {
                name: "+1 suits score 1 point".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Red,
                    value: 7,
                }],
                expected_score: 1,
            },
            ScoreScenario {
                name: "cancel suits score -1 point".to_string(),
                cards_won: vec![Card {
                    id: 0,
                    suit: Suit::Green,
                    value: 5,
                }],
                expected_score: -1,
            },
        ];

        for scenario in scenarios {
            game.cards_won[0] = scenario.cards_won;
            assert_eq!(
                game.score_player(0),
                scenario.expected_score,
                "Scenario: {}, Cards: {:?} Expected score: {}",
                scenario.name,
                game.cards_won[0],
                scenario.expected_score
            );
        }
    }

    struct GetBidMovesScenario {
        name: String,
        bids: [Option<Suit>; PLAYER_COUNT],
        hand: Vec<Card>,
        expected_moves_for_card_selection: Vec<i32>,
        card_selection_move: i32,
        expected_state_after_apply_move: State,
        expected_next_player: usize,
        expected_moves_after_card_selection: Option<Vec<i32>>,
        bid_offset_move: Option<i32>,
        expected_bids_after_bid_move: Option<[Option<Suit>; PLAYER_COUNT]>,
        expected_state_after_bid_move: Option<State>,
    }

    #[test]
    pub fn test_get_moves_bid_phase() {
        let red7 = Card {
            id: 0,
            suit: Suit::Red,
            value: 7,
        };

        let orange8 = Card {
            id: 1,
            suit: Suit::Orange,
            value: 8,
        };

        let purple5 = Card {
            id: 2,
            suit: Suit::Purple,
            value: 5,
        };

        let scenarios = [
            GetBidMovesScenario {
                name: "No bids yet - any suit can be bid".to_string(),
                bids: [None, None, None, None],
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, orange8.id, purple5.id, PASS_BID],
                card_selection_move: orange8.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 3,
                expected_moves_after_card_selection: Some(vec![
                    BID_OFFSET,
                    BID_OFFSET + 1,
                    BID_OFFSET + 2,
                    BID_OFFSET + 3,
                ]),
                bid_offset_move: Some(BID_OFFSET + 1),
                expected_bids_after_bid_move: Some([None, Some(Suit::Orange), None, None]),
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
            GetBidMovesScenario {
                name: "Cards matching previous bid not available to bid".to_string(),
                bids: [None, Some(Suit::Orange), None, None],
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: red7.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 3,
                expected_moves_after_card_selection: Some(vec![
                    BID_OFFSET,
                    BID_OFFSET + 2,
                    BID_OFFSET + 3,
                ]),
                bid_offset_move: Some(BID_OFFSET),
                expected_bids_after_bid_move: Some([
                    Some(Suit::Red),
                    Some(Suit::Orange),
                    None,
                    None,
                ]),
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
            GetBidMovesScenario {
                name: "Pass should move to the next player".to_string(),
                bids: [None, Some(Suit::Orange), None, None],
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: PASS_BID,
                expected_state_after_apply_move: State::BidSelectBidCard,
                expected_next_player: 0,
                expected_moves_after_card_selection: None,
                bid_offset_move: None,
                expected_bids_after_bid_move: None,
                expected_state_after_bid_move: Some(State::BidSelectBidCard),
            },
            GetBidMovesScenario {
                name: "Should transition to play phase once the bid board is full".to_string(),
                bids: [
                    Some(Suit::Green),
                    Some(Suit::Orange),
                    Some(Suit::Yellow),
                    None,
                ],
                hand: vec![red7, orange8, purple5],
                expected_moves_for_card_selection: vec![red7.id, purple5.id, PASS_BID],
                card_selection_move: red7.id,
                expected_state_after_apply_move: State::BidSelectBidLocation,
                expected_next_player: 3,
                expected_moves_after_card_selection: Some(vec![BID_OFFSET + 3]),
                bid_offset_move: Some(BID_OFFSET + 3),
                expected_bids_after_bid_move: Some([
                    Some(Suit::Green),
                    Some(Suit::Orange),
                    Some(Suit::Yellow),
                    Some(Suit::Red),
                ]),
                expected_state_after_bid_move: Some(State::SelectCardToPlay),
            },
        ];

        for scenario in scenarios {
            let mut game = PalaGame::new();
            game.current_player = 3;
            game.state = State::BidSelectBidCard;
            game.hands[game.current_player] = scenario.hand;
            game.bids = scenario.bids;
            let moves = game.get_moves();
            assert_eq!(
                moves, scenario.expected_moves_for_card_selection,
                "Scenario: {}, Moves: {:?} Expected moves: {:?}",
                scenario.name, moves, scenario.expected_moves_for_card_selection
            );
            game.apply_move(scenario.card_selection_move);
            assert_eq!(
                game.current_player, scenario.expected_next_player,
                "Scenario: {}, Next player: {:?} Expected next player: {:?}",
                scenario.name, game.current_player, scenario.expected_next_player,
            );
            assert_eq!(
                game.state, scenario.expected_state_after_apply_move,
                "Scenario: {}, State: {:?} Expected state: {:?}",
                scenario.name, game.state, scenario.expected_state_after_apply_move,
            );
            let moves = game.get_moves();
            if scenario.expected_moves_after_card_selection.is_none() {
                continue;
            }
            let expected_moves = scenario.expected_moves_after_card_selection.unwrap();
            assert_eq!(
                moves, expected_moves,
                "Scenario: {}, Moves: {:?} Expected moves: {:?}",
                scenario.name, moves, expected_moves
            );
            let bid_offset_move = scenario.bid_offset_move.unwrap();
            game.apply_move(bid_offset_move);
            let expected_bids_after_bid_move = scenario.expected_bids_after_bid_move.unwrap();
            assert_eq!(
                game.bids, expected_bids_after_bid_move,
                "Scenario: {}, Bids: {:?} Expected bids: {:?}",
                scenario.name, game.bids, expected_bids_after_bid_move,
            );
            let expected_state_after_bid_move = scenario.expected_state_after_bid_move.unwrap();
            assert_eq!(
                game.state, expected_state_after_bid_move,
                "Scenario: {}, State: {:?} Expected state: {:?}",
                scenario.name, game.state, expected_state_after_bid_move,
            );
        }
    }
}
