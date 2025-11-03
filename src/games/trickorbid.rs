/*
Game: Trick or Bid
Designer: Jeremy Zero
BoardGameGeek: https://boardgamegeek.com/boardgame/427341/trick-or-bid
*/

use std::collections::{HashMap, HashSet};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

const PLAYER_COUNT: usize = 4;
const HAND_SIZE: usize = 13;
const PASS: i32 = -1;

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
    Purple = 0,
    Green = 1,
    Yellow = 2,
    Red = 3,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    pub suit: Suit,
    value: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Play, // Play a card from hand
    SelectBidOrPass, // Select a bid from captured cards or pass
    GameOver,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Bid,
    Hand,
    Score,
    Message,
    Play,
    TricksTaken,  // Move tricks to the player that won them
    TricksBurned, // Move tied tricks off screen - not to a particular player
    ReorderHand,
    ScoreCards,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    Shuffle,
    ShowPlayable,
    HidePlayable,
    ShowWinningCard,
    Message,
    Score,
    GameOver,
    OptionalPause,
    BurnTrick,
    TricksToWinner,
    Reorder,
    SelectBidCardOrPass,
    Bid, // Card selected to bid
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    pub object_id: i32,
    pub dest: Location,
    pub start_score: i32,
    pub end_score: i32,
    pub offset: usize,
    pub player: usize,
    pub length: usize,
    pub message: Option<String>,
}

pub struct TrickResult {
    // Tied tricks accumulate
    tie: bool,
    winning_player: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrickOrBidGame {
    pub hands: [Vec<Card>; PLAYER_COUNT],
    pub bid_cards: [Option<Card>; PLAYER_COUNT],
    pub state: State,
    pub changes: Vec<Vec<Change>>,
    pub no_changes: bool,
    pub scores: [i32; PLAYER_COUNT],
    pub winner: Option<usize>,
    pub current_player: usize,
    pub lead_player: usize,
    pub dealer: usize,
    pub current_hand: [Option<Card>; PLAYER_COUNT],
    pub round: i32,
    pub cards_won: [Vec<Card>; PLAYER_COUNT],
    pub previous_trick_cards: Vec<Card>,
    pub trump_card: Option<Card>,
    pub experiment: bool, // Set to true when testing new reward functions
}

impl TrickOrBidGame {
    pub fn new() -> Self {
        let mut game = TrickOrBidGame {
            ..Default::default()
        };
        // Randomly select a start player each game
        game.dealer = thread_rng().gen_range(0..PLAYER_COUNT);
        game.deal();
        game
    }

    fn deal(&mut self) {
        let mut deck = TrickOrBidGame::deck();
        self.cards_won = [vec![], vec![], vec![], vec![]];
        self.bid_cards = [None; PLAYER_COUNT];
        self.state = State::Play;
        self.dealer = (self.dealer + 1) % PLAYER_COUNT;
        self.current_player = self.dealer;
        self.lead_player = self.dealer;

        let shuffle_index = self.new_change();
        let deal_index = self.new_change();

        // Shuffle animation
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Shuffle,
                object_id: 0,
                dest: Location::Deck,
                ..Default::default()
            },
        );

        self.hands = [
            deck.drain(..HAND_SIZE).collect::<Vec<_>>(),
            deck.drain(..HAND_SIZE).collect::<Vec<_>>(),
            deck.drain(..HAND_SIZE).collect::<Vec<_>>(),
            deck,
        ];

        // Sort player 0's hand by suit, then by value (high to low)
        self.hands[0].sort_by(|a, b| match a.suit.cmp(&b.suit) {
            std::cmp::Ordering::Equal => b.value.cmp(&a.value), // Same suit: high to low
            other => other,                                     // Different suits: sort by suit
        });

