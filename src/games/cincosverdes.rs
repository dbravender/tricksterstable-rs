/*
Game: Cincos Verdes (The Green Fivura / Fives)
Designer: Taiki Shinzawa
*/

use enum_iterator::Sequence;
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

const PLAYER_COUNT: usize = 4;
const HAND_SIZE: usize = 13;
const TARGET_SUM: i32 = 25;
const ROUNDS: usize = 4;
const STARTING_POINTS: i32 = 4;

// Special move values for playing cards face-down as green 5
const FACE_DOWN_OFFSET: i32 = 100; // card_id + FACE_DOWN_OFFSET = play face down
const UNDO: i32 = -2; // Undo card selection (human player only)
const GREEN_FIVE_OPTION: i32 = -5; // Option to play selected card as green 5

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
    Orange = 0,
    Purple = 1, // Trump suit
    Pink = 2,
    Green = 3,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i32,
    pub suit: Suit,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Play,
    SelectPlayStyle, // Player selected a card that can be played face-up or face-down
    GameOver,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Hand,
    Score,
    Message,
    Play,
    TricksTaken,
    ReorderHand,
    ScoreCards,
    Preview,      // Card selected but not yet played - shown in preview position
    Green5Option, // Location for the green 5 option button/card
    UndoOption,   // Location for the undo button
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
    TricksToWinner,
    Reorder,
    ShowScoringCards,
    UpdateTrickCount,
    PlayFaceDown,   // Play a card face-down as green 5
    UpdateTrickSum, // Show trick sum change after a trick
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
    pub animate_score: bool,
    pub trick_count: i32,
    pub face_down: bool, // True when card is played face-down as green 5
    pub disabled: bool,  // True when option is shown but not selectable
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct CincosVerdesGame {
    pub hands: [Vec<Card>; PLAYER_COUNT],
    pub state: State,
    pub changes: Vec<Vec<Change>>,
    pub no_changes: bool,
    pub scores: [i32; PLAYER_COUNT],
    pub winner: Option<usize>,
    pub current_player: usize,
    pub lead_player: usize,
    pub dealer: usize,
    pub current_trick: [Option<Card>; PLAYER_COUNT],
    pub played_face_down: [bool; PLAYER_COUNT], // Which cards in current_trick are face-down (green 5)
    pub round: i32,
    pub num_players: usize,
    pub trick_sums: [i32; PLAYER_COUNT], // Sum of winning card values this round
    pub carry_over_bonus: i32,           // Bonus carried over when tied for first
    pub voids: [Vec<Suit>; PLAYER_COUNT], // Known voids for determination
    pub green_five_played: bool,         // Track if green 5 already played this trick
    pub selected_card: Option<Card>,     // Card selected by human player (for play style choice)
    pub green_five_available: bool,      // Whether selected card can be played as green 5
    pub face_down_cards: [Vec<Card>; PLAYER_COUNT], // Cards played face-down per player (hidden info for ISMCTS)
}

impl CincosVerdesGame {
    pub fn new() -> Self {
        let mut game = CincosVerdesGame {
            scores: [STARTING_POINTS; PLAYER_COUNT],
            num_players: PLAYER_COUNT,
            ..Default::default()
        };
        game.dealer = thread_rng().gen_range(0..PLAYER_COUNT);
        game.deal(true);
        game
    }

    fn deal(&mut self, animate: bool) {
        let mut deck = CincosVerdesGame::deck();
        self.trick_sums = [0; PLAYER_COUNT];
        self.current_trick = [None; PLAYER_COUNT];
        self.played_face_down = [false; PLAYER_COUNT];
        self.green_five_played = false;
        self.voids = [vec![], vec![], vec![], vec![]];
        self.face_down_cards = [vec![], vec![], vec![], vec![]];
        self.state = State::Play;
        self.dealer = (self.dealer + 1) % PLAYER_COUNT;

        self.hands = [
            deck.drain(..HAND_SIZE).collect::<Vec<_>>(),
            deck.drain(..HAND_SIZE).collect::<Vec<_>>(),
            deck.drain(..HAND_SIZE).collect::<Vec<_>>(),
            deck,
        ];

        // Find who has green 0 - they start
        let green_zero_holder = self.find_green_zero_holder();
        self.current_player = green_zero_holder;
        self.lead_player = green_zero_holder;

        self.sort_hand(0);

        if !animate {
            return;
        }

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
        self.show_playable();
        self.show_message();
    }

    fn find_green_zero_holder(&self) -> usize {
        for (player, hand) in self.hands.iter().enumerate() {
            if hand.iter().any(|c| c.suit == Suit::Green && c.value == 0) {
                return player;
            }
        }
        0 // Fallback
    }

    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        // Orange, Purple, Pink: 1-13 each
        for suit in [Suit::Orange, Suit::Purple, Suit::Pink] {
            for value in 1..=13 {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        // Green: 0-4, 6-13 (no 5 - it's on card backs)
        for value in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13] {
            deck.push(Card {
                id,
                value,
                suit: Suit::Green,
            });
            id += 1;
        }

        deck.shuffle(&mut thread_rng());
        deck
    }

    pub fn get_moves(&self) -> Vec<i32> {
        match self.state {
            State::Play => self.playable_moves(),
            State::SelectPlayStyle => {
                // Player has selected a card - show play options
                let mut moves = vec![self.selected_card.unwrap().id, UNDO];
                // Only include green 5 option if it's actually available
                if self.green_five_available {
                    moves.push(GREEN_FIVE_OPTION);
                }
                moves
            }
            State::GameOver => vec![],
        }
    }

