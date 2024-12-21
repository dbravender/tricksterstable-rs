/*
Game: Six of VIII
Designer: Carol LaGrow
BoardGameGeek: https://boardgamegeek.com/boardgame/394691/the-six-of-viii
*/

use std::{
    cmp::{max, min, Ordering},
    collections::{HashMap, HashSet},
    ops::{Range, RangeInclusive},
};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::thread_rng;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

const KING: i32 = 13;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    // Passing cards
    PassCard,
    // Trick play
    Play,
    // Optionally play the Church of England
    OptionallyPlayChurchOfEngland,
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
    Black = 0,
    Red = 1,
    Orange = 2,
    Yellow = 3,
    Green = 4,
    Blue = 5,
    Purple = 6,
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
    TrickAndScoreCounter,
    PassCard,
    Message,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    suit: Suit,
    points: i32,
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
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    Reorder,
    PassCard,
    Message,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    object_id: i32,
    dest: Location,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    offset: usize,
    player: usize,
    length: usize,
    message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SixOfVIIIGame {
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
    // Voids revealed when a player couldn't follow a lead card - only applies
    // to hand - not to straw piles - used to determine possible hands
    pub voids: [Vec<Suit>; 4],
    // Total number of tricks taken for the current hand
    pub tricks_taken: [i32; 4],
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
    // Current round
    pub round: usize,
    // Cards from player at index that were passed to clockwise opponent
    pub passed_cards: [Vec<Card>; 4],
    // Which player is the human player
    pub human_player: Option<usize>,
    // Track if the Church of England ability has been used this hand
    pub church_of_england_played: bool,
    // 3 cards that were not dealt to players (used during determination)
    pub burned_cards: Vec<Card>,
    // Current trump suit
    pub current_trump: Suit,
}