        // Deal animations
        for hand_index in 0..HAND_SIZE {
            for player in 0..PLAYER_COUNT {
                if hand_index < self.hands[player].len() {
                    let card = self.hands[player][hand_index];
                    self.add_change(
                        deal_index,
                        Change {
                            change_type: ChangeType::Deal,
                            object_id: card.id,
                            dest: Location::Hand,
                            player,
                            offset: hand_index,
                            length: self.hands[player].len(),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        self.round += 1;

        // Show playable cards and message after dealing
        self.show_playable();
        self.show_message();
    }

    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        for suit in all::<Suit>() {
            for value in [0, 1, 1, 2, 2, 3, 3, 4, 5, 6, 7, 8, 9] {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
    }

    pub fn get_moves(self: &TrickOrBidGame) -> Vec<i32> {
        match self.state {
            State::SelectBidOrPass => {
                let mut moves = vec![PASS];
                moves.extend(self.previous_trick_cards.iter().map(|c| c.id));
                moves
            }
            State::Play => self.playable_card_ids(),
            State::GameOver => panic!("No moves can be made when the game is over"),
        }
    }

    pub fn playable_card_ids(&self) -> Vec<i32> {
        // Must follow
        if self.current_hand[self.lead_player].is_some() {
            let lead_suit = self.current_hand[self.lead_player].unwrap().suit;
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

    pub fn current_player_card_ids(&self) -> Vec<i32> {
        self.hands[self.current_player]
            .iter()
            .map(|c| c.id)
            .collect()
    }

    pub fn apply_move(&mut self, card_id: i32) {
        self.changes = vec![vec![]];

        if !self.get_moves().contains(&card_id) {
            panic!("invalid move");
        }

        match self.state {
            State::GameOver => panic!("Cannot play when the game is over"),
            State::SelectBidOrPass => self.select_bid_card_or_pass(card_id),
            State::Play => self.play(card_id),
        }
        self.show_playable();
        self.show_message();
    }

    pub fn select_bid_card_or_pass(&mut self, card_id: i32) {
        if card_id == PASS {
            // Player takes the trick and a new hand is started
            self.end_hand();
            return;
        }
        let card = self.pop_card(card_id);
        let player = self.current_player;

        // Animate pain card selection (face down initially)
        self.add_change(
            0,
            Change {
                change_type: ChangeType::Bid,
                object_id: card_id,
                dest: Location::Bid,
                player,
                offset: self.bid_cards.iter().flatten().count(),
                ..Default::default()
            },
        );

        self.reorder_hand(player, false);

        self.bid_cards[player] = Some(card);
        self.current_player = (self.current_player + 1) % PLAYER_COUNT;

        // If all pain cards have been selected, reveal them all
        if self.bid_cards.iter().all(|c| c.is_some()) {
            let reveal_index = self.new_change();
            // Collect pain cards to avoid borrow checker issues
            let pain_cards_to_reveal: Vec<(usize, Card)> = self
                .bid_cards
                .iter()
                .enumerate()
                .filter_map(|(idx, card)| card.map(|c| (idx, c)))
                .collect();

            // Flip all pain cards face up
            for (player_index, card) in pain_cards_to_reveal {
                self.add_change(
                    reveal_index,
                    Change {
                        change_type: ChangeType::RevealPainCards,
                        object_id: card.id,
                        dest: Location::PainColor,
                        player: player_index,
                        offset: player_index,
                        ..Default::default()
                    },
                );
            }

            self.current_player = self.dealer;
            self.state = State::Play;
        }
    }

    pub fn play(&mut self, card_id: i32) {
        let card = self.pop_card(card_id);
        let player = self.current_player;

        // Animate played cards
        self.add_change(
            0,
            Change {
                change_type: ChangeType::Play,
                object_id: card_id,
                dest: Location::Play,
                player,
                ..Default::default()
            },
        );

        self.reorder_hand(player, false);

        self.current_hand[player] = Some(card);

        if self.current_hand.iter().any(|c| c.is_none()) {
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
            return;
        }

        // The trick is over
        let trick_result = TrickOrBidGame::trick_winner(self.lead_player, self.current_hand);
        let trick_winner = trick_result.winning_player;

        // Show winning card and pause
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::ShowWinningCard,
                object_id: self.current_hand[trick_winner].unwrap().id,
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

        self.current_player = trick_winner;
        self.lead_player = trick_winner;

        if trick_result.score_hand {
            // Add won cards to the winner's won cards
            self.cards_won[trick_winner].extend(self.current_hand.iter().flatten().copied());

            // Animate trick won to winner
            let change_index = self.new_change();
            for card in self.current_hand {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.unwrap().id,
                        dest: Location::TricksTaken,
                        player: trick_winner,
                        ..Default::default()
                    },
                );
            }
        } else {
            // Animate trick off screen - not to winner
            let change_index = self.new_change();
            for card in self.current_hand {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.unwrap().id,
                        dest: Location::TricksTaken,
                        player: trick_winner,
                        ..Default::default()
                    },
                );
            }
        }

        // Reset the trick
        self.current_hand = [None; 4];

        if self.hands.iter().any(|h| h.len() > 0) {
            // Hand continues
            return;
        }
    }

    pub fn end_hand(&mut self) {
        // Round is over
        // Show scoring animations for each player, interleaved with score updates
        for player in 0..PLAYER_COUNT {
            // Show this player's captured cards
            self.show_scoring_cards(player);

            // Show score modification for this player
            let score_index = self.new_change();
            let score = TrickOrBidGame::score_cards_won(
                self.bid_cards[player].unwrap(),
                &self.cards_won[player],
            );
            self.add_change(
                score_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: self.scores[player],
                    end_score: self.scores[player] + score,
                    ..Default::default()
                },
            );
            self.scores[player] += score;

            // Hide the scoring cards before moving to the next player
            let hide_index = self.new_change();
            self.add_change(
                hide_index,
                Change {
                    change_type: ChangeType::HideScoringCards,
                    object_id: 0,
                    dest: Location::ScoreCards,
                    player,
                    ..Default::default()
                },
            );
        }

        if self.round >= PLAYER_COUNT as i32 {
            self.state = State::GameOver;
            let max_score = self.scores.iter().max().unwrap();
            for player in 0..PLAYER_COUNT {
                if self.scores[player] == *max_score {
                    // Ties go to player 0 (human player)
                    self.winner = Some(player);
                    break;
                }
            }

            let game_over_index = self.new_change();
            self.add_change(
                game_over_index,
                Change {
                    change_type: ChangeType::GameOver,
                    ..Default::default()
                },
            );

            return;
        } else {
            self.deal();
        }
    }

    pub fn pop_card(&mut self, card_id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == card_id)
            .unwrap();
        self.hands[self.current_player].remove(pos)
    }

    pub fn trick_winner(
        lead_player: usize,
        current_hand: [Option<Card>; PLAYER_COUNT],
    ) -> TrickResult {
        let mut winning_player = lead_player;
        let mut winning_card = current_hand[lead_player].unwrap();
        let eligible_cards = TrickOrBidGame::non_matching_cards(current_hand);

        if eligible_cards.len() == 0 {
            return TrickResult {
                tie: true,
                winning_player: lead_player,
            };
        }

        let mut card_id_to_player: HashMap<i32, usize> = HashMap::new();
        for (player, card) in current_hand.iter().enumerate() {
            if let Some(card) = card {
                card_id_to_player.insert(card.id, player);
            }
        }

        let lead_suit = winning_card.suit;


        eligible_cards.sort_by_key(|c| std::cmp::Reverse(self.value_for_card(lead_suit, c)));
        *card_id_to_player
            .get(
                &eligible_cards
                    .first()
                    .expect("there should be a winning card")
                    .id,
            )
            .expect("cards_to_player missing card")


        TrickResult {
            tie: false,
            winning_player,
        }
    }

    fn non_matching_cards(cards: [Option<Card>; 4]) -> Vec<Card> {
        let mut card_count: HashMap<Card, i32> = HashMap::new();
        for card in cards {
            *card_count.entry(card.unwrap()).or_insert(0) += 1;
        }
        card_count
            .iter()
            .filter(|(_, v)| *v == &1)
            .map(|(k, _)| *k)
            .collect::<Vec<_>>() // Collect first
            .into()
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
                dest: Location::Hand,
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

    fn show_message(&mut self) {
        let player_name = match self.current_player {
            0 => "You".to_string(),
            1 => "West".to_string(),
            2 => "North".to_string(),
            _ => "East".to_string(),
        };

        let message = match self.state {
            State::SelectPainColor => Some(format!("{} must select a pain color", player_name)),
            State::Play => None,
            State::GameOver => None,
        };

        let index = self.new_change();
        self.set_message(message, index);
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

    fn show_scoring_cards(&mut self, player: usize) {
        let bid_card = self.bid_cards[player].unwrap();
        let cards_won = &self.cards_won[player];

        // Calculate score delta for this player
        let score_delta = TrickOrBidGame::score_cards_won(bid_card, cards_won);

        // Separate pain cards from other cards
        let mut pain_cards_won: Vec<Card> = cards_won
            .iter()
            .filter(|c| c.suit == bid_card.suit)
            .copied()
            .collect();
        let mut other_cards: Vec<Card> = cards_won
            .iter()
            .filter(|c| c.suit != bid_card.suit)
            .copied()
            .collect();

        // Add the selected pain card to the pain cards list so it's sorted with them
        pain_cards_won.push(bid_card);

        // Sort ALL pain cards by value (high to low)
        // This ensures the selected pain card appears in the correct sorted position
        pain_cards_won.sort_by(|a, b| b.value.cmp(&a.value));

        // Sort other cards by suit, then by value (high to low)
        other_cards.sort_by(|a, b| {
            match a.suit.cmp(&b.suit) {
                std::cmp::Ordering::Equal => b.value.cmp(&a.value), // Same suit: high to low
                other => other,                                     // Different suits: sort by suit
            }
        });

        // Total cards to display = captured cards + selected pain card
        let total_length = cards_won.len() + 1;

        // Show other cards
        for card in other_cards {
            self.add_change(
                change_index,
                Change {
                    change_type: ChangeType::ShowScoringCard,
                    object_id: card.id,
                    dest: Location::ScoreCards,
                    player,
                    offset,
                    length: total_length,
                    start_score: self.scores[player],
                    end_score: self.scores[player] + score_delta,
                    ..Default::default()
                },
            );
            offset += 1;
        }

        // Add optional pause after all cards are shown
        // User can review all cards and the score delta before score is updated
        self.add_change(
            change_index,
            Change {
                change_type: ChangeType::OptionalPause,
                object_id: 0,
                dest: Location::ScoreCards,
                player,
                start_score: self.scores[player],
                end_score: self.scores[player] + score_delta,
                ..Default::default()
            },
        );
    }
}

impl ismcts::Game for StickEmGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        // Pain cards are played face down until all are played
        let mut pain_card_played = [false; PLAYER_COUNT];

        if self.bid_cards.contains(&None) {
            for (player, card) in self.bid_cards.iter().enumerate() {
                if let Some(card) = card {
                    pain_card_played[player] = true;
                    self.hands[player].push(*card);
                }
            }
        }

        for p1 in 0..PLAYER_COUNT {
            for p2 in 0..PLAYER_COUNT {
                if p1 == p2 {
                    continue;
                }
                if p1 == self.current_player() || p2 == self.current_player() {
                    // Don't swap current player's cards - player knows exactly what they have
                    continue;
                }
                let mut new_hands = vec![
                    self.hands[p1 as usize].clone(),
                    self.hands[p2 as usize].clone(),
                ];

                shuffle_and_divide_matching_cards(|_: &Card| true, &mut new_hands, rng);

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();
            }
        }

        for player in 0..PLAYER_COUNT {
            if pain_card_played[player] {
                let card = self.hands[player].pop();
                self.bid_cards[player] = card;
            }
        }
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
        if self.state != State::GameOver {
            // the hand is not over
            None
        } else {
            let max_score = *self.scores.iter().max().unwrap() as f64;
            let min_score = *self.scores.iter().min().unwrap() as f64;
            let player_score = self.scores[player] as f64;

            if max_score == min_score {
                // Everyone tied - neutral result
                return Some(0.0);
            }

            // Exponential reward function (based on experiments showing improved performance)
            // Linear interpolation between worst and best score, then apply exponential transformation
            let linear = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;

            // Apply exponential transformation: sign(x) * (|x|^2)
            // This amplifies reward differences while preserving direction
            let exponential_reward = linear.signum() * linear.abs().powf(2.0);

            Some(exponential_reward)
        }
    }
}

pub fn get_mcts_move(game: &StickEmGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 8;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    ismcts.best_move().expect("should have a move to make")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let game = StickEmGame::new();
        assert!(
            game.hands.iter().all(|h| h.len() == 15),
            "Every player should have 15 cards in their hand"
        );
        assert_eq!(
            game.state,
            State::SelectPainColor,
            "Every player selects a pain color first"
        );
        assert_eq!(game.round, 1, "The game starts in round 1");
    }