    fn playable_moves(&self) -> Vec<i32> {
        let mut moves = Vec::new();
        let hand = &self.hands[self.current_player];

        // Check if we need to follow suit
        let must_follow = if let Some(lead_card) = self.current_trick[self.lead_player] {
            let lead_suit = if self.played_face_down[self.lead_player] {
                Suit::Green // Face-down card is green 5
            } else {
                lead_card.suit
            };
            // Check if player has cards of lead suit
            hand.iter().any(|c| c.suit == lead_suit)
        } else {
            false // Leading, can play anything
        };

        for card in hand {
            // Can always play the card face-up if we're leading or following correctly
            if !must_follow {
                moves.push(card.id);
            } else {
                let lead_card = self.current_trick[self.lead_player].unwrap();
                let lead_suit = if self.played_face_down[self.lead_player] {
                    Suit::Green
                } else {
                    lead_card.suit
                };

                if card.suit == lead_suit {
                    moves.push(card.id);
                }
            }

            // Can play face-down as green 5 in these cases:
            // 1. Only one green 5 per trick (check green_five_played)
            // 2. If leading, can play any card face down
            // 3. If lead is green and we have green cards, we MAY play one face down
            // 4. If we don't have lead suit, we can play any card face down
            // 5. If we have exactly one card of lead suit, we can play it face down
            if !self.green_five_played {
                let lead_card_opt = self.current_trick[self.lead_player];

                if lead_card_opt.is_none() {
                    // Leading - can play any card face down
                    moves.push(card.id + FACE_DOWN_OFFSET);
                } else {
                    let lead_suit = if self.played_face_down[self.lead_player] {
                        Suit::Green
                    } else {
                        lead_card_opt.unwrap().suit
                    };

                    if lead_suit == Suit::Green {
                        // Lead is green - can play green 5 even if we have other green cards
                        // But we're not obligated to if we have no green
                        if card.suit == Suit::Green {
                            moves.push(card.id + FACE_DOWN_OFFSET);
                        } else if !hand.iter().any(|c| c.suit == Suit::Green) {
                            // No green cards - can play any card face down
                            moves.push(card.id + FACE_DOWN_OFFSET);
                        }
                    } else {
                        // Lead is not green
                        if !hand.iter().any(|c| c.suit == lead_suit) {
                            // Can't follow suit - can play any card face down
                            moves.push(card.id + FACE_DOWN_OFFSET);
                        } else {
                            // Has lead suit cards
                            let lead_suit_count =
                                hand.iter().filter(|c| c.suit == lead_suit).count();
                            if lead_suit_count == 1 && card.suit == lead_suit {
                                // Exactly one card of lead suit - can play it face up or face down
                                moves.push(card.id + FACE_DOWN_OFFSET);
                            }
                        }
                    }
                }
            }
        }

        // Remove duplicates
        moves.sort();
        moves.dedup();
        moves
    }

    pub fn apply_move(&mut self, mov: i32) {
        self.changes = vec![vec![]];

        if !self.get_moves().contains(&mov) {
            panic!("invalid move: {}", mov);
        }

        match self.state {
            State::GameOver => return,
            State::SelectPlayStyle => self.apply_play_style_selection(mov),
            State::Play => {
                // For human player (player 0), always show play selection UI
                // But not during AI simulation (when no_changes is true)
                if self.current_player == 0 && !self.no_changes {
                    let face_down_move = mov + FACE_DOWN_OFFSET;
                    let all_moves = self.all_playable_moves();
                    // Track if green 5 option is available for this card
                    self.green_five_available = all_moves.contains(&face_down_move);
                    // Always go to selection state for human player
                    self.select_card_for_play_style(mov);
                    self.show_playable();
                    self.show_message();
                    return;
                }
                self.play(mov);
            }
        }

        self.show_playable();
        self.show_message();
    }

    fn select_card_for_play_style(&mut self, card_id: i32) {
        // Remove the card from hand and store it
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == card_id)
            .expect("Card not found in hand");
        let card = self.hands[self.current_player].remove(pos);

        self.selected_card = Some(card);
        self.state = State::SelectPlayStyle;

