/*
Game: Kansas City: The Trick-Taking Game
Designer: Chris Wray
BoardGameGeek: https://boardgamegeek.com/boardgame/424451/kansas-city-the-trick-taking-game
*/

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

const SKIP_TRUMP_PROMOTION: i32 = -1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    // Trick play
    Play,
    // Optionally select a card from your hand to convert it to trump
    OptionallyPromoteTrump,
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
    Stars = 0,
    Spades = 1,
    Moons = 2,
    Hearts = 3,
    Diamonds = 4,
    Clubs = 5,
    Triangles = 6,
    Trump = 7,
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
    PromoteToTrump,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    Reorder,
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
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
struct BidOption {
    id: i32,
    description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KansasCityGame {
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
}

impl KansasCityGame {
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
        self.state = State::Play;
        self.tricks_taken = [0, 0, 0, 0];
        self.round += 1;
        self.hands = [vec![], vec![], vec![], vec![]];
        self.current_player = self.dealer;
        self.lead_player = self.current_player;
        self.current_trick = [None; 4];
        self.dealer = (self.dealer + 1) % 2;
        self.voids = [vec![], vec![], vec![], vec![]];
        let mut cards = KansasCityGame::deck();
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
            self.hands[player].sort_by(if player == 0 {
                human_card_sorter
            } else {
                opponent_card_sorter
            });
            self.reorder_hand(player, player == 0);
        }
        self.show_playable();
    }

    pub fn deck() -> Vec<Card> {
        let mut deck: Vec<Card> = vec![];
        let mut id = 0;
        for suit in all::<Suit>() {
            if suit == Suit::Trump {
                continue;
            }
            for value in 1..=8 {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }
        deck.shuffle(&mut thread_rng());
        deck
    }

    pub fn trick_winner(&self) -> usize {
        return self.get_winner(
            self.current_trick[self.lead_player].unwrap().suit,
            &self.current_trick,
        );
    }

    pub fn get_moves(self: &KansasCityGame) -> Vec<i32> {
        match self.state {
            State::OptionallyPromoteTrump => {
                let mut promote_ids = self.promotable_card_ids();
                promote_ids.insert(0, SKIP_TRUMP_PROMOTION);
                promote_ids
            }
            State::Play => self.playable_card_ids(),
        }
    }

    pub fn promotable_card_ids(&self) -> Vec<i32> {
        let active_trump_values: HashSet<i32> = self
            .hands
            .concat()
            .iter()
            .filter(|c| c.suit == Suit::Trump)
            .map(|c| c.value)
            .collect();
        return self.hands[self.current_player]
            .iter()
            .filter(|c| c.suit != Suit::Trump && !active_trump_values.contains(&c.value))
            .map(|c| c.id)
            .collect();
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
        return self.hands[self.current_player]
            .iter()
            .map(|c| c.id)
            .collect();
    }

    fn apply_move_internal(&mut self, action: i32) {
        match self.state {
            State::OptionallyPromoteTrump => {
                if action != SKIP_TRUMP_PROMOTION {
                    let index = self.new_change();
                    for hand_card in self.hands[self.current_player].iter_mut() {
                        if hand_card.id == action {
                            hand_card.suit = Suit::Trump;
                        }
                    }
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::PromoteToTrump,
                            dest: Location::Hand,
                            player: self.current_player,
                            object_id: action,
                            ..Default::default()
                        },
                    );
                    self.hands[self.current_player].sort_by(if self.current_player == 0 {
                        human_card_sorter
                    } else {
                        opponent_card_sorter
                    });
                    self.reorder_hand(self.current_player, true);
                }

                loop {
                    self.current_player = (self.current_player + 1) % 4;
                    if self.current_player == self.lead_player {
                        self.state = State::Play;
                        break;
                    }
                    if self.get_moves().len() == 1 {
                        // Don't present an option to select a trump card
                        // to players that can only pass
                        continue;
                    }
                    break;
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

                    let trick_winner = self.trick_winner();
                    self.lead_player = trick_winner;
                    self.current_player = (trick_winner + 1) % 4;
                    self.tricks_taken[trick_winner] += 1;

                    // Report scored 4s
                    let points: i32 = self
                        .current_trick
                        .iter()
                        .filter(|c| c.unwrap().value == 4)
                        .count() as i32
                        * 2;

                    let index = self.new_change();

                    if points > 0 {
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::Score,
                                object_id: self.lead_player as i32,
                                player: self.lead_player,
                                dest: Location::Score,
                                start_score: self.scores[self.lead_player],
                                end_score: self.scores[self.lead_player] + points,
                                ..Default::default()
                            },
                        );
                        // Record score change
                        self.scores[self.lead_player] += points;
                    }

                    self.state = State::OptionallyPromoteTrump;

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

                    // Animate tricks to winner
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

                        // Update scores
                        let score_index = self.new_change();
                        for player in 0..4 {
                            let points = match self.tricks_taken[player] {
                                1 => 5,
                                2 => 10,
                                3 => 15,
                                4 => 5,
                                _ => 0,
                            };
                            // Report the score change to the UI
                            self.add_change(
                                score_index,
                                Change {
                                    change_type: ChangeType::Score,
                                    object_id: player as i32,
                                    player,
                                    dest: Location::Score,
                                    start_score: self.scores[player as usize],
                                    end_score: self.scores[player as usize] + points,
                                    ..Default::default()
                                },
                            );
                            // Record score change
                            self.scores[player] += points;
                        }

                        // Check if the game is over
                        if self.round >= 2 {
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
        self.add_change(
            change_index,
            Change {
                object_id: SKIP_TRUMP_PROMOTION,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                player: self.current_player,
                ..Default::default()
            },
        );
    }

    pub fn get_winner(&self, lead_suit: Suit, trick: &[Option<Card>; 4]) -> usize {
        let mut card_id_to_player: HashMap<i32, usize> = HashMap::new();
        for (player, card) in trick.iter().enumerate() {
            if let Some(card) = card {
                card_id_to_player.insert(card.id, player);
            }
        }
        let mut cards: Vec<Card> = trick
            .iter() // Convert the Vec into an Iterator
            .filter_map(|&x| x) // filter_map will only pass through the Some values
            .collect();
        cards.sort_by_key(|c| std::cmp::Reverse(self.value_for_card(lead_suit, c)));
        *card_id_to_player
            .get(&cards.first().expect("there should be a winning card").id)
            .expect("cards_to_player missing card")
    }

    pub fn value_for_card(&self, lead_suit: Suit, card: &Card) -> i32 {
        let mut bonus: i32 = 0;
        if card.suit == lead_suit {
            bonus += 100;
        }
        if card.suit == Suit::Trump {
            bonus += 200;
        }
        card.value + bonus
    }
}

