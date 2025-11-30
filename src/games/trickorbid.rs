/*
Game: Trick or Bid
Designer: Jeremy Zero
BoardGameGeek: https://boardgamegeek.com/boardgame/427341/trick-or-bid
*/

use std::collections::HashMap;

use enum_iterator::Sequence;
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
    Orange = 2,
    Black = 3,
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
    UnwonTricks,  // Facedown pile for no-winner tricks (visible with counter)
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
    Bid,              // Card selected to bid
    ShowScoringCards, // Show cards in middle for scoring review
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
    pub is_trump: bool, // True when this bid establishes trump (first bid card)
    pub animate_score: bool, // True to animate score to completion, false to just show preview
    pub trick_count: i32, // Number of tricks won (for displaying on card backs)
    pub unwon_trick_count: i32, // Number of unwon tricks in the pile (for counter display)
}

pub struct TrickResult {
    // When true, no one wins - trick accumulates for next winner
    no_winner: bool,
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
    pub trump_suit: Option<Suit>,
    pub accumulated_tricks: Vec<Card>, // Cards from no-winner tricks (go to next winner's score pile)
    pub tricks_won_count: [i32; PLAYER_COUNT], // Count of tricks won (not cards)
    pub voids: [Vec<Suit>; PLAYER_COUNT], // Suits each player is void in (for determination)
    pub experiment: bool,              // Set to true when testing new reward functions
}

impl TrickOrBidGame {
    pub fn new() -> Self {
        let mut game = TrickOrBidGame {
            ..Default::default()
        };
        // Randomly select a start player each game
        game.dealer = thread_rng().gen_range(0..PLAYER_COUNT);
        game.deal(true); // Animate initial deal
        game
    }

    fn deal(&mut self, animate: bool) {
        let mut deck = TrickOrBidGame::deck();
        self.cards_won = [vec![], vec![], vec![], vec![]];
        self.bid_cards = [None; PLAYER_COUNT];
        self.tricks_won_count = [0; PLAYER_COUNT];
        self.trump_suit = None;
        self.accumulated_tricks = vec![];
        self.voids = [vec![], vec![], vec![], vec![]];
        self.state = State::Play;
        self.dealer = (self.dealer + 1) % PLAYER_COUNT;
        self.current_player = self.dealer;
        self.lead_player = self.dealer;

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

        if !animate {
            return;
        }

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

        for suit in [Suit::Purple, Suit::Green, Suit::Orange, Suit::Black] {
            for value in [0, 1, 1, 2, 2, 3, 3, 4, 5, 6, 7, 8, 9] {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        deck
    }

    pub fn get_moves(self: &TrickOrBidGame) -> Vec<i32> {
        match self.state {
            State::SelectBidOrPass => {
                // If player already has a bid card, they can only pass (take the trick)
                if self.bid_cards[self.current_player].is_some() {
                    // This never should happen in practice because this state should be skipped
                    // for players that already have bid
                    vec![PASS]
                } else {
                    let mut moves = vec![PASS];
                    // Can only bid with cards from the current trick that the winner DIDN'T play
                    let winner_card_id = self.current_hand[self.current_player].map(|c| c.id);
                    moves.extend(
                        self.current_hand
                            .iter()
                            .flatten()
                            .filter(|c| Some(c.id) != winner_card_id)
                            .map(|c| c.id),
                    );
                    moves
                }
            }
            State::Play => self.playable_card_ids(),
            State::GameOver => {
                // Return empty moves instead of panicking - ISMCTS will use result() to determine game is over
                vec![]
            }
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
            State::GameOver => {
                // Game is over, nothing to do - ISMCTS will use result() to determine game is over
                return;
            }
            State::SelectBidOrPass => self.select_bid_card_or_pass(card_id),
            State::Play => self.play(card_id),
        }
        self.show_playable();
        self.show_message();
    }

    pub fn select_bid_card_or_pass(&mut self, card_id: i32) {
        let player = self.current_player;

        if card_id == PASS {
            // Player takes the current trick - add to their won cards
            self.tricks_won_count[player] += 1;
            let cards_to_add: Vec<Card> = self.current_hand.iter().flatten().copied().collect();
            self.cards_won[player].extend(cards_to_add.iter().copied());

            // Animate cards going to score pile
            let change_index = self.new_change();
            let current_trick_count = self.tricks_won_count[player];

            for card in &cards_to_add {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.id,
                        dest: Location::Score,
                        player,
                        trick_count: current_trick_count,
                        ..Default::default()
                    },
                );
            }

            self.current_hand = [None; PLAYER_COUNT];

            // Continue play or end round
            if self.hands.iter().any(|h| !h.is_empty()) {
                self.state = State::Play;
            } else {
                self.end_hand();
            }
            return;
        }

        // Player is selecting a bid card from the current trick
        // Find the selected card in current_hand
        let card_pos = self
            .current_hand
            .iter()
            .position(|c| c.is_some() && c.unwrap().id == card_id);
        if card_pos.is_none() {
            panic!("Selected bid card not found in current trick");
        }
        let card = self.current_hand[card_pos.unwrap()].unwrap();

        // Set trump suit if this is the first bid card
        let is_first_bid = self.trump_suit.is_none();
        if is_first_bid {
            self.trump_suit = Some(card.suit);
        }

        // Animate bid card moving to bid area
        let change_index = self.new_change();
        self.add_change(
            change_index,
            Change {
                change_type: ChangeType::Bid,
                object_id: card_id,
                dest: Location::Bid,
                player,
                offset: self.bid_cards.iter().flatten().count(),
                is_trump: is_first_bid,
                ..Default::default()
            },
        );

        // Remaining cards are DISCARDED (not counted as tricks)
        // Include player info so frontend knows where each card is animating from
        let cards_to_burn: Vec<(i32, usize)> = self
            .current_hand
            .iter()
            .enumerate()
            .filter_map(|(card_player, card_opt)| {
                card_opt
                    .filter(|c| c.id != card_id)
                    .map(|c| (c.id, card_player))
            })
            .collect();
        for (burn_card_id, card_player) in cards_to_burn {
            self.add_change(
                change_index,
                Change {
                    change_type: ChangeType::BurnTrick,
                    object_id: burn_card_id,
                    dest: Location::TricksBurned,
                    player: card_player,
                    ..Default::default()
                },
            );
        }

        self.current_hand = [None; PLAYER_COUNT];
        self.bid_cards[player] = Some(card);

        // Continue play or end round
        if self.hands.iter().any(|h| !h.is_empty()) {
            self.state = State::Play;
        } else {
            self.end_hand();
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

        // Track voids for determination
        if player != self.lead_player {
            let lead_suit = self.current_hand[self.lead_player].unwrap().suit;
            if card.suit != lead_suit && !self.voids[player].contains(&lead_suit) {
                // Player revealed a void
                self.voids[player].push(lead_suit);
            }
        }

        if self.current_hand.iter().any(|c| c.is_none()) {
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
            return;
        }

        // The trick is over
        let trick_result =
            TrickOrBidGame::trick_winner(self.lead_player, self.current_hand, self.trump_suit);

        if trick_result.no_winner {
            // No winner - add these cards to accumulated tricks (visible facedown pile)
            self.accumulated_tricks
                .extend(self.current_hand.iter().flatten().copied());

            // Calculate number of accumulated tricks (PLAYER_COUNT cards per trick)
            let accumulated_trick_count = (self.accumulated_tricks.len() / PLAYER_COUNT) as i32;

            // Animate cards moving to the unwon tricks pile (facedown with counter)
            let change_index = self.new_change();
            for card in self.current_hand {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::BurnTrick,
                        object_id: card.unwrap().id,
                        dest: Location::UnwonTricks,
                        unwon_trick_count: accumulated_trick_count,
                        ..Default::default()
                    },
                );
            }

            // Reset the trick - lead player stays the same
            self.current_hand = [None; PLAYER_COUNT];
            self.current_player = self.lead_player;

            // Check if this was the last trick
            if self.hands.iter().all(|h| h.is_empty()) {
                // Last trick with no winner - discard accumulated tricks
                self.accumulated_tricks.clear();
                // Since all players have finished their hands, just end the round
                // No need to enter SelectBidOrPass state
                self.end_hand();
            }
            return;
        }

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