        // Animate card to preview position
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Play,
                dest: Location::Preview,
                object_id: card_id,
                player: self.current_player,
                ..Default::default()
            },
        );

        // Reorder remaining cards to fill the gap
        self.reorder_hand(self.current_player, false);
    }

    fn apply_play_style_selection(&mut self, mov: i32) {
        let card = self.selected_card.expect("No card selected");

        if mov == UNDO {
            // Return card to hand and hide the option cards
            self.hands[self.current_player].push(card);
            self.sort_hand(self.current_player);
            self.selected_card = None;
            self.state = State::Play;
            self.hide_playable();
            self.reorder_hand(self.current_player, true);
            return;
        }

        // Hide the option cards before playing
        self.hide_playable();

        // Add card back to hand before calling play (which will pop it)
        self.hands[self.current_player].push(card);

        if mov == GREEN_FIVE_OPTION {
            // Play face-down as green 5
            self.selected_card = None;
            self.state = State::Play;
            self.play(card.id + FACE_DOWN_OFFSET);
            return;
        }

        // Play face-up (mov should be card.id)
        self.selected_card = None;
        self.state = State::Play;
        self.play(card.id);
    }

    /// Returns all playable moves including face-down options (used internally)
    fn all_playable_moves(&self) -> Vec<i32> {
        let mut moves = Vec::new();
        let hand = &self.hands[self.current_player];

        let must_follow = if let Some(lead_card) = self.current_trick[self.lead_player] {
            let lead_suit = if self.played_face_down[self.lead_player] {
                Suit::Green
            } else {
                lead_card.suit
            };
            hand.iter().any(|c| c.suit == lead_suit)
        } else {
            false
        };

        for card in hand {
            if !must_follow {
                moves.push(card.id);
            } else {
                let lead_card = self.current_trick[self.lead_player].unwrap();
                let lead_suit = if self.played_face_down[self.lead_player] {
                    Suit::Green
                } else {
                    lead_card.suit
                };

                if card.suit == lead_suit {
                    moves.push(card.id);
                }
            }

            if !self.green_five_played {
                let lead_card_opt = self.current_trick[self.lead_player];

                if lead_card_opt.is_none() {
                    moves.push(card.id + FACE_DOWN_OFFSET);
                } else {
                    let lead_suit = if self.played_face_down[self.lead_player] {
                        Suit::Green
                    } else {
                        lead_card_opt.unwrap().suit
                    };

                    if lead_suit == Suit::Green {
                        if card.suit == Suit::Green || !hand.iter().any(|c| c.suit == Suit::Green) {
                            moves.push(card.id + FACE_DOWN_OFFSET);
                        }
                    } else if !hand.iter().any(|c| c.suit == lead_suit) {
                        moves.push(card.id + FACE_DOWN_OFFSET);
                    } else {
                        let lead_suit_count = hand.iter().filter(|c| c.suit == lead_suit).count();
                        if lead_suit_count == 1 && card.suit == lead_suit {
                            moves.push(card.id + FACE_DOWN_OFFSET);
                        }
                    }
                }
            }
        }

        moves.sort();
        moves.dedup();
        moves
    }

    fn play(&mut self, mov: i32) {
        let face_down = mov >= FACE_DOWN_OFFSET;
        let card_id = if face_down {
            mov - FACE_DOWN_OFFSET
        } else {
            mov
        };

        let card = self.pop_card(card_id);
        let player = self.current_player;

        // Mark if green 5 was played this trick and track the card for ISMCTS
        if face_down {
            self.green_five_played = true;
            self.face_down_cards[player].push(card);
        }

        // Animate card play
        self.add_change(
            0,
            Change {
                change_type: if face_down {
                    ChangeType::PlayFaceDown
                } else {
                    ChangeType::Play
                },
                object_id: card_id,
                dest: Location::Play,
                player,
                face_down,
                ..Default::default()
            },
        );

        self.reorder_hand(player, false);

        self.current_trick[player] = Some(card);
        self.played_face_down[player] = face_down;

        // Track voids for determination
        if player != self.lead_player {
            let lead_suit = if self.played_face_down[self.lead_player] {
                Suit::Green
            } else {
                self.current_trick[self.lead_player].unwrap().suit
            };

            let played_suit = if face_down { Suit::Green } else { card.suit };

            if played_suit != lead_suit && !self.voids[player].contains(&lead_suit) {
                self.voids[player].push(lead_suit);
            }
        }

        // Check if trick is complete
        if self.current_trick.iter().any(|c| c.is_none()) {
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
            return;
        }

        // Determine trick winner
        let trick_winner = self.determine_trick_winner();

        // Get the value of the winning card (5 if played face-down)
        let winning_value = if self.played_face_down[trick_winner] {
            5
        } else {
            self.current_trick[trick_winner].unwrap().value
        };

        let old_trick_sum = self.trick_sums[trick_winner];
        self.trick_sums[trick_winner] += winning_value;

        // Show winning card
        let winning_card_id = self.current_trick[trick_winner].unwrap().id;
        let pause_index = self.new_change();
        self.add_change(
            pause_index,
            Change {
                change_type: ChangeType::ShowWinningCard,
                object_id: winning_card_id,
                dest: Location::Play,
                ..Default::default()
            },
        );

        // Show trick sum preview (delta)
        self.add_change(
            pause_index,
            Change {
                change_type: ChangeType::UpdateTrickSum,
                player: trick_winner,
                start_score: old_trick_sum,
                end_score: self.trick_sums[trick_winner],
                animate_score: false,
                ..Default::default()
            },
        );

        // Pause for player to see the result
        self.add_change(
            pause_index,
            Change {
                change_type: ChangeType::OptionalPause,
                object_id: 0,
                dest: Location::Play,
                ..Default::default()
            },
        );

        // Complete trick sum animation
        let complete_index = self.new_change();
        self.add_change(
            complete_index,
            Change {
                change_type: ChangeType::UpdateTrickSum,
                player: trick_winner,
                start_score: old_trick_sum,
                end_score: self.trick_sums[trick_winner],
                animate_score: true,
                ..Default::default()
            },
        );

        // Move cards to winner
        let change_index = self.new_change();
        let trick_card_ids: Vec<i32> = self
            .current_trick
            .iter()
            .filter_map(|c| c.map(|card| card.id))
            .collect();
        for card_id in trick_card_ids {
            self.add_change(
                change_index,
                Change {
                    change_type: ChangeType::TricksToWinner,
                    object_id: card_id,
                    dest: Location::Score,
                    player: trick_winner,
                    ..Default::default()
                },
            );
        }

        // Reset for next trick
        self.current_trick = [None; PLAYER_COUNT];
        self.played_face_down = [false; PLAYER_COUNT];
        self.green_five_played = false;
        self.current_player = trick_winner;
        self.lead_player = trick_winner;

        // Check if round is over
        if self.hands.iter().all(|h| h.is_empty()) {
            self.end_hand();
        }
    }

    fn determine_trick_winner(&self) -> usize {
        let lead_suit = if self.played_face_down[self.lead_player] {
            Suit::Green
        } else {
            self.current_trick[self.lead_player].unwrap().suit
        };

        let mut best_player = self.lead_player;
        let mut best_value = if self.played_face_down[self.lead_player] {
            5 // Face-down card is green 5
        } else {
            self.current_trick[self.lead_player].unwrap().value
        };
        let mut best_is_purple = if self.played_face_down[self.lead_player] {
            false // Green 5, not purple
        } else {
            self.current_trick[self.lead_player].unwrap().suit == Suit::Purple
        };

        for player in 0..PLAYER_COUNT {
            if player == self.lead_player {
                continue;
            }

            let card = self.current_trick[player].unwrap();
            let card_suit = if self.played_face_down[player] {
                Suit::Green
            } else {
                card.suit
            };
            let card_value = if self.played_face_down[player] {
                5
            } else {
                card.value
            };
            let is_purple = card_suit == Suit::Purple;

            // Purple (trump) beats everything except higher purple
            if is_purple {
                if !best_is_purple || card_value > best_value {
                    best_player = player;
                    best_value = card_value;
                    best_is_purple = true;
                }
            } else if !best_is_purple {
                // No purple in play yet, compare by lead suit
                if card_suit == lead_suit && card_value > best_value {
                    best_player = player;
                    best_value = card_value;
                }
            }
            // If best_is_purple and current is not, best stays
        }

        best_player
    }

    fn end_hand(&mut self) {
        let preview_index = if !self.changes.is_empty() {
            self.changes.len() - 1
        } else {
            self.new_change()
        };

        // Calculate scores
        let score_deltas = self.calculate_round_scores();

        // Show score previews
        for (player, &delta) in score_deltas.iter().enumerate() {
            self.add_change(
                preview_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: self.scores[player],
                    end_score: self.scores[player] + delta,
                    animate_score: false,
                    ..Default::default()
                },
            );
        }

        // Pause
        self.add_change(
            preview_index,
            Change {
                change_type: ChangeType::OptionalPause,
                object_id: -1,
                ..Default::default()
            },
        );

        // Animate scores
        let animate_index = self.new_change();
        for (player, &delta) in score_deltas.iter().enumerate() {
            self.add_change(
                animate_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: self.scores[player],
                    end_score: self.scores[player] + delta,
                    animate_score: true,
                    ..Default::default()
                },
            );
        }

        // Apply scores
        for (player, &delta) in score_deltas.iter().enumerate() {
            self.scores[player] += delta;
        }

        if self.round >= ROUNDS as i32 {
            self.state = State::GameOver;
            let max_score = self.scores.iter().max().unwrap();
            for player in 0..PLAYER_COUNT {
                if self.scores[player] == *max_score {
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
            self.deal(true);
        }
    }

    fn calculate_round_scores(&mut self) -> [i32; PLAYER_COUNT] {
        let mut deltas = [0i32; PLAYER_COUNT];
        let mut rankings: Vec<(usize, i32)> =
            (0..PLAYER_COUNT).map(|p| (p, self.trick_sums[p])).collect();

        // Sort by sum, highest first (but only those <= 25)
        rankings.sort_by(|a, b| {
            let a_valid = a.1 <= TARGET_SUM;
            let b_valid = b.1 <= TARGET_SUM;
            match (a_valid, b_valid) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.1.cmp(&a.1), // Both valid or both over: higher is better (for valid)
            }
        });

        // Count over-25 players and their penalty points
        let mut over_25_penalty = 0;
        for (player, sum) in &rankings {
            if *sum > TARGET_SUM {
                deltas[*player] = -1;
                over_25_penalty += 1;
            }
        }

        // Find valid players (sum <= 25)
        let valid_rankings: Vec<(usize, i32)> = rankings
            .iter()
            .filter(|(_, sum)| *sum <= TARGET_SUM)
            .copied()
            .collect();

        if !valid_rankings.is_empty() {
            // Award points based on rank
            // 4 player: 3, 2, 1, 0
            // Ties get the lower rank's points (per "sharing the lower rank" rule)
            let points_by_rank = [3, 2, 1, 0];

            let mut current_rank = 0;
            let mut i = 0;
            while i < valid_rankings.len() && current_rank < points_by_rank.len() {
                let current_sum = valid_rankings[i].1;
                // Find all players tied at this sum
                let mut tied_players: Vec<usize> = vec![valid_rankings[i].0];
                let mut j = i + 1;
                while j < valid_rankings.len() && valid_rankings[j].1 == current_sum {
                    tied_players.push(valid_rankings[j].0);
                    j += 1;
                }

                // All tied players get the lower rank's points
                // e.g., two-way tie for 1st gets 2nd place points
                let lower_rank_index =
                    (current_rank + tied_players.len() - 1).min(points_by_rank.len() - 1);
                let points = points_by_rank[lower_rank_index];
                for player in &tied_players {
                    deltas[*player] += points;

                    // Bonus for exactly 25
                    if current_sum == TARGET_SUM {
                        deltas[*player] += 1;
                    }
                }

                // Award bonus points from over-25 players
                if current_rank == 0 && tied_players.len() == 1 && over_25_penalty > 0 {
                    deltas[tied_players[0]] += over_25_penalty;
                } else if current_rank == 0 && tied_players.len() > 1 {
                    // Tied for first - bonus carries over
                    self.carry_over_bonus += over_25_penalty;
                }

                // Add any carried over bonus
                if current_rank == 0 && tied_players.len() == 1 && self.carry_over_bonus > 0 {
                    deltas[tied_players[0]] += self.carry_over_bonus;
                    self.carry_over_bonus = 0;
                }

                i = j;
                current_rank += tied_players.len();
            }
        }

        deltas
    }

    fn pop_card(&mut self, card_id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == card_id)
            .unwrap();
        self.hands[self.current_player].remove(pos)
    }

    fn sort_hand(&mut self, player: usize) {
        // Sort hand by suit, then by value (high to low)
        self.hands[player].sort_by(|a, b| match a.suit.cmp(&b.suit) {
            std::cmp::Ordering::Equal => b.value.cmp(&a.value),
            other => other,
        });
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

    fn reorder_hand(&mut self, player: usize, force_new_animation: bool) {
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
        let change_index = self.changes.len() - 1;

        // Handle SelectPlayStyle state - show green 5 option and undo
        if self.state == State::SelectPlayStyle {
            // Show the green 5 option
            self.add_change(
                change_index,
                Change {
                    object_id: GREEN_FIVE_OPTION,
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::Green5Option,
                    player: 0,
                    disabled: !self.green_five_available,
                    ..Default::default()
                },
            );
            // Show the undo option
            self.add_change(
                change_index,
                Change {
                    object_id: UNDO,
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::UndoOption,
                    player: 0,
                    ..Default::default()
                },
            );
            // Show the selected card in preview (playable to indicate it can be tapped)
            if let Some(card) = self.selected_card {
                self.add_change(
                    change_index,
                    Change {
                        object_id: card.id,
                        change_type: ChangeType::ShowPlayable,
                        dest: Location::Preview,
                        player: 0,
                        ..Default::default()
                    },
                );
            }
            return;
        }

        if self.current_player == 0 {
            let moves = self.get_moves();
            for id in moves {
                let card_id = if id >= FACE_DOWN_OFFSET {
                    id - FACE_DOWN_OFFSET
                } else {
                    id
                };
                self.add_change(
                    change_index,
                    Change {
                        object_id: card_id,
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
        let message: Option<String> = None; // Can add messages if needed

        let index = self.new_change();
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
        // Also hide the special option cards
        self.add_change(
            change_index,
            Change {
                object_id: GREEN_FIVE_OPTION,
                change_type: ChangeType::HidePlayable,
                dest: Location::Green5Option,
                player: 0,
                ..Default::default()
            },
        );
        self.add_change(
            change_index,
            Change {
                object_id: UNDO,
                change_type: ChangeType::HidePlayable,
                dest: Location::UndoOption,
                player: 0,
                ..Default::default()
            },
        );
    }
}

impl ismcts::Game for CincosVerdesGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    #[allow(clippy::needless_range_loop)]
    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        // Collect all non-observer face_down_cards into a combined hidden pool
        let mut hidden_pool: Vec<Card> = Vec::new();
        let mut hidden_counts = [0usize; PLAYER_COUNT];
        for p in 0..PLAYER_COUNT {
            if p != observer {
                hidden_counts[p] = self.face_down_cards[p].len();
                hidden_pool.append(&mut self.face_down_cards[p]);
            }
        }

        // For each non-observer, shuffle their hand against the hidden pool
        // This allows face-down card identities to be randomized across all hidden info
        for p in 0..PLAYER_COUNT {
            if p == observer || hidden_pool.is_empty() {
                continue;
            }

            let mut hands = vec![self.hands[p].clone(), hidden_pool];
            shuffle_and_divide_matching_cards(
                |_: &Card| true, // No suit constraints for hidden pool
                &mut hands,
                rng,
            );
            self.hands[p] = hands[0].clone();
            hidden_pool = hands[1].clone();
        }

        // Redistribute the hidden pool back to face_down_cards
        for p in 0..PLAYER_COUNT {
            if p != observer && hidden_counts[p] > 0 {
                self.face_down_cards[p] = hidden_pool.drain(..hidden_counts[p]).collect();
            }
        }

        // Shuffle hands between non-observers (respecting void constraints)
        for p1 in 0..PLAYER_COUNT {
            for p2 in 0..PLAYER_COUNT {
                if p1 == p2 || p1 == observer || p2 == observer {
                    continue;
                }

                let mut combined_voids = [false; 4];
                for suit in &self.voids[p1] {
                    combined_voids[*suit as usize] = true;
                }
                for suit in &self.voids[p2] {
                    combined_voids[*suit as usize] = true;
                }

                let mut new_hands = vec![self.hands[p1].clone(), self.hands[p2].clone()];
                shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids[c.suit as usize],
                    &mut new_hands,
                    rng,
                );

                self.hands[p1] = new_hands[0].clone();
                self.hands[p2] = new_hands[1].clone();
            }
        }
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
            return None; // ISMCTS continues simulation
        }

        // Exponential reward - amplifies score differences
        // Lowest bust rate (~38%) and avg sum closest to target (25)
        let scores = self.scores;
        let player_score = scores[player] as f64;
        let max_score = *scores.iter().max().unwrap() as f64;
        let min_score = *scores.iter().min().unwrap() as f64;

        if max_score == min_score {
            return Some(0.0);
        }
        let linear = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;
        // Apply exponential transformation: sign(x) * |x|^2
        Some(linear.signum() * linear.abs().powf(2.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_game() {
        let game = CincosVerdesGame::new();
        assert!(game.hands.iter().all(|h| h.len() == HAND_SIZE));
        assert_eq!(game.state, State::Play);
        assert_eq!(game.round, 1);
        assert!(game.scores.iter().all(|&s| s == STARTING_POINTS));
    }

    #[test]
    fn test_deck_composition() {
        let deck = CincosVerdesGame::deck();
        assert_eq!(deck.len(), 52);

        // Count by suit
        let orange_count = deck.iter().filter(|c| c.suit == Suit::Orange).count();
        let purple_count = deck.iter().filter(|c| c.suit == Suit::Purple).count();
        let pink_count = deck.iter().filter(|c| c.suit == Suit::Pink).count();
        let green_count = deck.iter().filter(|c| c.suit == Suit::Green).count();

        assert_eq!(orange_count, 13); // 1-13
        assert_eq!(purple_count, 13); // 1-13
        assert_eq!(pink_count, 13); // 1-13
        assert_eq!(green_count, 13); // 0-4, 6-13 (no 5)

        // Verify green has no 5
        assert!(!deck.iter().any(|c| c.suit == Suit::Green && c.value == 5));
    }

    #[test]
    fn test_trick_winner_highest_lead_suit() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;

        game.current_trick = [
            Some(Card {
                id: 0,
                suit: Suit::Orange,
                value: 5,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Orange,
                value: 9,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 3,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Orange,
                value: 7,
            }),
        ];
        game.played_face_down = [false; PLAYER_COUNT];
        game.lead_player = 0;

        let winner = game.determine_trick_winner();
        assert_eq!(winner, 1, "Player 1 has highest orange (9)");
    }

    #[test]
    fn test_trick_winner_purple_trump() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;

        game.current_trick = [
            Some(Card {
                id: 0,
                suit: Suit::Orange,
                value: 13,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Purple,
                value: 1,
            }), // Trump!
            Some(Card {
                id: 2,
                suit: Suit::Orange,
                value: 10,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Orange,
                value: 11,
            }),
        ];
        game.played_face_down = [false; PLAYER_COUNT];
        game.lead_player = 0;

        let winner = game.determine_trick_winner();
        assert_eq!(winner, 1, "Purple 1 beats all oranges");
    }

    #[test]
    fn test_trick_winner_highest_purple() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;

        game.current_trick = [
            Some(Card {
                id: 0,
                suit: Suit::Orange,
                value: 13,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Purple,
                value: 5,
            }),
            Some(Card {
                id: 2,
                suit: Suit::Purple,
                value: 9,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Orange,
                value: 11,
            }),
        ];
        game.played_face_down = [false; PLAYER_COUNT];
        game.lead_player = 0;

        let winner = game.determine_trick_winner();
        assert_eq!(winner, 2, "Purple 9 beats Purple 5");
    }

    #[test]
    fn test_face_down_as_green_five() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;

        game.current_trick = [
            Some(Card {
                id: 0,
                suit: Suit::Green,
                value: 10,
            }),
            Some(Card {
                id: 1,
                suit: Suit::Orange,
                value: 13,
            }), // Played face-down
            Some(Card {
                id: 2,
                suit: Suit::Green,
                value: 3,
            }),
            Some(Card {
                id: 3,
                suit: Suit::Green,
                value: 7,
            }),
        ];
        game.played_face_down = [false, true, false, false]; // Player 1 played face-down
        game.lead_player = 0;

        let winner = game.determine_trick_winner();
        // Green 10 should win (face-down is green 5, so green 10 > green 7 > green 5 > green 3)
        assert_eq!(winner, 0, "Green 10 wins");
    }

    #[test]
    fn test_scoring_exact_25() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [25, 20, 30, 15];

        let deltas = game.calculate_round_scores();

        // Player 0: exact 25 = 3 (closest) + 1 (exact bonus) + 1 (over penalty from P2) = 5
        // Player 1: 20 = 2 (second closest)
        // Player 2: 30 = -1 (over)
        // Player 3: 15 = 1 (third closest)
        assert_eq!(deltas[0], 5, "Exact 25 gets 3 + 1 bonus + 1 over penalty");
        assert_eq!(deltas[1], 2, "Second closest gets 2");
        assert_eq!(deltas[2], -1, "Over 25 loses 1");
        assert_eq!(deltas[3], 1, "Third closest gets 1");
    }

    #[test]
    fn test_scoring_with_tie_for_second() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [23, 16, 16, 39];

        let deltas = game.calculate_round_scores();

        // Player 0: 23 = 3 (closest) + 1 (bonus from player 3 exceeding) = 4
        // Player 1: 16 = 1 (tied for second gets lower rank = 3rd place)
        // Player 2: 16 = 1 (tied for second gets lower rank = 3rd place)
        // Player 3: 39 = -1 (over)
        assert_eq!(deltas[0], 4, "Closest to 25 gets 3 + 1 bonus");
        assert_eq!(
            deltas[1], 1,
            "Tied for second gets lower rank (3rd place) = 1"
        );
        assert_eq!(
            deltas[2], 1,
            "Tied for second gets lower rank (3rd place) = 1"
        );
        assert_eq!(deltas[3], -1, "Over 25 loses 1");
    }

    #[test]
    fn test_scoring_all_over() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [26, 27, 28, 29];

        let deltas = game.calculate_round_scores();

        // All over 25, everyone loses 1
        assert!(deltas.iter().all(|&d| d == -1));
    }

    #[test]
    fn test_scoring_tied_for_first() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [24, 24, 20, 30];
        game.carry_over_bonus = 0;

        let deltas = game.calculate_round_scores();

        // Players 0 and 1 tied at 24 - both get 2nd place points (lower rank rule)
        // No bonus from over-25 player goes to anyone (tied)
        // Player 2: 20 = third place
        // Player 3: -1 (over)
        assert_eq!(
            deltas[0], 2,
            "Tied for first gets lower rank (2nd place) = 2"
        );
        assert_eq!(
            deltas[1], 2,
            "Tied for first gets lower rank (2nd place) = 2"
        );
        // After tie, rank 2 gets third place points
        assert_eq!(deltas[2], 1, "Third place after tie");
        assert_eq!(deltas[3], -1, "Over 25");
        assert_eq!(game.carry_over_bonus, 1, "Bonus carries over when tied");
    }

    #[test]
    fn test_scoring_two_way_tie_for_first_with_exact_25() {
        // Bug report: 25-25-24-28 was scoring 4-4-1-(-1) but should be 3-3-1-(-1)
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [25, 25, 24, 28];
        game.carry_over_bonus = 0;

        let deltas = game.calculate_round_scores();

        // Players 0 and 1 tied at 25 - both get 2nd place points (2) + exact 25 bonus (1) = 3
        assert_eq!(
            deltas[0], 3,
            "Tied for first at 25: 2 (lower rank) + 1 (exact bonus) = 3"
        );
        assert_eq!(
            deltas[1], 3,
            "Tied for first at 25: 2 (lower rank) + 1 (exact bonus) = 3"
        );
        // Player 2 at 24 gets 3rd place points
        assert_eq!(deltas[2], 1, "Third place gets 1");
        assert_eq!(deltas[3], -1, "Over 25 loses 1");
    }

    #[test]
    fn test_scoring_three_way_tie_for_first_with_exact_25() {
        // Bug report: three players at 25 were scoring 4 points each
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [25, 25, 25, 28];
        game.carry_over_bonus = 0;

        let deltas = game.calculate_round_scores();

        // Players 0, 1, 2 tied at 25 - all get 3rd place points (1) + exact 25 bonus (1) = 2
        assert_eq!(
            deltas[0], 2,
            "Three-way tie at 25: 1 (lower rank) + 1 (exact bonus) = 2"
        );
        assert_eq!(
            deltas[1], 2,
            "Three-way tie at 25: 1 (lower rank) + 1 (exact bonus) = 2"
        );
        assert_eq!(
            deltas[2], 2,
            "Three-way tie at 25: 1 (lower rank) + 1 (exact bonus) = 2"
        );
        assert_eq!(deltas[3], -1, "Over 25 loses 1");
    }

    #[test]
    fn test_scoring_four_way_tie_at_25() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [25, 25, 25, 25];
        game.carry_over_bonus = 0;

        let deltas = game.calculate_round_scores();

        // All tied at 25 - all get 4th place points (0) + exact 25 bonus (1) = 1
        assert_eq!(
            deltas[0], 1,
            "Four-way tie at 25: 0 (lower rank) + 1 (exact bonus) = 1"
        );
        assert_eq!(
            deltas[1], 1,
            "Four-way tie at 25: 0 (lower rank) + 1 (exact bonus) = 1"
        );
        assert_eq!(
            deltas[2], 1,
            "Four-way tie at 25: 0 (lower rank) + 1 (exact bonus) = 1"
        );
        assert_eq!(
            deltas[3], 1,
            "Four-way tie at 25: 0 (lower rank) + 1 (exact bonus) = 1"
        );
    }

    #[test]
    fn test_scoring_tie_for_second() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [24, 20, 20, 28];
        game.carry_over_bonus = 0;

        let deltas = game.calculate_round_scores();

        // Player 0 at 24 gets 1st place (3) + over bonus (1) = 4
        assert_eq!(deltas[0], 4, "First place gets 3 + 1 over bonus = 4");
        // Players 1 and 2 tied at 20 - both get 3rd place points (lower rank rule)
        assert_eq!(
            deltas[1], 1,
            "Tied for second gets lower rank (3rd place) = 1"
        );
        assert_eq!(
            deltas[2], 1,
            "Tied for second gets lower rank (3rd place) = 1"
        );
        assert_eq!(deltas[3], -1, "Over 25 loses 1");
    }

    #[test]
    fn test_scoring_three_way_tie_for_second() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.trick_sums = [24, 20, 20, 20];
        game.carry_over_bonus = 0;

        let deltas = game.calculate_round_scores();

        // Player 0 at 24 gets 1st place (3)
        assert_eq!(deltas[0], 3, "First place gets 3");
        // Players 1, 2, 3 tied at 20 - all get 4th place points (0) - lowest rank in tie
        assert_eq!(
            deltas[1], 0,
            "Three-way tie for second gets lower rank (4th place) = 0"
        );
        assert_eq!(
            deltas[2], 0,
            "Three-way tie for second gets lower rank (4th place) = 0"
        );
        assert_eq!(
            deltas[3], 0,
            "Three-way tie for second gets lower rank (4th place) = 0"
        );
    }

    #[test]
    fn test_green_five_only_one_per_trick() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.state = State::Play;
        game.current_player = 0;
        game.lead_player = 0;

        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Orange,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Pink,
                value: 3,
            },
        ];

        // First, no green 5 played yet
        game.green_five_played = false;
        let moves = game.get_moves();
        // Should be able to play both cards face-up or face-down
        assert!(moves.contains(&0)); // Orange 5 face-up
        assert!(moves.contains(&100)); // Orange 5 face-down
        assert!(moves.contains(&1)); // Pink 3 face-up
        assert!(moves.contains(&101)); // Pink 3 face-down

        // Now mark green 5 as played
        game.green_five_played = true;
        let moves = game.get_moves();
        // Should only be able to play face-up
        assert!(moves.contains(&0));
        assert!(!moves.contains(&100));
        assert!(moves.contains(&1));
        assert!(!moves.contains(&101));
    }

    #[test]
    fn test_must_follow_suit() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.state = State::Play;
        game.lead_player = 1;
        game.current_player = 0;

        game.current_trick[1] = Some(Card {
            id: 10,
            suit: Suit::Orange,
            value: 7,
        });
        game.played_face_down[1] = false;
        game.green_five_played = false;

        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Orange,
                value: 5,
            },
            Card {
                id: 1,
                suit: Suit::Pink,
                value: 3,
            },
            Card {
                id: 2,
                suit: Suit::Green,
                value: 8,
            },
        ];

        let moves = game.get_moves();

        // Must follow orange - only orange card playable face-up
        assert!(moves.contains(&0), "Can play orange face-up");
        // Can't play pink or green face-up
        assert!(
            !moves.contains(&1),
            "Can't play pink when must follow orange"
        );
        assert!(
            !moves.contains(&2),
            "Can't play green when must follow orange"
        );

        // Can play orange face-down if exactly one orange card
        assert!(moves.contains(&100), "Can play only orange card face-down");
    }

    #[test]
    fn test_green_zero_starts() {
        let game = CincosVerdesGame::new();

        // Find who has green 0
        let green_zero_holder = game.find_green_zero_holder();

        assert_eq!(game.current_player, green_zero_holder);
        assert_eq!(game.lead_player, green_zero_holder);
    }

    #[test]
    fn test_face_down_cards_tracked() {
        let mut game = CincosVerdesGame::new();
        game.no_changes = true;
        game.state = State::Play;
        game.current_player = 1;
        game.lead_player = 1;

        // Give player 1 a specific card
        let card = Card {
            id: 50,
            suit: Suit::Orange,
            value: 7,
        };
        game.hands[1] = vec![card];

        // Play face-down (id + 100)
        game.apply_move(150);

        // Verify the card was tracked in face_down_cards
        assert_eq!(game.face_down_cards[1].len(), 1);
        assert_eq!(game.face_down_cards[1][0].id, 50);
    }

    #[test]
    fn test_randomize_determination_shuffles_face_down_cards() {
        use ismcts::Game;

        let mut game = CincosVerdesGame::new();
        game.no_changes = true;

        // Set up a scenario where player 1 has face-down cards
        let face_down_card = Card {
            id: 50,
            suit: Suit::Orange,
            value: 7,
        };
        game.face_down_cards[1] = vec![face_down_card];

        // Player 2 has some cards in hand
        game.hands[2] = vec![
            Card {
                id: 51,
                suit: Suit::Pink,
                value: 3,
            },
            Card {
                id: 52,
                suit: Suit::Green,
                value: 8,
            },
        ];

        // Observer is player 0 - they should not know what player 1's face-down card is
        // After randomization, the face-down card could have been swapped with player 2's hand

        // Run randomization many times to verify cards can be swapped
        let mut face_down_changed = false;
        for _ in 0..100 {
            let mut test_game = game.clone();
            test_game.randomize_determination(0);

            // Check if the face-down card was swapped
            if test_game.face_down_cards[1][0].id != 50 {
                face_down_changed = true;
                break;
            }
        }

        assert!(
            face_down_changed,
            "Face-down cards should be shuffled with other hidden cards"
        );
    }

    #[test]
    fn test_randomize_determination_preserves_observer_face_down() {
        use ismcts::Game;

        let mut game = CincosVerdesGame::new();
        game.no_changes = true;

        // Observer (player 0) has a face-down card - they know what it is
        let observer_face_down = Card {
            id: 99,
            suit: Suit::Purple,
            value: 10,
        };
        game.face_down_cards[0] = vec![observer_face_down];

        // Other player has cards
        game.hands[1] = vec![Card {
            id: 51,
            suit: Suit::Pink,
            value: 3,
        }];

        // Run randomization with player 0 as observer
        game.randomize_determination(0);

        // Observer's face-down card should NOT be shuffled
        assert_eq!(game.face_down_cards[0].len(), 1);
        assert_eq!(
            game.face_down_cards[0][0].id, 99,
            "Observer's face-down card should be preserved"
        );
    }
}