impl ismcts::Game for KansasCityGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        for p1 in 0..4 {
            for p2 in 0..4 {
                if p1 == self.current_player() || p2 == self.current_player() || p1 == p2 {
                    // Don't swap current player's cards - player knows exactly what they have
                    continue;
                }

                let mut combined_voids: HashSet<Suit> =
                    HashSet::from_iter(self.voids[p1 as usize].iter().cloned());
                combined_voids.extend(self.voids[p2 as usize].iter());

                let mut new_hands = vec![
                    self.hands[p1 as usize].clone(),
                    self.hands[p2 as usize].clone(),
                ];

                for value in 1..=8 {
                    shuffle_and_divide_matching_cards(
                        |c: &Card| {
                            // Trump cards are visible - do not swap
                            c.suit != Suit::Trump
                                 // Do not swap cards where one player has a known void in that suit
                                && !combined_voids.contains(&c.suit)
                                // Values are visible on the backs of cards, only exchange
                                // cards with the same value
                                && c.value == value
                        },
                        &mut new_hands,
                        rng,
                    );
                }

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();
            }
        }
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
            if self.winner.is_none() {
                // the hand is not over
                None
            } else {
                let current_player_score = self.scores[player] as f64;
                let other_player_score = *self
                    .scores
                    .iter()
                    .enumerate()
                    .filter(|(player, _)| *player != self.current_player)
                    .max()
                    .unwrap()
                    .1 as f64;
                let score_difference = current_player_score - other_player_score;
                if current_player_score >= other_player_score {
                    Some(0.8 + ((score_difference / 10.0) * 0.2))
                } else {
                    Some(0.2 + ((score_difference / 10.0) * 0.2))
                }
            }
        }
    }
}

pub fn get_mcts_move(game: &KansasCityGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    // reset scores for the simulation
    new_game.scores = [0; 4];
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

fn human_card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

fn opponent_card_sorter(a: &Card, b: &Card) -> Ordering {
    if a.suit != b.suit && a.suit == Suit::Trump {
        Ordering::Greater
    } else if a.suit != b.suit && b.suit == Suit::Trump {
        Ordering::Less
    } else {
        a.value.cmp(&b.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck() {
        let d = KansasCityGame::deck();
        assert_eq!(d.len(), 56);
    }

    #[derive(Debug)]
    struct TrickWinnerTestCase {
        description: String,
        current_trick: [Option<Card>; 4],
        lead_player: usize,
        expected_winner: usize,
    }

    #[test]
    fn test_trick_winner() {
        let test_cases = [
            TrickWinnerTestCase {
                description: "Highest trump card wins".to_string(),
                lead_player: 0,
                current_trick: [
                    Some(Card {
                        suit: Suit::Triangles,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Trump,
                        value: 10,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 3,
                        suit: Suit::Clubs,
                    }),
                    Some(Card {
                        id: 3,
                        value: 2,
                        suit: Suit::Clubs,
                    }),
                ],
                expected_winner: 1,
            },
            TrickWinnerTestCase {
                description: "Highest lead suit wins".to_string(),
                lead_player: 0,
                current_trick: [
                    Some(Card {
                        suit: Suit::Triangles,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Spades,
                        value: 10,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 8,
                        suit: Suit::Triangles,
                    }),
                    Some(Card {
                        id: 3,
                        value: 2,
                        suit: Suit::Clubs,
                    }),
                ],
                expected_winner: 2,
            },
        ];
        for test_case in test_cases {
            let mut game = KansasCityGame::new();
            game.lead_player = test_case.lead_player;
            game.current_trick = test_case.current_trick;
            assert_eq!(
                game.trick_winner(),
                test_case.expected_winner,
                "{} {:?}",
                test_case.description,
                test_case
            );
        }
    }
}