        let change_index = self.new_change();

        // First, move any accumulated no-winner tricks to the winner's score pile
        // These are automatically won - not available for bid selection
        if !self.accumulated_tricks.is_empty() {
            let accumulated_trick_count = (self.accumulated_tricks.len() / PLAYER_COUNT) as i32;
            self.tricks_won_count[trick_winner] += accumulated_trick_count;
            let accumulated_cards: Vec<Card> = self.accumulated_tricks.clone();
            self.cards_won[trick_winner].extend(accumulated_cards.iter().copied());

            // Animate accumulated cards going to score pile
            let trick_count = self.tricks_won_count[trick_winner];
            for card in &accumulated_cards {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.id,
                        dest: Location::Score,
                        player: trick_winner,
                        trick_count,
                        ..Default::default()
                    },
                );
            }
            self.accumulated_tricks.clear();
        }

        // If player already has a bid card, automatically pass (take the current trick)
        if self.bid_cards[trick_winner].is_some() {
            // Add current trick to score pile
            self.tricks_won_count[trick_winner] += 1;
            let current_trick_cards: Vec<Card> =
                self.current_hand.iter().flatten().copied().collect();
            self.cards_won[trick_winner].extend(current_trick_cards.iter().copied());

            let current_trick_count = self.tricks_won_count[trick_winner];

            // Animate current trick cards going to score pile
            for card in &current_trick_cards {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.id,
                        dest: Location::Score,
                        player: trick_winner,
                        trick_count: current_trick_count,
                        ..Default::default()
                    },
                );
            }

            self.current_hand = [None; PLAYER_COUNT];

            // Continue play or end round
            if self.hands.iter().any(|h| !h.is_empty()) {
                self.state = State::Play;
            } else {
                self.end_hand();
            }
            return;
        }

        // Transition to bid selection state (only if player doesn't have a bid yet)
        // current_hand is kept - bid selection is only from the current trick
        self.state = State::SelectBidOrPass;

        // Highlight biddable cards for player 0 (excluding the winner's own card)
        if trick_winner == 0 {
            let winner_card_id = self.current_hand[trick_winner].map(|c| c.id);
            let biddable_card_ids: Vec<i32> = self
                .current_hand
                .iter()
                .flatten()
                .filter(|c| Some(c.id) != winner_card_id)
                .map(|c| c.id)
                .collect();
            for card_id in biddable_card_ids {
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::SelectBidCardOrPass,
                        object_id: card_id,
                        dest: Location::Play,
                        player: trick_winner,
                        ..Default::default()
                    },
                );
            }
        }
    }

    pub fn end_hand(&mut self) {
        // Show score previews in last animation group, pause, then animate to completion
        let preview_index = if !self.changes.is_empty() {
            self.changes.len() - 1
        } else {
            self.new_change()
        };

        // Calculate score deltas for all players
        let score_deltas: [i32; PLAYER_COUNT] = std::array::from_fn(|player| {
            TrickOrBidGame::calculate_score(self.bid_cards[player], self.tricks_won_count[player])
        });

        // Show score delta previews (don't animate yet)
        for (player, &delta) in score_deltas.iter().enumerate() {
            self.add_change(
                preview_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: self.scores[player],
                    end_score: self.scores[player] + delta,
                    animate_score: false, // Just show preview
                    ..Default::default()
                },
            );
        }

        // Add optional pause to review score previews
        self.add_change(
            preview_index,
            Change {
                change_type: ChangeType::OptionalPause,
                object_id: -1,
                ..Default::default()
            },
        );

        // After pause, animate scores to completion
        let animate_index = self.new_change();
        for (player, &delta) in score_deltas.iter().enumerate() {
            self.add_change(
                animate_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: self.scores[player],
                    end_score: self.scores[player] + delta,
                    animate_score: true, // Animate to completion
                    ..Default::default()
                },
            );
        }

        // Update scores after sending all changes
        for (player, &delta) in score_deltas.iter().enumerate() {
            self.scores[player] += delta;
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

            self.add_change(
                animate_index,
                Change {
                    change_type: ChangeType::GameOver,
                    ..Default::default()
                },
            );
        } else {
            self.deal(true); // Always animate deal between hands
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
        trump_suit: Option<Suit>,
    ) -> TrickResult {
        let lead_suit = current_hand[lead_player].unwrap().suit;

        // Get cards that are not tied (appear only once)
        let eligible_cards = TrickOrBidGame::non_matching_cards(current_hand);

        if eligible_cards.is_empty() {
            // All cards are tied - no winner
            return TrickResult {
                no_winner: true,
                winning_player: lead_player,
            };
        }

        // Map card IDs to players
        let mut card_id_to_player: HashMap<i32, usize> = HashMap::new();
        for (player, card) in current_hand.iter().enumerate() {
            if let Some(card) = card {
                card_id_to_player.insert(card.id, player);
            }
        }

        // Find trump cards first (if trump exists)
        if let Some(trump) = trump_suit {
            let trump_cards: Vec<Card> = eligible_cards
                .iter()
                .filter(|c| c.suit == trump)
                .copied()
                .collect();

            if !trump_cards.is_empty() {
                // Find highest trump card
                let winning_card = trump_cards.iter().max_by_key(|c| c.value).unwrap();
                let winning_player = *card_id_to_player.get(&winning_card.id).unwrap();
                return TrickResult {
                    no_winner: false,
                    winning_player,
                };
            }
        }

        // No trump cards (or no trump suit) - find highest lead suit card
        let lead_cards: Vec<Card> = eligible_cards
            .iter()
            .filter(|c| c.suit == lead_suit)
            .copied()
            .collect();

        if !lead_cards.is_empty() {
            // Find highest lead suit card
            let winning_card = lead_cards.iter().max_by_key(|c| c.value).unwrap();
            let winning_player = *card_id_to_player.get(&winning_card.id).unwrap();
            return TrickResult {
                no_winner: false,
                winning_player,
            };
        }

        // No trump and no lead suit cards remain - no winner
        TrickResult {
            no_winner: true,
            winning_player: lead_player,
        }
    }

    fn non_matching_cards(cards: [Option<Card>; PLAYER_COUNT]) -> Vec<Card> {
        // Count cards by (value, suit) pair to detect ties
        let mut value_suit_count: HashMap<(i32, Suit), Vec<Card>> = HashMap::new();
        for card in cards {
            let card = card.unwrap();
            value_suit_count
                .entry((card.value, card.suit))
                .or_default()
                .push(card);
        }

        // Only return cards that appear once (not tied)
        value_suit_count
            .into_iter()
            .filter(|(_, cards)| cards.len() == 1)
            .flat_map(|(_, cards)| cards)
            .collect()
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
            match self.state {
                State::SelectBidOrPass => {
                    // During bid selection, highlight cards in play area and show pass button
                    for id in moves {
                        if id == PASS {
                            // Show pass button
                            self.add_change(
                                change_index,
                                Change {
                                    object_id: PASS,
                                    change_type: ChangeType::SelectBidCardOrPass,
                                    dest: Location::Play,
                                    player: self.current_player,
                                    ..Default::default()
                                },
                            );
                        } else {
                            // Highlight biddable cards in play area
                            self.add_change(
                                change_index,
                                Change {
                                    object_id: id,
                                    change_type: ChangeType::ShowPlayable,
                                    dest: Location::Play,
                                    player: self.current_player,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
                State::Play => {
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
                }
                State::GameOver => {}
            }
        } else {
            self.hide_playable();
        }
    }

    fn show_message(&mut self) {
        let message = match self.state {
            State::SelectBidOrPass => {
                if self.current_player == 0 {
                    Some("Select a bid card or pass".to_string())
                } else {
                    let player_name = match self.current_player {
                        1 => "West",
                        2 => "North",
                        _ => "East",
                    };
                    Some(format!("{} must select a bid or pass", player_name))
                }
            }
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

    pub fn calculate_score(bid_card: Option<Card>, tricks_won: i32) -> i32 {
        // If player didn't take a bid card, they score -1
        if bid_card.is_none() {
            return -1;
        }

        let bid = bid_card.unwrap().value;
        let diff = (tricks_won - bid).abs();

        if diff == 0 {
            // Exact bid: 3 points
            3
        } else if diff == 1 {
            // Off by 1: 1 point
            1
        } else {
            // Otherwise: 0 points
            0
        }
    }
}

impl ismcts::Game for TrickOrBidGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        // In Trick or Bid:
        // - Bid cards are PUBLIC (kept face up) - don't shuffle them
        // - Only shuffle unknown cards in other players' hands
        // - Observer knows their own hand, all bid cards, and revealed voids
        // - Voids are revealed when a player doesn't follow suit

        for p1 in 0..PLAYER_COUNT {
            for p2 in 0..PLAYER_COUNT {
                if p1 == p2 {
                    continue;
                }
                if p1 == observer || p2 == observer {
                    // Don't swap observer's cards - they know exactly what they have
                    continue;
                }

                // Build combined void set for both players
                let mut combined_voids = [false; 4];
                for suit in &self.voids[p1] {
                    combined_voids[*suit as usize] = true;
                }
                for suit in &self.voids[p2] {
                    combined_voids[*suit as usize] = true;
                }

                let mut new_hands = vec![self.hands[p1].clone(), self.hands[p2].clone()];

                // Only shuffle cards that aren't in the combined void set
                shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids[c.suit as usize],
                    &mut new_hands,
                    rng,
                );

                self.hands[p1] = new_hands[0].clone();
                self.hands[p2] = new_hands[1].clone();
            }
        }

        // Bid cards and trump suit are public knowledge - don't modify them
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player + 1) % PLAYER_COUNT
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
            let scores = self.scores;
            let player_score = scores[player];
            let max_score = *scores.iter().max().unwrap();

            // Winner-takes-all reward function (based on experiments showing 64% win rate vs 59% for exponential)
            // Simple binary outcome: win = 1.0, lose = -1.0, tie = 0.0
            if player_score == max_score {
                // Check if there are multiple players with the max score (tie)
                if scores.iter().filter(|&&s| s == player_score).count() > 1 {
                    Some(0.0) // Tie
                } else {
                    Some(1.0) // Win
                }
            } else {
                Some(-1.0) // Lose
            }
        }
    }
}