impl SixOfVIIIGame {
    pub fn new() -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..=3);
        game.deal();
        game
    }

    pub fn new_with_human_player(human_player: usize) -> Self {
        let mut game = Self::new();
        game.human_player = Some(human_player);
        game
    }

    // Called at the start of a game and when a new hand is dealt
    pub fn deal(&mut self) {
        self.state = State::PassCard;
        self.tricks_taken = [0, 0, 0, 0];
        self.round += 1;
        self.hands = [vec![], vec![], vec![], vec![]];
        self.passed_cards = [vec![], vec![], vec![], vec![]];
        self.current_player = self.dealer;
        self.lead_player = self.current_player;
        self.current_trick = [None; 4];
        self.dealer = (self.dealer + 1) % 4;
        self.voids = [vec![], vec![], vec![], vec![]];
        let mut cards = SixOfVIIIGame::deck();
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
        for hand_index in 0..14 {
            for player in 0..4 {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        offset: hand_index,
                        length: 14,
                        ..Default::default()
                    },
                );
                self.hands[player].push(card);
            }
        }
        for player in 0..4 {
            self.sort_hand(player);
            self.reorder_hand(player, player == 0);
        }
        self.show_playable();
        self.show_message();
    }

    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        let card_values = |suit: Suit| -> Option<RangeInclusive<i32>> {
            match suit {
                Suit::Black | Suit::Red => Some(0..=12),
                Suit::Orange => Some(4..=12),
                Suit::Yellow => Some(7..=12),
                Suit::Green => Some(4..=12),
                Suit::Blue => Some(1..=12),
                Suit::Purple => Some(KING..=KING),
            }
        };

        let point_values = |suit: Suit, value: i32| -> i32 {
            match suit {
                Suit::Black | Suit::Red => match value {
                    10 => 1,
                    8 => 3,
                    6 => 2,
                    4 => 1,
                    _ => 0,
                },
                Suit::Orange => match value {
                    10 => 1,
                    8 => 3,
                    6 => 2,
                    4 => 2,
                    _ => 0,
                },
                Suit::Yellow => match value {
                    10 => 1,
                    _ => 0,
                },
                Suit::Green => match value {
                    10 => 1,
                    8 => 3,
                    6 => 2,
                    _ => 0,
                },
                Suit::Blue => match value {
                    10 => 1,
                    8 => 3,
                    6 => 2,
                    _ => 0,
                },
                _ => 0, // Default for suits with no specific point values
            }
        };

        for suit in all::<Suit>() {
            if let Some(range) = card_values(suit) {
                for value in range {
                    let points = point_values(suit, value);
                    deck.push(Card {
                        id,
                        value,
                        points,
                        suit,
                    });
                    id += 1;
                }
            }
        }

        return deck;
    }

    pub fn get_moves(self: &SixOfVIIIGame) -> Vec<i32> {
        match self.state {
            State::PassCard => {
                if self.human_player == Some(self.current_player) {
                    let mut moves = self.current_player_card_ids();
                    moves.extend(self.passed_cards[self.current_player].iter().map(|c| c.id));
                    moves
                } else {
                    self.current_player_card_ids()
                }
            }
            State::Play => self.playable_card_ids(),
            State::OptionallyPlayChurchOfEngland => todo!(),
        }
    }

    pub fn current_player_card_ids(&self) -> Vec<i32> {
        self.hands[self.current_player]
            .iter()
            .map(|c| c.id)
            .collect()
    }

    pub fn playable_card_ids(&self) -> Vec<i32> {
        // Must follow
        if self.current_trick[self.lead_player].is_some() {
            let lead_suit = self.current_trick[self.lead_player].clone().unwrap().suit;
            let moves: Vec<i32> = self.hands[self.current_player]
                .iter()
                .filter(|c| c.suit == lead_suit)
                .map(|c| c.id)
                .collect();
            if !moves.is_empty() {
                return moves;
            }
        }
        self.current_player_card_ids()
    }

    fn apply_move_internal(&mut self, action: i32) {
        match self.state {
            State::OptionallyPlayChurchOfEngland => todo!(),
            State::PassCard => {
                if let Some(pos) = self.passed_cards[self.current_player]
                    .iter()
                    .position(|c| c.id == action)
                {
                    // Player is trying to deselect a passed card
                    // Only the human player should take this action
                    let card = self.passed_cards[self.current_player].remove(pos);
                    self.hands[self.current_player].push(card);
                    self.sort_hand(self.current_player);
                    self.reorder_hand(self.current_player, false);

                    let passed_cards = &self.passed_cards[self.current_player].clone();

                    for (index, card) in passed_cards.iter().enumerate() {
                        self.add_change(
                            0,
                            Change {
                                change_type: ChangeType::PassCard,
                                object_id: card.id,
                                dest: Location::PassCard,
                                player: self.current_player,
                                offset: index,
                                ..Default::default()
                            },
                        );
                    }

                    return;
                }

                let pos = self.hands[self.current_player]
                    .iter()
                    .position(|c| c.id == action)
                    .unwrap();
                let card = self.hands[self.current_player].remove(pos);

                self.add_change(
                    0,
                    Change {
                        change_type: ChangeType::PassCard,
                        object_id: action,
                        dest: Location::PassCard,
                        player: self.current_player,
                        offset: self.passed_cards[self.current_player].len(),
                        ..Default::default()
                    },
                );

                self.sort_hand(self.current_player);
                self.reorder_hand(self.current_player, false);

                self.passed_cards[self.current_player].push(card);

                if self.passed_cards[self.current_player].len() >= 3 {
                    self.current_player = (self.current_player + 1) % 4;
                    if self.passed_cards[self.current_player].len() >= 3 {
                        // All players have selected cards to pass, actually pass the cards
                        for player in 0..4 {
                            self.new_change();
                            let receiving_player = (player + 1) % 4;
                            if receiving_player == 0 && !self.no_changes {
                                let passed_cards = self.passed_cards[player].clone();
                                for (pass_index, card) in passed_cards.iter().enumerate() {
                                    self.add_change(
                                        player,
                                        Change {
                                            change_type: ChangeType::PassCard,
                                            object_id: card.id,
                                            dest: Location::PassCard,
                                            player: 0,
                                            offset: pass_index,
                                            ..Default::default()
                                        },
                                    );
                                }
                                self.set_message(Some("Cards received from East".to_string()), 0);
                                self.add_change(
                                    3,
                                    Change {
                                        change_type: ChangeType::OptionalPause,
                                        object_id: 0,
                                        dest: Location::Play,
                                        ..Default::default()
                                    },
                                );
                            }
                            self.hands[receiving_player].extend(self.passed_cards[player].iter());
                            self.sort_hand(receiving_player);
                            self.reorder_hand(receiving_player, false);
                        }
                        // Move to play state
                        self.state = State::Play;
                    }
                }
            }
            State::Play => {
                let lead_suit = match self.current_trick[self.lead_player] {
                    Some(lead_card) => Some(lead_card.suit),
                    None => None,
                };

                let pos = self.hands[self.current_player]
                    .iter()
                    .position(|c| c.id == action)
                    .unwrap();
                let card = self.hands[self.current_player].remove(pos);

                self.add_change(
                    0,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: action,
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

                self.current_player = (self.current_player + 1) % 4;
                self.hide_playable();

                if self.current_trick.iter().flatten().count() == 4 {
                    // End trick

                    let trick_winner = self.get_trick_winner();
                    self.lead_player = trick_winner;
                    self.current_player = (trick_winner + 1) % 4;
                    self.tricks_taken[trick_winner] += 1;

                    let index = self.new_change();

                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::ShowWinningCard,
                            object_id: self.current_trick[trick_winner].unwrap().id,
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

                    // Animate potential earned score and tricks to winner
                    let change_index = self.new_change();

                    for card in self.current_trick {
                        self.add_change(
                            change_index,
                            Change {
                                change_type: ChangeType::TricksToWinner,
                                object_id: card.unwrap().id,
                                dest: Location::TricksTaken,
                                player: trick_winner,
                                tricks_taken: self.tricks_taken[trick_winner],
                                ..Default::default()
                            },
                        );
                    }

                    // Clear trick
                    self.current_trick = [None; 4];

                    if self.hands.iter().all(|x| x.is_empty()) {
                        // The hand is over

                        // Check if the game is over
                        if self.round >= 3 {
                            let max_score = self.scores.iter().max().unwrap();
                            for player in 0..4 {
                                // 0 is first so human player will win ties
                                if self.scores[player] == *max_score {
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
        self.show_playable();
        self.show_message();
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
        if self.current_player == 0 {
            let moves = self.get_moves();
            let passed_cards: HashSet<i32> =
                HashSet::from_iter(self.passed_cards[0].iter().map(|c| c.id));
            for id in moves {
                if passed_cards.contains(&id) {
                    continue;
                }
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
        let player_name = self.player_name_string();
        let message = match self.state {
            State::PassCard => Some(format!(
                "{} must select 2 cards to pass to partner",
                player_name
            )),
            State::Play => None,
            State::OptionallyPlayChurchOfEngland => todo!(),
        };
        let index = self.new_change();
        self.set_message(message, index);
    }

    fn player_name_string(&mut self) -> String {
        match self.current_player {
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

    pub fn get_trick_winner(&self) -> usize {
        let mut card_id_to_player: HashMap<i32, usize> = HashMap::new();
        for (player, card) in self.current_trick.iter().enumerate() {
            if let Some(card) = card {
                card_id_to_player.insert(card.id, player);
            }
        }
        let mut cards: Vec<Card> = self
            .current_trick
            .iter() // Convert the Vec into an Iterator
            .filter_map(|&x| x) // filter_map will only pass through the Some values
            .collect();
        cards.sort_by_key(|c| std::cmp::Reverse(self.value_for_card(c)));
        *card_id_to_player
            .get(&cards.first().expect("there should be a winning card").id)
            .expect("cards_to_player missing card")
    }

    pub fn value_for_card(&self, card: &Card) -> i32 {
        let lead_suit = self.current_trick[self.lead_player].unwrap().suit;
        let mut bonus: i32 = 0;
        if card.suit == lead_suit {
            bonus += 100;
        }
        if card.suit == self.current_trump {
            bonus += 200;
        }
        if card.value == KING {
            bonus += 500;
        }
        card.value + bonus
    }
}

impl ismcts::Game for SixOfVIIIGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        todo!();

        // 3 cards are burned
        // assert!(remaining_cards.len() == 3);
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player + 1) % 4
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.apply_move(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.winner.is_none() {
            // the hand is not over
            None
        } else {
            if !self.experiment {
                todo!();
            } else {
                todo!();
            }
        }
    }
}

pub fn get_mcts_move(game: &SixOfVIIIGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    // reset scores for the simulation
    new_game.scores = [0; 4];
    new_game.round = 6; // force evaluation of a single hand
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 8;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    ismcts.best_move().expect("should have a move to make")
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

    #[test]
    fn test_deck() {
        let d = SixOfVIIIGame::deck();
        assert_eq!(d.len(), 63);
    }

    #[derive(Debug)]
    struct TrickWinnerTestCase {
        description: String,
        current_trick: [Option<Card>; 4],
        trump: Suit,
        lead_player: usize,
        expected_winner: usize,
    }

    #[test]
    fn test_trick_winner() {
        let test_cases = [
            TrickWinnerTestCase {
                description: "Highest trump card wins".to_string(),
                lead_player: 0,
                trump: Suit::Black,
                current_trick: [
                    Some(Card {
                        suit: Suit::Orange,
                        value: 3,
                        points: 0,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Black,
                        value: 10,
                        points: 0,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 3,
                        points: 0,
                        suit: Suit::Red,
                    }),
                    Some(Card {
                        id: 3,
                        value: 2,
                        points: 0,
                        suit: Suit::Green,
                    }),
                ],
                expected_winner: 1,
            },
            TrickWinnerTestCase {
                description: "Highest lead suit wins".to_string(),
                lead_player: 0,
                trump: Suit::Blue,
                current_trick: [
                    Some(Card {
                        suit: Suit::Orange,
                        value: 3,
                        points: 0,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Green,
                        value: 10,
                        points: 0,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 8,
                        points: 0,
                        suit: Suit::Red,
                    }),
                    Some(Card {
                        id: 3,
                        value: 2,
                        points: 0,
                        suit: Suit::Black,
                    }),
                ],
                expected_winner: 0,
            },
        ];
        for test_case in test_cases {
            let mut game = SixOfVIIIGame::new();
            game.lead_player = test_case.lead_player;
            game.current_trick = test_case.current_trick;
            game.current_trump = test_case.trump;
            assert_eq!(
                game.get_trick_winner(),
                test_case.expected_winner,
                "{} {:?}",
                test_case.description,
                test_case
            );
        }
    }
}