    struct TrickWinnerScenario {
        lead_player: usize,
        current_hand: [Option<Card>; 4],
        expected_winning_player: usize,
        expected_score_hand: bool,
        name: String,
    }

    #[test]
    fn test_trick_winner() {
        let scenarios = [
            TrickWinnerScenario {
                name: "All zeroes should skip scoring and keep lead player".to_string(),
                lead_player: 2,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 0,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Green,
                        value: 0,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Yellow,
                        value: 0,
                    }),
                ],
                expected_winning_player: 2,
                expected_score_hand: false,
            },
            TrickWinnerScenario {
                name: "Only non-lead suit should win".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 1,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Purple,
                        value: 1,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Purple,
                        value: 2,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "All lead suit - highest value wins".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Blue,
                        value: 3,
                    }), // lead
                    Some(Card {
                        id: 1,
                        suit: Suit::Blue,
                        value: 10,
                    }), // winner
                    Some(Card {
                        id: 2,
                        suit: Suit::Blue,
                        value: 7,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 5,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Mix of suits - highest off-suit wins".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Yellow,
                        value: 9,
                    }), // lead
                    Some(Card {
                        id: 1,
                        suit: Suit::Green,
                        value: 5,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Red,
                        value: 6,
                    }), // winner (highest off-suit)
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 4,
                    }),
                ],
                expected_winning_player: 2,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Tie in off-suit - first played wins".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 11,
                    }), // lead
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 5,
                    }), // winner (first of tied highest)
                    Some(Card {
                        id: 2,
                        suit: Suit::Yellow,
                        value: 5,
                    }), // same value but played later
                    Some(Card {
                        id: 3,
                        suit: Suit::Green,
                        value: 8,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Zeroes ignored - off-suit beats lead suit".to_string(),
                lead_player: 2,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Blue,
                        value: 7,
                    }), // winner (only off-suit)
                    Some(Card {
                        id: 2,
                        suit: Suit::Green,
                        value: 11,
                    }), // lead
                    Some(Card {
                        id: 3,
                        suit: Suit::Green,
                        value: 0,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "High lead card loses to low off-suit".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Blue,
                        value: 14,
                    }), // lead (highest overall)
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 1,
                    }), // winner (lowest off-suit)
                    Some(Card {
                        id: 2,
                        suit: Suit::Blue,
                        value: 12,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 10,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Multiple off-suits with zeroes mixed in".to_string(),
                lead_player: 3,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Yellow,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 8,
                    }), // winner
                    Some(Card {
                        id: 2,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Green,
                        value: 5,
                    }), // lead
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "BUG: Lead player 2 has highest card, all same suit - should win".to_string(),
                lead_player: 2,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Blue,
                        value: 3,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Blue,
                        value: 7,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Blue,
                        value: 11,
                    }), // lead - highest card
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 5,
                    }),
                ],
                expected_winning_player: 2,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "BUG: Lead player 3 plays only non-zero card - should win".to_string(),
                lead_player: 3,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Blue,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 0,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Green,
                        value: 0,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Yellow,
                        value: 5,
                    }), // lead - only non-zero
                ],
                expected_winning_player: 3,
                expected_score_hand: true,
            },
        ];
        for scenario in scenarios {
            let trick_result =
                StickEmGame::trick_winner(scenario.lead_player, scenario.current_hand);
            assert_eq!(
                scenario.expected_winning_player, trick_result.winning_player,
                "winning player for scenario {}",
                scenario.name
            );
            assert_eq!(
                scenario.expected_score_hand, trick_result.score_hand,
                "score hand for scenario {}",
                scenario.name
            );
        }
    }

    struct ScoreScenario {
        pain_card: Card,
        cards_won: Vec<Card>,
        expected_score: i32,
    }

    #[test]
    fn test_score() {
        let scenarios = [
            // Jillian selected the Yellow 2 at the beginning of the round as her Pain Color. She
            // receives 11 negative points (2+5+4). For the other six cards, she receives one point
            // each for a total of 6 positive points. Jillian’s point total is -5.
            ScoreScenario {
                pain_card: Card {
                    id: 0,
                    suit: Suit::Yellow,
                    value: 2,
                },
                cards_won: vec![
                    Card {
                        id: 1,
                        suit: Suit::Yellow,
                        value: 5,
                    },
                    Card {
                        id: 2,
                        suit: Suit::Yellow,
                        value: 4,
                    },
                    Card {
                        id: 3,
                        suit: Suit::Red,
                        value: 5,
                    },
                    Card {
                        id: 4,
                        suit: Suit::Purple,
                        value: 11,
                    },
                    Card {
                        id: 5,
                        suit: Suit::Green,
                        value: 11,
                    },
                    Card {
                        id: 6,
                        suit: Suit::Green,
                        value: 8,
                    },
                    Card {
                        id: 7,
                        suit: Suit::Green,
                        value: 2,
                    },
                    Card {
                        id: 8,
                        suit: Suit::Blue,
                        value: 1,
                    },
                ],
                expected_score: -5,
            },
            // Andrew selected the Red 0 at the beginning of the round as his Pain Color. He
            // receives 7 negative points (0+2+4+1). For the other nine cards, he receives one
            // point each for a total of 9 positive points. Andrew’s point total is +2.
            ScoreScenario {
                pain_card: Card {
                    id: 0,
                    suit: Suit::Red,
                    value: 0,
                },
                cards_won: vec![
                    Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 2,
                    },
                    Card {
                        id: 2,
                        suit: Suit::Red,
                        value: 4,
                    },
                    Card {
                        id: 3,
                        suit: Suit::Red,
                        value: 1,
                    },
                    Card {
                        id: 4,
                        suit: Suit::Yellow,
                        value: 0,
                    },
                    Card {
                        id: 5,
                        suit: Suit::Blue,
                        value: 3,
                    },
                    Card {
                        id: 6,
                        suit: Suit::Blue,
                        value: 7,
                    },
                    Card {
                        id: 7,
                        suit: Suit::Blue,
                        value: 10,
                    },
                    Card {
                        id: 8,
                        suit: Suit::Blue,
                        value: 6,
                    },
                    Card {
                        id: 9,
                        suit: Suit::Purple,
                        value: 9,
                    },
                    Card {
                        id: 10,
                        suit: Suit::Purple,
                        value: 8,
                    },
                    Card {
                        id: 11,
                        suit: Suit::Purple,
                        value: 0,
                    },
                    Card {
                        id: 12,
                        suit: Suit::Green,
                        value: 6,
                    },
                ],
                expected_score: 2,
            },
        ];

        for scenario in scenarios {
            let actual_score =
                StickEmGame::score_cards_won(scenario.pain_card, &scenario.cards_won);
            assert_eq!(actual_score, scenario.expected_score);
        }
    }

    #[test]
    fn test_select_final_pain_card() {
        let mut game = StickEmGame::new();
        game.current_player = 1;
        game.dealer = 2;
        game.hands[1] = vec![Card {
            id: 4,
            value: 11,
            suit: Suit::Purple,
        }];
        game.bid_cards = [
            Some(Card {
                id: 0,
                value: 0,
                suit: Suit::Red,
            }),
            None,
            Some(Card {
                id: 2,
                value: 0,
                suit: Suit::Yellow,
            }),
            Some(Card {
                id: 3,
                value: 0,
                suit: Suit::Purple,
            }),
        ];
        game.apply_move(4);
        assert_eq!(
            game.current_player, game.dealer,
            "After last player selects pain suit "
        );
        assert_eq!(game.bid_cards[1].unwrap().id, 4);
        assert_eq!(
            game.state,
            State::Play,
            "Game should transition to play state after all pain cards are selected"
        );
    }

    #[test]
    fn test_play_final_card_in_trick() {
        let mut game = StickEmGame::new();
        game.state = State::Play;
        game.current_player = 1;
        game.lead_player = 2;
        game.dealer = 2;
        game.hands[1] = vec![Card {
            id: 4,
            value: 11,
            suit: Suit::Purple,
        }];
        game.current_hand = [
            Some(Card {
                id: 0,
                value: 0,
                suit: Suit::Red,
            }),
            None,
            Some(Card {
                id: 2,
                value: 0,
                suit: Suit::Yellow,
            }),
            Some(Card {
                id: 3,
                value: 0,
                suit: Suit::Purple,
            }),
        ];
        game.apply_move(4);
        assert_eq!(
            game.current_player, 1,
            "Player 1 won by playing the highest off suit card"
        );
        assert!(
            game.current_hand.iter().all(|c| c.is_none()),
            "Hand is reset"
        );
    }

    #[test]
    fn test_play_final_card_in_trick_last_round() {
        let mut game = StickEmGame::new();
        game.scores = [-50, 0, 0, 0];
        game.round = PLAYER_COUNT as i32;
        game.state = State::Play;
        game.current_player = 1;
        game.lead_player = 2;
        game.dealer = 2;
        game.bid_cards = [
            Some(Card {
                id: 4,
                value: 1,
                suit: Suit::Purple,
            }),
            Some(Card {
                id: 4,
                value: 2,
                suit: Suit::Purple,
            }),
            Some(Card {
                id: 4,
                value: 3,
                suit: Suit::Purple,
            }),
            Some(Card {
                id: 4,
                value: 4,
                suit: Suit::Purple,
            }),
        ];
        game.hands = [
            vec![],
            vec![Card {
                id: 4,
                value: 11,
                suit: Suit::Purple,
            }],
            vec![],
            vec![],
        ];
        game.current_hand = [
            Some(Card {
                id: 0,
                value: 0,
                suit: Suit::Red,
            }),
            None,
            Some(Card {
                id: 2,
                value: 0,
                suit: Suit::Yellow,
            }),
            Some(Card {
                id: 3,
                value: 0,
                suit: Suit::Purple,
            }),
        ];
        game.apply_move(4);
        assert_eq!(
            game.current_player, 1,
            "Player 1 won by playing the highest off suit card"
        );
        assert_eq!(game.state, State::GameOver);
        assert_eq!(game.scores, [-51, -11, -3, -4], "Scores are correct");
        assert_eq!(game.winner, Some(2), "Winner is properly set")
    }
}