pub fn get_mcts_move(game: &TrickOrBidGame, iterations: i32, _debug: bool) -> i32 {
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
    use ismcts::Game;

    #[test]
    fn test_new() {
        let game = TrickOrBidGame::new();
        assert!(
            game.hands.iter().all(|h| h.len() == HAND_SIZE),
            "Every player should have 13 cards in their hand"
        );
        assert_eq!(game.state, State::Play, "Game starts in Play state");
        assert_eq!(game.round, 1, "The game starts in round 1");
    }

    // Test trick winner with no trump - lead suit wins
    #[test]
    fn test_trick_winner_no_trump_highest_lead_suit() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 3,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Purple,
                value: 7,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Purple,
                value: 2,
            }),
        ];
        let result = TrickOrBidGame::trick_winner(0, hand, None);
        assert!(!result.no_winner);
        assert_eq!(
            result.winning_player, 1,
            "Player 1 has highest lead suit card"
        );
    }

    // Test trump beats lead suit
    #[test]
    fn test_trick_winner_trump_beats_lead_suit() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 9,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 1,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Purple,
                value: 7,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Purple,
                value: 5,
            }),
        ];
        let result = TrickOrBidGame::trick_winner(0, hand, Some(Suit::Green));
        assert!(!result.no_winner);
        assert_eq!(result.winning_player, 1, "Trump beats lead suit");
    }

    // Test highest trump wins
    #[test]
    fn test_trick_winner_highest_trump() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 9,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 2,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Green,
                value: 5,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Orange,
                value: 8,
            }),
        ];
        let result = TrickOrBidGame::trick_winner(0, hand, Some(Suit::Green));
        assert!(!result.no_winner);
        assert_eq!(result.winning_player, 2, "Player 2 has highest trump");
    }

    // Test ties cancel out
    #[test]
    fn test_trick_winner_ties_cancel_out() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 3,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Purple,
                value: 1,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Purple,
                value: 2,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Purple,
                value: 3,
            }),
        ];
        let result = TrickOrBidGame::trick_winner(0, hand, None);
        assert!(!result.no_winner);
        assert_eq!(result.winning_player, 2, "Two 3s cancel, 2 wins");
    }

    // Test all cards tied - no winner (all same value AND suit)
    #[test]
    fn test_trick_winner_all_tied() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 2,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Purple,
                value: 2,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Purple,
                value: 2,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Purple,
                value: 2,
            }),
        ];
        let result = TrickOrBidGame::trick_winner(0, hand, None);
        assert!(
            result.no_winner,
            "All cards tied should result in no winner"
        );
        assert_eq!(result.winning_player, 0, "Lead player retained");
    }

    // Test scoring: exact bid = 3 points
    #[test]
    fn test_calculate_score_exact_bid() {
        let bid_card = Some(Card {
            id: 0,
            suit: Suit::Purple,
            value: 5,
        });
        let score = TrickOrBidGame::calculate_score(bid_card, 5);
        assert_eq!(score, 3);
    }

    // Test scoring: off by 1 = 1 point
    #[test]
    fn test_calculate_score_off_by_one() {
        let bid_card = Some(Card {
            id: 0,
            suit: Suit::Purple,
            value: 4,
        });
        let score = TrickOrBidGame::calculate_score(bid_card, 5);
        assert_eq!(score, 1);

        let score = TrickOrBidGame::calculate_score(bid_card, 3);
        assert_eq!(score, 1);
    }

    // Test scoring: off by 2+ = 0 points
    #[test]
    fn test_calculate_score_off_by_two_or_more() {
        let bid_card = Some(Card {
            id: 0,
            suit: Suit::Purple,
            value: 3,
        });
        let score = TrickOrBidGame::calculate_score(bid_card, 5);
        assert_eq!(score, 0);
    }

    // Test scoring: no bid = -1 point
    #[test]
    fn test_calculate_score_no_bid() {
        let score = TrickOrBidGame::calculate_score(None, 5);
        assert_eq!(score, -1);
    }

    // Test scoring: bid 0 exact = 3 points
    #[test]
    fn test_calculate_score_bid_zero_exact() {
        let bid_card = Some(Card {
            id: 0,
            suit: Suit::Purple,
            value: 0,
        });
        let score = TrickOrBidGame::calculate_score(bid_card, 0);
        assert_eq!(score, 3);
    }

    // Test non_matching_cards with all different
    #[test]
    fn test_non_matching_cards_all_different() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 1,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 2,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 3,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 4,
            }),
        ];
        let result = TrickOrBidGame::non_matching_cards(hand);
        assert_eq!(result.len(), 4);
    }

    // Test non_matching_cards with ties (same value AND suit)
    #[test]
    fn test_non_matching_cards_with_ties() {
        let hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 2,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Purple,
                value: 2,
            }), // ties with id 0
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 4,
            }),
        ];
        let result = TrickOrBidGame::non_matching_cards(hand);
        assert_eq!(
            result.len(),
            2,
            "Two Purple 2s cancel out, leaving Green 3 and Red 4"
        );
    }

    // Test deck has correct composition
    #[test]
    fn test_deck_composition() {
        let deck = TrickOrBidGame::deck();
        assert_eq!(deck.len(), 52);

        let mut value_counts = std::collections::HashMap::new();
        for card in &deck {
            *value_counts.entry(card.value).or_insert(0) += 1;
        }

        // Verify expected counts: 0,1,1,2,2,3,3,4,5,6,7,8,9 per suit
        assert_eq!(*value_counts.get(&0).unwrap(), 4);
        assert_eq!(*value_counts.get(&1).unwrap(), 8);
        assert_eq!(*value_counts.get(&2).unwrap(), 8);
        assert_eq!(*value_counts.get(&3).unwrap(), 8);
        assert_eq!(*value_counts.get(&4).unwrap(), 4);
        assert_eq!(*value_counts.get(&5).unwrap(), 4);
    }

    // Integration test: Full trick playthrough with bid selection
    #[test]
    fn test_play_trick_and_select_bid() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        // Set up a known hand for testing
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Purple,
                value: 3,
            },
        ];
        game.hands[1] = vec![
            Card {
                id: 2,
                suit: Suit::Purple,
                value: 7,
            },
            Card {
                id: 3,
                suit: Suit::Green,
                value: 2,
            },
        ];
        game.hands[2] = vec![
            Card {
                id: 4,
                suit: Suit::Purple,
                value: 4,
            },
            Card {
                id: 5,
                suit: Suit::Orange,
                value: 1,
            },
        ];
        game.hands[3] = vec![
            Card {
                id: 6,
                suit: Suit::Purple,
                value: 2,
            },
            Card {
                id: 7,
                suit: Suit::Black,
                value: 8,
            },
        ];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 0;

        // Player 0 leads with Purple 5
        game.apply_move(0);
        assert_eq!(game.current_player, 1);
        assert_eq!(game.state, State::Play);

        // Player 1 plays Purple 7 (highest card)
        game.apply_move(2);
        assert_eq!(game.current_player, 2);

        // Player 2 plays Purple 4
        game.apply_move(4);
        assert_eq!(game.current_player, 3);

        // Player 3 plays Purple 2 (completes trick)
        game.apply_move(6);

        // Player 1 should win (Purple 7 is highest)
        assert_eq!(game.current_player, 1, "Player 1 won the trick");
        assert_eq!(game.state, State::SelectBidOrPass);
        // current_hand contains the trick cards for bid selection
        assert!(
            game.current_hand.iter().all(|c| c.is_some()),
            "current_hand should contain 4 cards for bid selection"
        );

        // Player 1 cannot bid with their own card (Purple 7, id 2)
        // They must select a card from the trick that they didn't play
        // Player 1 selects Purple 5 (id 0) as bid card
        game.apply_move(0); // card id 0 is Purple 5 (played by player 0)

        assert_eq!(
            game.bid_cards[1],
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5
            })
        );
        assert_eq!(
            game.trump_suit,
            Some(Suit::Purple),
            "Trump suit established"
        );
        // When selecting a bid card, the trick doesn't count and other cards are discarded
        assert_eq!(
            game.tricks_won_count[1], 0,
            "Bid selection doesn't count as trick won"
        );
        assert_eq!(
            game.cards_won[1].len(),
            0,
            "Other cards are discarded when bidding"
        );
    }

    // Test passing on bid selection
    #[test]
    fn test_pass_on_bid_selection() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.state = State::SelectBidOrPass;
        game.current_player = 0;
        // current_hand contains the trick cards for bid selection
        game.current_hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 2,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 4,
            }),
        ];
        game.hands[0] = vec![Card {
            id: 10,
            suit: Suit::Purple,
            value: 9,
        }];

        // Player passes (takes trick without bidding)
        game.apply_move(PASS);

        assert_eq!(game.tricks_won_count[0], 1);
        assert_eq!(game.cards_won[0].len(), 4);
        assert_eq!(game.bid_cards[0], None);
        assert_eq!(game.state, State::Play);
    }

    // Test accumulated tricks (no winner)
    #[test]
    fn test_accumulated_tricks_no_winner() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        // Set up a trick where all cards tie (all same value and suit)
        game.hands[0] = vec![Card {
            id: 0,
            suit: Suit::Purple,
            value: 3,
        }];
        game.hands[1] = vec![Card {
            id: 1,
            suit: Suit::Purple,
            value: 3,
        }];
        game.hands[2] = vec![Card {
            id: 2,
            suit: Suit::Purple,
            value: 3,
        }];
        game.hands[3] = vec![Card {
            id: 3,
            suit: Suit::Purple,
            value: 3,
        }];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 0;

        // Play all cards
        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Last trick with no winner - cards are discarded per rules
        assert_eq!(
            game.accumulated_tricks.len(),
            0,
            "Last trick with no winner is discarded"
        );
        // Round ends and new round is dealt (since not at round 4 yet)
        assert_eq!(
            game.state,
            State::Play,
            "Last trick with no winner ends round and deals new hand"
        );
        assert_eq!(game.round, 2, "Round incremented after end_hand");
        assert!(
            game.hands.iter().all(|h| h.len() == HAND_SIZE),
            "New round dealt"
        );
    }

    // Test trump beats lead suit in play
    #[test]
    fn test_trump_beats_lead_in_play() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;
        game.trump_suit = Some(Suit::Green);

        game.hands[0] = vec![Card {
            id: 0,
            suit: Suit::Purple,
            value: 9,
        }];
        game.hands[1] = vec![Card {
            id: 1,
            suit: Suit::Green,
            value: 1,
        }]; // trump
        game.hands[2] = vec![Card {
            id: 2,
            suit: Suit::Purple,
            value: 8,
        }];
        game.hands[3] = vec![Card {
            id: 3,
            suit: Suit::Purple,
            value: 7,
        }];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 0;

        game.apply_move(0); // Purple 9 (lead)
        game.apply_move(1); // Green 1 (trump)
        game.apply_move(2); // Purple 8
        game.apply_move(3); // Purple 7

        // Player 1 should win with trump
        assert_eq!(game.current_player, 1);
        assert_eq!(game.state, State::SelectBidOrPass);
    }

    // Test must-follow rules
    #[test]
    fn test_must_follow_suit() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            },
            Card {
                id: 2,
                suit: Suit::Purple,
                value: 2,
            },
        ];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 1;
        game.current_hand[1] = Some(Card {
            id: 10,
            suit: Suit::Purple,
            value: 8,
        });

        // Must follow Purple suit
        let moves = game.get_moves();
        assert_eq!(moves.len(), 2, "Only Purple cards should be playable");
        assert!(moves.contains(&0));
        assert!(moves.contains(&2));
        assert!(!moves.contains(&1), "Green card should not be playable");
    }

    // Test can play any card when can't follow suit
    #[test]
    fn test_cannot_follow_suit_play_any() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Green,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Orange,
                value: 3,
            },
            Card {
                id: 2,
                suit: Suit::Black,
                value: 2,
            },
        ];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 1;
        game.current_hand[1] = Some(Card {
            id: 10,
            suit: Suit::Purple,
            value: 8,
        });

        // Can't follow Purple, so can play any card
        let moves = game.get_moves();
        assert_eq!(moves.len(), 3, "All cards should be playable");
    }

    // Test get_moves in SelectBidOrPass state
    #[test]
    fn test_get_moves_bid_or_pass() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.state = State::SelectBidOrPass;
        game.current_player = 0;
        // current_hand contains the trick cards for bid selection
        // Player 0's card is at index 0, which should be excluded from bid options
        game.current_hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 2,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 4,
            }),
        ];

        let moves = game.get_moves();
        // Should have PASS + 3 cards (excluding player 0's own card at index 0)
        assert_eq!(moves.len(), 4, "Should have PASS + 3 cards");
        assert!(moves.contains(&PASS));
        assert!(!moves.contains(&0), "Cannot bid with own card");
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
        assert!(moves.contains(&3));
    }

    // Test end of round scoring
    #[test]
    fn test_end_round_scoring() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        // Set up end of round
        game.round = 1;
        game.hands = [vec![], vec![], vec![], vec![]];
        game.state = State::SelectBidOrPass;

        // Set bid cards and tricks won
        game.bid_cards = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }), // bid 5
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }), // bid 3
            None, // no bid
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 2,
            }), // bid 2
        ];

        game.tricks_won_count = [5, 4, 7, 2]; // P0: exact, P1: off by 1, P2: no bid, P3: exact
        game.scores = [0, 0, 0, 0];

        game.end_hand();

        // Check scores
        assert_eq!(game.scores[0], 3, "Player 0: exact bid = 3 points");
        assert_eq!(game.scores[1], 1, "Player 1: off by 1 = 1 point");
        assert_eq!(game.scores[2], -1, "Player 2: no bid = -1 point");
        assert_eq!(game.scores[3], 3, "Player 3: exact bid = 3 points");

        // Should deal new round
        assert_eq!(game.round, 2);
        assert!(game.hands.iter().all(|h| h.len() == HAND_SIZE));
    }

    // Test game over after final round
    #[test]
    fn test_game_over() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.round = PLAYER_COUNT as i32; // Final round
        game.hands = [vec![], vec![], vec![], vec![]];
        game.state = State::SelectBidOrPass;

        game.bid_cards = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 4,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 2,
            }),
        ];

        game.tricks_won_count = [5, 3, 2, 1];
        game.scores = [10, 15, 5, 8]; // Player 1 has highest

        game.end_hand();

        assert_eq!(game.state, State::GameOver);
        assert_eq!(
            game.winner,
            Some(1),
            "Player 1 should win with highest score"
        );
    }

    // Test accumulated tricks go to next winner
    #[test]
    fn test_accumulated_tricks_to_next_winner() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        // First trick: all tie (no winner)
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Purple,
                value: 2,
            },
            Card {
                id: 8,
                suit: Suit::Purple,
                value: 9,
            },
        ];
        game.hands[1] = vec![
            Card {
                id: 1,
                suit: Suit::Purple,
                value: 2,
            },
            Card {
                id: 9,
                suit: Suit::Purple,
                value: 3,
            },
        ];
        game.hands[2] = vec![
            Card {
                id: 2,
                suit: Suit::Purple,
                value: 2,
            },
            Card {
                id: 10,
                suit: Suit::Purple,
                value: 4,
            },
        ];
        game.hands[3] = vec![
            Card {
                id: 3,
                suit: Suit::Purple,
                value: 2,
            },
            Card {
                id: 11,
                suit: Suit::Purple,
                value: 5,
            },
        ];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 0;

        // First trick - all tie
        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        assert_eq!(
            game.accumulated_tricks.len(),
            PLAYER_COUNT,
            "No-winner tricks go to accumulated_tricks"
        );
        assert_eq!(game.state, State::Play, "Continue playing after no winner");
        assert_eq!(game.current_player, 0, "Lead player stays same");

        // Second trick - player 0 wins with Purple 9 (highest card)
        game.apply_move(8); // Purple 9
        game.apply_move(9); // Purple 3
        game.apply_move(10); // Purple 4
        game.apply_move(11); // Purple 5

        assert_eq!(game.current_player, 0, "Player 0 wins with Purple 9");
        assert_eq!(game.state, State::SelectBidOrPass);
        // Accumulated no-winner tricks go directly to winner's score pile
        assert_eq!(
            game.accumulated_tricks.len(),
            0,
            "accumulated_tricks cleared when winner takes them"
        );
        assert_eq!(
            game.tricks_won_count[0], 1,
            "Player 0 got 1 trick from the no-winner trick"
        );
        assert_eq!(
            game.cards_won[0].len(),
            PLAYER_COUNT,
            "Player 0 has 4 cards from the no-winner trick"
        );
        // current_hand contains the current trick for bid selection
        assert!(
            game.current_hand.iter().all(|c| c.is_some()),
            "current_hand should contain 4 cards for bid selection"
        );
    }

    // Test playable_card_ids when leading
    #[test]
    fn test_playable_when_leading() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            },
            Card {
                id: 2,
                suit: Suit::Orange,
                value: 2,
            },
        ];

        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 0;
        game.current_hand = [None; PLAYER_COUNT];

        let moves = game.playable_card_ids();
        assert_eq!(moves.len(), 3, "All cards playable when leading");
    }

    // Test first bid card establishes trump
    #[test]
    fn test_first_bid_establishes_trump() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.state = State::SelectBidOrPass;
        game.current_player = 0;
        game.trump_suit = None;
        // current_hand contains the trick cards
        // Player 0's card is at index 0, so they cannot bid with it
        game.current_hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 3,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 5,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 2,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 4,
            }),
        ];
        game.hands[0] = vec![];

        // Select Green 5 (id 1) as bid - this is not player 0's card
        game.apply_move(1);

        assert_eq!(game.trump_suit, Some(Suit::Green));
        assert_eq!(game.bid_cards[0].unwrap().suit, Suit::Green);
    }

    // Test second bid doesn't change trump
    #[test]
    fn test_second_bid_keeps_trump() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.state = State::SelectBidOrPass;
        game.current_player = 1;
        game.trump_suit = Some(Suit::Green); // Already established
                                             // current_hand contains the trick cards
                                             // Player 1's card is at index 1, so they cannot bid with id 1
        game.current_hand = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Orange,
                value: 3,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Black,
                value: 2,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Green,
                value: 4,
            }),
        ];
        game.hands[1] = vec![];

        // Select Purple 5 (id 0) as bid - this is not player 1's card
        game.apply_move(0);

        assert_eq!(
            game.trump_suit,
            Some(Suit::Green),
            "Trump should remain Green"
        );
        assert_eq!(game.bid_cards[1].unwrap().suit, Suit::Purple);
    }

    // Test round progression
    #[test]
    fn test_round_progression() {
        let mut game = TrickOrBidGame::new();
        assert_eq!(game.round, 1);

        let initial_dealer = game.dealer;

        // Simulate end of round
        game.no_changes = true;
        game.hands = [vec![], vec![], vec![], vec![]];
        game.state = State::SelectBidOrPass;
        game.bid_cards = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 4,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 2,
            }),
        ];
        game.tricks_won_count = [5, 3, 4, 2];

        game.end_hand();

        assert_eq!(game.round, 2);
        assert_eq!(game.dealer, (initial_dealer + 1) % PLAYER_COUNT);
        assert_eq!(game.state, State::Play);
    }

    // Test randomize_determination keeps bid cards public
    #[test]
    fn test_randomize_determination_keeps_bid_cards() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        // Set up a game state with bid cards
        game.bid_cards = [
            Some(Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Green,
                value: 3,
            }),
            None,
            Some(Card {
                id: 3,
                suit: Suit::Black,
                value: 2,
            }),
        ];
        game.trump_suit = Some(Suit::Purple);

        let original_bid_cards = game.bid_cards;
        let original_trump = game.trump_suit;

        // Randomize from player 0's perspective
        game.randomize_determination(0);

        // Bid cards and trump should remain unchanged (they're public)
        assert_eq!(
            game.bid_cards, original_bid_cards,
            "Bid cards are public and should not change"
        );
        assert_eq!(
            game.trump_suit, original_trump,
            "Trump suit is public and should not change"
        );
    }

    // Test randomize_determination preserves observer's hand
    #[test]
    fn test_randomize_determination_preserves_observer_hand() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Purple,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Purple,
                value: 3,
            },
        ];
        game.hands[1] = vec![
            Card {
                id: 2,
                suit: Suit::Green,
                value: 7,
            },
            Card {
                id: 3,
                suit: Suit::Green,
                value: 2,
            },
        ];

        let original_hand = game.hands[0].clone();

        // Randomize from player 0's perspective
        game.randomize_determination(0);

        // Player 0's hand should remain unchanged
        assert_eq!(
            game.hands[0], original_hand,
            "Observer's hand should not change"
        );
    }

    // Test accumulated tricks counting: accumulated_tricks are awarded in play() when winner
    // is determined, current_hand is used for bid selection (pass = take current trick)
    #[test]
    fn test_accumulated_tricks_counting() {
        let mut game = TrickOrBidGame::new();
        game.no_changes = true;

        // Set up current_hand with the current winning trick (4 cards)
        // (accumulated_tricks would have been awarded already in play() before SelectBidOrPass)
        game.current_hand = [
            Some(Card {
                id: 8,
                suit: Suit::Purple,
                value: 9,
            }),
            Some(Card {
                id: 9,
                suit: Suit::Purple,
                value: 1,
            }),
            Some(Card {
                id: 10,
                suit: Suit::Purple,
                value: 0,
            }),
            Some(Card {
                id: 11,
                suit: Suit::Purple,
                value: 2,
            }),
        ];

        game.current_player = 0;
        game.state = State::SelectBidOrPass;

        // Player passes - takes the current trick (1 trick from current_hand)
        game.select_bid_card_or_pass(PASS);

        // Verify tricks_won_count is 1 (just the current trick)
        assert_eq!(
            game.tricks_won_count[0], 1,
            "Player should have won 1 trick from current_hand"
        );

        // Verify current_hand cards were moved to cards_won
        assert_eq!(
            game.cards_won[0].len(),
            4,
            "Current trick (4 cards) should be in player's cards_won"
        );

        // Verify current_hand is now empty
        assert!(
            game.current_hand.iter().all(|c| c.is_none()),
            "Current hand should be cleared after passing"
        );
    }
}
