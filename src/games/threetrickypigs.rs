/*
Game: 3 Tricky Pigs
Designers: Andrew Stiles and Steven Ungaro
BoardGameGeek: https://boardgamegeek.com/boardgame/441614/3-tricky-pigs
*/

/*

Planned flow:
- Player can stage huff cards, puff cards and then a card from their hand
- As soon as a card is played from the hand the play is committed
- We'll play the actual card on top of the huff and puff cards
*/

use ismcts::IsmctsHandler;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rand::Rng;
use serde::{Deserialize, Serialize};

const PLAYER_COUNT: usize = 4;
const HAND_SIZE: usize = 12;
const ROUNDS: usize = 4;

// Special move values
const UNDO: i32 = -2; // Undo modifier or bid selection (human player only)

use std::collections::HashSet;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
pub enum Suit {
    Straw,
    Sticks,
    Bricks,
    Wolf,
    Huff,
    Puff,
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
    Modifier,      // Location for staged huff/puff cards
    UndoOption,    // Location for the undo button
    BidSelection,  // Location for bid cards during selection
    BidConfirm,    // Location for selected bid card during confirmation
    BidOffscreen,  // Location for bid cards moved offscreen
    ScoringCenter, // Center of screen for bid card during scoring display
    ScoringBelow,  // Below the bid card for leftover huff/puff cards
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
    PlayModifier,      // Play a huff or puff card
    ShowBidCards,      // Show all bid cards for selection
    MoveBidCard,       // Move a bid card to a location
    HideBidCards,      // Hide all bid cards
    ShowScoringBid,    // Show bid card during end-of-round scoring
    ShowLeftoverCards, // Show leftover huff/puff cards during scoring
    HideScoringCards,  // Hide the scoring bid and leftover cards
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
    pub disabled: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Bid {
    /// Try to win 0 tricks
    Sleep,
    /// Win 2 tricks
    Play,
    /// Win 3 or more tricks
    Work,
    /// Win the most tricks of any player
    Eat,
}

impl Bid {
    /// Convert bid to display string for UI
    pub fn to_display_string(&self) -> &'static str {
        match self {
            Bid::Sleep => "0",
            Bid::Play => "2",
            Bid::Work => "3+",
            Bid::Eat => "⬆",
        }
    }

    /// Convert bid to index (for bid card IDs)
    pub fn to_index(&self) -> i32 {
        match self {
            Bid::Sleep => 0,
            Bid::Play => 1,
            Bid::Work => 2,
            Bid::Eat => 3,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum State {
    /// Select one of the bid cards
    #[default]
    Bid,
    /// Confirm the selected bid (human player only)
    BidConfirm,
    /// Standard must-follow trick taking
    Play,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub value: i32,
    pub suit: Suit,
    pub id: i32,
}

impl Card {
    fn is_puff(&self) -> bool {
        self.suit == Suit::Puff
    }
    fn is_huff(&self) -> bool {
        self.suit == Suit::Huff
    }
    fn is_regular(&self) -> bool {
        !self.is_huff() && !self.is_puff()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ThreeTrickyPigsGame {
    /// Current game state
    pub state: State,
    /// Current player
    pub current_player: usize,
    /// Lead player for the current trick
    pub lead_player: usize,
    /// Whether or not the Wolf suit has been broken yet
    pub wolf_suit_broken: bool,
    /// Regular cards in a trick (indexed by player)
    pub current_trick_regular: [Option<Card>; PLAYER_COUNT],
    /// Huff cards in the current trick (indexed by player)
    pub current_trick_huff: [Option<Card>; PLAYER_COUNT],
    /// Puff cards in the current trick (indexed by player)
    pub current_trick_puff: [Option<Card>; PLAYER_COUNT],
    /// Each player's hand
    pub hands: [Vec<Card>; PLAYER_COUNT],
    /// Each player's current bid
    pub bids: [Option<Bid>; PLAYER_COUNT],
    /// Selected bid pending confirmation (for human player)
    pub selected_bid: Option<Bid>,
    /// Tricks won by each player this round
    pub tricks_won: [usize; PLAYER_COUNT],
    /// Current round (1-4)
    pub current_round: usize,
    /// Total scores for each player
    pub scores: [i32; PLAYER_COUNT],
    /// Known voids for each player (used for ISMCTS determination)
    pub voids: [Vec<Suit>; PLAYER_COUNT],
    /// Players who can undo (typically just the human player)
    #[serde(default)]
    pub undo_players: HashSet<usize>,
    /// Skip generating change animations (for MCTS simulations)
    pub no_changes: bool,
    /// Animation changes for UI
    pub changes: Vec<Vec<Change>>,
    /// Winner of the game (if game over)
    pub winner: Option<usize>,
}

impl ThreeTrickyPigsGame {
    /// Create a new game with shuffled and dealt cards
    pub fn new() -> Self {
        let mut rng = thread_rng();
        let starting_player = rng.gen_range(0..PLAYER_COUNT);
        let mut game = ThreeTrickyPigsGame {
            current_round: 1,
            lead_player: starting_player,
            current_player: starting_player,
            undo_players: HashSet::from([0]), // Human player can undo
            ..Default::default()
        };
        game.deal(true);
        game
    }

    /// Set which players can undo moves
    pub fn with_undo_players(&mut self, players: HashSet<usize>) {
        self.undo_players = players;
    }

    /// Check if current player can undo
    fn can_undo(&self) -> bool {
        self.undo_players.contains(&self.current_player)
    }

    /// Deal cards for a new round
    pub fn deal(&mut self, animate: bool) {
        let mut cards = deck();
        let rng = &mut thread_rng();
        cards.shuffle(rng);

        // Deal 12 cards to each player (includes huff/puff cards)
        for player in 0..PLAYER_COUNT {
            self.hands[player] = cards.drain(..HAND_SIZE).collect();
        }

        // Sort human player's hand by suit then value
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

        self.show_playable();
        self.show_message();
    }

    /// Sort a player's hand by suit then value
    fn sort_hand(&mut self, player: usize) {
        self.hands[player].sort_by(|a, b| {
            // Sort order: Straw, Sticks, Bricks, Wolf, Huff, Puff
            let suit_order = |s: &Suit| match s {
                Suit::Straw => 0,
                Suit::Sticks => 1,
                Suit::Bricks => 2,
                Suit::Wolf => 3,
                Suit::Huff => 4,
                Suit::Puff => 5,
            };
            match suit_order(&a.suit).cmp(&suit_order(&b.suit)) {
                std::cmp::Ordering::Equal => a.value.cmp(&b.value),
                other => other,
            }
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
        // For Play state, create a new change index so ShowPlayable is processed last
        // This prevents conflicts with other changes in the same batch
        let change_index = if self.state == State::Play {
            self.new_change()
        } else {
            self.changes.len() - 1
        };

        if self.current_player == 0 {
            let moves = self.get_moves();
            let undo_available = moves.contains(&UNDO);

            for id in moves {
                // For bid state, don't show cards as playable (bids are shown differently)
                if self.state == State::Bid {
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

            // Also highlight staged huff/puff cards that can be undone
            if self.state == State::Play && self.can_undo() {
                if let Some(huff_card) = self.current_trick_huff[0] {
                    self.add_change(
                        change_index,
                        Change {
                            object_id: huff_card.id,
                            change_type: ChangeType::ShowPlayable,
                            dest: Location::Modifier,
                            player: 0,
                            ..Default::default()
                        },
                    );
                }
                if let Some(puff_card) = self.current_trick_puff[0] {
                    self.add_change(
                        change_index,
                        Change {
                            object_id: puff_card.id,
                            change_type: ChangeType::ShowPlayable,
                            dest: Location::Modifier,
                            player: 0,
                            ..Default::default()
                        },
                    );
                }
            }

            // Hide undo button if it's not available
            if !undo_available && self.state == State::Play {
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
                    player: 0,
                    ..Default::default()
                },
            );
        }

        // Also hide playable on undo button
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

        // Hide playable on staged modifier cards
        if let Some(huff_card) = self.current_trick_huff[0] {
            self.add_change(
                change_index,
                Change {
                    object_id: huff_card.id,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Modifier,
                    player: 0,
                    ..Default::default()
                },
            );
        }
        if let Some(puff_card) = self.current_trick_puff[0] {
            self.add_change(
                change_index,
                Change {
                    object_id: puff_card.id,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Modifier,
                    player: 0,
                    ..Default::default()
                },
            );
        }
    }

    fn show_message(&mut self) {
        let message: Option<String> = match self.state {
            State::Bid if self.current_player == 0 => Some("Select your bid".to_string()),
            State::BidConfirm if self.current_player == 0 => {
                Some("Tap to confirm or undo your bid".to_string())
            }
            _ => None,
        };

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

        // Show bid cards at selection positions when entering bid state
        if self.state == State::Bid && self.current_player == 0 {
            for i in 0..4 {
                let bid_card_id = -(10 + i);
                self.add_change(
                    index,
                    Change {
                        change_type: ChangeType::ShowBidCards,
                        object_id: bid_card_id,
                        dest: Location::BidSelection,
                        offset: i as usize,
                        ..Default::default()
                    },
                );
            }
            // Show "?" in trick counter when selecting bid
            self.add_change(
                index,
                Change {
                    change_type: ChangeType::UpdateTrickCount,
                    player: 0,
                    trick_count: self.tricks_won[0] as i32,
                    message: Some("?".to_string()),
                    ..Default::default()
                },
            );
        }

        // Show undo button when in bid confirm state
        if self.state == State::BidConfirm && self.current_player == 0 {
            self.add_change(
                index,
                Change {
                    change_type: ChangeType::ShowBidCards,
                    object_id: UNDO,
                    dest: Location::UndoOption,
                    ..Default::default()
                },
            );
            // Show staged bid in trick counter
            if let Some(bid) = self.selected_bid {
                self.add_change(
                    index,
                    Change {
                        change_type: ChangeType::UpdateTrickCount,
                        player: 0,
                        trick_count: self.tricks_won[0] as i32,
                        message: Some(bid.to_display_string().to_string()),
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Returns possible moves
    pub fn get_moves(&self) -> Vec<i32> {
        match self.state {
            // 4 bid options
            State::Bid => (0..=3).collect(),
            // Confirm or undo bid selection (human player)
            State::BidConfirm => {
                let mut moves = vec![0, 1, 2, 3]; // Tap to confirm (same as bid)
                if self.can_undo() {
                    moves.push(UNDO);
                }
                moves
            }
            // Regular trick play
            State::Play => {
                let lead_suit = self.current_trick_regular[self.lead_player].map(|c| c.suit);
                let puff_played = self.current_trick_puff[self.current_player].is_some();
                let huff_played = self.current_trick_huff[self.current_player].is_some();
                let hand = &self.hands[self.current_player];
                let is_leading = lead_suit.is_none();

                // Find playable regular cards

                // Find all cards in the current lead suit
                let follow_suit_cards: Vec<&Card> = hand
                    .iter()
                    .filter(|c| lead_suit.map_or(false, |s| c.suit == s))
                    .collect();

                // Check if player has any pig cards (non-wolf regular cards)
                let has_pig_cards = hand.iter().any(|c| c.is_regular() && c.suit != Suit::Wolf);

                let playable_regular_cards: Vec<&Card> = if follow_suit_cards.is_empty() {
                    // No lead card or no cards in lead suit - any regular card in hand
                    // can be played (wolves allowed if broken, or not leading, or player has no pigs)
                    hand.iter()
                        .filter(|c| {
                            c.is_regular()
                                && (c.suit != Suit::Wolf
                                    || self.wolf_suit_broken
                                    || !is_leading
                                    || !has_pig_cards)
                        })
                        .collect()
                } else {
                    // Must follow suit in 3 Tricky Pigs
                    follow_suit_cards
                };

                // Find playable huff and puff cards
                let playable_huff_and_puff_cards = hand
                    .iter()
                    .filter(|c| (!puff_played && c.is_puff()) || (!huff_played && c.is_huff()));

                // Build the moves list
                let mut moves: Vec<i32> = playable_regular_cards
                    .into_iter()
                    .chain(playable_huff_and_puff_cards)
                    .map(|c| c.id)
                    .collect();

                // Allow undo if huff or puff was played and player can undo
                if self.can_undo() && (huff_played || puff_played) {
                    moves.push(UNDO);
                }

                moves
            }
        }
    }

    /// Apply a move to the game state
    pub fn apply_move(&mut self, card_id: i32) {
        self.changes = vec![vec![]];

        // Validate move is legal
        let valid_moves = self.get_moves();
        if !valid_moves.contains(&card_id) {
            panic!(
                "Invalid move: {} not in valid moves {:?}",
                card_id, valid_moves
            );
        }

        match self.state {
            State::Bid => {
                // card_id 0-3 maps to bid variants
                let bid = match card_id {
                    0 => Bid::Sleep,
                    1 => Bid::Play,
                    2 => Bid::Work,
                    3 => Bid::Eat,
                    _ => panic!("Invalid bid"),
                };

                // If human player, go to confirmation state
                if self.can_undo() {
                    self.selected_bid = Some(bid);
                    self.state = State::BidConfirm;

                    // Move selected bid card to center, others offscreen
                    let index = self.new_change();
                    for i in 0..4 {
                        let bid_card_id = -(10 + i);
                        if i == card_id {
                            // Selected bid moves to center
                            self.add_change(
                                index,
                                Change {
                                    change_type: ChangeType::MoveBidCard,
                                    object_id: bid_card_id,
                                    dest: Location::BidConfirm,
                                    offset: i as usize,
                                    ..Default::default()
                                },
                            );
                        } else {
                            // Other bids move offscreen
                            self.add_change(
                                index,
                                Change {
                                    change_type: ChangeType::MoveBidCard,
                                    object_id: bid_card_id,
                                    dest: Location::BidOffscreen,
                                    offset: i as usize,
                                    ..Default::default()
                                },
                            );
                        }
                    }

                    self.show_message();
                } else {
                    // AI players confirm immediately
                    self.bids[self.current_player] = Some(bid);
                    self.current_player = (self.current_player + 1) % PLAYER_COUNT;

                    // If all players have bid, move to play state
                    if self.bids.iter().all(|b| b.is_some()) {
                        self.state = State::Play;
                        // Reset current_player to lead_player for first trick
                        self.current_player = self.lead_player;
                    }

                    self.show_playable();
                    self.show_message();
                }
            }
            State::BidConfirm => {
                if card_id == UNDO {
                    // Undo bid selection - go back to bid state
                    self.selected_bid = None;
                    self.state = State::Bid;

                    // Hide undo button
                    let index = self.new_change();
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::HideBidCards,
                            object_id: UNDO,
                            dest: Location::BidOffscreen,
                            ..Default::default()
                        },
                    );

                    // Move all bid cards back to selection positions
                    for i in 0..4 {
                        let bid_card_id = -(10 + i);
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::MoveBidCard,
                                object_id: bid_card_id,
                                dest: Location::BidSelection,
                                offset: i as usize,
                                ..Default::default()
                            },
                        );
                    }

                    // Just update message, don't re-emit ShowBidCards (MoveBidCard handles positioning)
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::Message,
                            message: Some("Select your bid".to_string()),
                            object_id: -1,
                            dest: Location::Message,
                            ..Default::default()
                        },
                    );

                    // Reset trick counter to show "?" when undoing bid
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::UpdateTrickCount,
                            player: 0,
                            trick_count: self.tricks_won[0] as i32,
                            message: Some("?".to_string()),
                            ..Default::default()
                        },
                    );
                } else {
                    // Confirm bid - move selected bid offscreen
                    let selected_bid_index = match self.selected_bid {
                        Some(Bid::Sleep) => 0,
                        Some(Bid::Play) => 1,
                        Some(Bid::Work) => 2,
                        Some(Bid::Eat) => 3,
                        None => 0,
                    };

                    let index = self.new_change();

                    // Hide undo button
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::HideBidCards,
                            object_id: UNDO,
                            dest: Location::BidOffscreen,
                            ..Default::default()
                        },
                    );

                    let bid_card_id = -(10 + selected_bid_index);
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::MoveBidCard,
                            object_id: bid_card_id,
                            dest: Location::BidOffscreen,
                            offset: selected_bid_index as usize,
                            ..Default::default()
                        },
                    );

                    // Update trick counter to show confirmed bid
                    if let Some(bid) = self.selected_bid {
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::UpdateTrickCount,
                                player: 0,
                                trick_count: self.tricks_won[0] as i32,
                                message: Some(bid.to_display_string().to_string()),
                                ..Default::default()
                            },
                        );
                    }

                    self.bids[self.current_player] = self.selected_bid;
                    self.selected_bid = None;
                    self.current_player = (self.current_player + 1) % PLAYER_COUNT;

                    // If all players have bid, move to play state
                    if self.bids.iter().all(|b| b.is_some()) {
                        self.state = State::Play;
                        // Reset current_player to lead_player for first trick
                        self.current_player = self.lead_player;
                    } else {
                        self.state = State::Bid;
                    }

                    self.show_playable();
                    self.show_message();
                }
            }
            State::Play => {
                // Handle undo of modifier cards
                if card_id == UNDO {
                    // Return huff and/or puff cards to hand
                    let current_player = self.current_player;
                    if let Some(huff_card) = self.current_trick_huff[current_player].take() {
                        self.hands[current_player].push(huff_card);
                    }
                    if let Some(puff_card) = self.current_trick_puff[current_player].take() {
                        self.hands[current_player].push(puff_card);
                    }
                    self.reorder_hand(current_player, true);
                    self.show_playable();
                    self.show_message();
                    return;
                }
                let current_player = self.current_player;
                let hand = &mut self.hands[current_player];

                // Find and remove the card from hand
                let card_index = hand.iter().position(|c| c.id == card_id).unwrap();
                let card = hand.remove(card_index);

                // Place card in appropriate trick slot
                if card.is_huff() {
                    self.current_trick_huff[current_player] = Some(card);

                    // Animate huff card play to modifier area
                    self.add_change(
                        0,
                        Change {
                            change_type: ChangeType::PlayModifier,
                            object_id: card_id,
                            dest: Location::Modifier,
                            player: current_player,
                            ..Default::default()
                        },
                    );
                    self.reorder_hand(current_player, false);
                    self.show_playable();
                    self.show_message();
                } else if card.is_puff() {
                    self.current_trick_puff[current_player] = Some(card);

                    // Animate puff card play to modifier area
                    self.add_change(
                        0,
                        Change {
                            change_type: ChangeType::PlayModifier,
                            object_id: card_id,
                            dest: Location::Modifier,
                            player: current_player,
                            ..Default::default()
                        },
                    );
                    self.reorder_hand(current_player, false);
                    self.show_playable();
                    self.show_message();
                } else {
                    // After a regular card (pig or wolf) is played the move is
                    // committed
                    self.current_trick_regular[current_player] = Some(card);

                    // Animate card play
                    self.add_change(
                        0,
                        Change {
                            change_type: ChangeType::Play,
                            object_id: card_id,
                            dest: Location::Play,
                            player: current_player,
                            ..Default::default()
                        },
                    );
                    self.reorder_hand(current_player, false);

                    // Track voids - if player couldn't follow suit
                    let lead_suit = self.current_trick_regular[self.lead_player].map(|c| c.suit);
                    if let Some(ls) = lead_suit {
                        if card.suit != ls && !self.voids[current_player].contains(&ls) {
                            self.voids[current_player].push(ls);
                        }
                    }

                    // Check if wolf was played when couldn't follow suit (breaks wolf)
                    if card.suit == Suit::Wolf && !self.wolf_suit_broken {
                        // Wolf is broken if player couldn't follow lead suit
                        // (lead_suit exists and player played wolf instead)
                        if lead_suit.is_some() && lead_suit != Some(Suit::Wolf) {
                            self.wolf_suit_broken = true;
                        }
                    }

                    // Advance to next player
                    self.current_player = (current_player + 1) % PLAYER_COUNT;

                    // Check if trick is complete (all players have played a regular card)
                    let trick_complete = self.current_trick_regular.iter().all(|c| c.is_some());

                    if trick_complete {
                        // Determine winner
                        let winner = trick_winner(
                            self.lead_player,
                            self.current_trick_regular,
                            self.current_trick_huff,
                            self.current_trick_puff,
                        );

                        // Show winning card
                        let winning_card_id = self.current_trick_regular[winner].unwrap().id;
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

                        // Update trick count for winner
                        self.tricks_won[winner] += 1;

                        let trick_index = self.new_change();
                        // For player 0, always show tricks/bid format
                        let message = if winner == 0 {
                            self.bids[0].map(|b| b.to_display_string().to_string())
                        } else {
                            None
                        };
                        self.add_change(
                            trick_index,
                            Change {
                                change_type: ChangeType::UpdateTrickCount,
                                player: winner,
                                trick_count: self.tricks_won[winner] as i32,
                                message,
                                ..Default::default()
                            },
                        );

                        // Move all cards in trick to winner
                        let change_index = self.new_change();
                        for player in 0..PLAYER_COUNT {
                            if let Some(card) = self.current_trick_regular[player] {
                                self.add_change(
                                    change_index,
                                    Change {
                                        change_type: ChangeType::TricksToWinner,
                                        object_id: card.id,
                                        dest: Location::Score,
                                        player: winner,
                                        ..Default::default()
                                    },
                                );
                            }
                            if let Some(card) = self.current_trick_huff[player] {
                                self.add_change(
                                    change_index,
                                    Change {
                                        change_type: ChangeType::TricksToWinner,
                                        object_id: card.id,
                                        dest: Location::Score,
                                        player: winner,
                                        ..Default::default()
                                    },
                                );
                            }
                            if let Some(card) = self.current_trick_puff[player] {
                                self.add_change(
                                    change_index,
                                    Change {
                                        change_type: ChangeType::TricksToWinner,
                                        object_id: card.id,
                                        dest: Location::Score,
                                        player: winner,
                                        ..Default::default()
                                    },
                                );
                            }
                        }

                        // Clear trick slots
                        self.current_trick_regular = [None; PLAYER_COUNT];
                        self.current_trick_huff = [None; PLAYER_COUNT];
                        self.current_trick_puff = [None; PLAYER_COUNT];

                        // Winner leads next trick
                        self.lead_player = winner;
                        self.current_player = winner;

                        // Check if round has ended (any player has no pig/wolf cards)
                        let round_ended = self
                            .hands
                            .iter()
                            .any(|hand| !hand.iter().any(|c| c.is_regular()));

                        if round_ended {
                            self.end_round();
                        }
                    }

                    self.show_playable();
                    self.show_message();
                }
            }
        }
    }

    /// Check if a player made their bid
    pub fn bid_succeeded(&self, player: usize) -> bool {
        let tricks = self.tricks_won[player];
        match self.bids[player] {
            Some(Bid::Sleep) => tricks == 0,
            Some(Bid::Play) => tricks == 2,
            Some(Bid::Work) => tricks >= 3,
            Some(Bid::Eat) => {
                let max_tricks = *self.tricks_won.iter().max().unwrap();
                let players_with_max = self.tricks_won.iter().filter(|&&t| t == max_tricks).count();
                tricks == max_tricks && players_with_max == 1
            }
            None => false,
        }
    }

    /// End the current round and calculate scores
    /// Shows each player one by one with their bid card and leftover modifiers
    #[allow(clippy::needless_range_loop)]
    pub fn end_round(&mut self) {
        // Calculate scores for each player first (we need these for the display)
        let old_scores = self.scores;
        let mut player_scores_breakdown: [(i32, i32, i32); PLAYER_COUNT] =
            [(0, 0, 0); PLAYER_COUNT]; // (tricks, modifier_penalty, bid_bonus)

        for player in 0..PLAYER_COUNT {
            let tricks = self.tricks_won[player] as i32;
            let leftover_modifiers = self.hands[player]
                .iter()
                .filter(|c| c.is_huff() || c.is_puff())
                .count() as i32;

            // +1 per trick won
            self.scores[player] += tricks;
            // -1 per leftover huff/puff in hand
            self.scores[player] -= leftover_modifiers;

            // Bid bonuses
            let bid_bonus = if let Some(bid) = self.bids[player] {
                match bid {
                    Bid::Sleep => {
                        if tricks == 0 {
                            12
                        } else {
                            0
                        }
                    }
                    Bid::Play => {
                        if tricks == 2 {
                            7
                        } else {
                            0
                        }
                    }
                    Bid::Work => {
                        if tricks >= 3 {
                            3
                        } else {
                            0
                        }
                    }
                    Bid::Eat => {
                        let max_tricks = *self.tricks_won.iter().max().unwrap();
                        let players_with_max =
                            self.tricks_won.iter().filter(|&&t| t == max_tricks).count();
                        if self.tricks_won[player] == max_tricks && players_with_max == 1 {
                            2 * tricks
                        } else {
                            0
                        }
                    }
                }
            } else {
                0
            };
            self.scores[player] += bid_bonus;
            player_scores_breakdown[player] = (tricks, leftover_modifiers, bid_bonus);
        }

        // Show each player's score one by one, starting with human player (player 0)
        let player_names = ["Your", "West player", "North player", "East player"];

        for player in 0..PLAYER_COUNT {
            let show_index = self.new_change();

            // Show message indicating whose score is being displayed
            let score_message = format!("{} score this hand", player_names[player]);
            self.add_change(
                show_index,
                Change {
                    change_type: ChangeType::Message,
                    message: Some(score_message),
                    player,
                    ..Default::default()
                },
            );

            // Show the bid card for this player centered on screen
            if let Some(bid) = self.bids[player] {
                let bid_card_id = -(10 + bid.to_index());
                self.add_change(
                    show_index,
                    Change {
                        change_type: ChangeType::ShowScoringBid,
                        object_id: bid_card_id,
                        dest: Location::ScoringCenter,
                        player,
                        // Include tricks/bid in message for display
                        message: Some(format!(
                            "{}/{}",
                            self.tricks_won[player],
                            bid.to_display_string()
                        )),
                        ..Default::default()
                    },
                );
            }

            // Show leftover huff/puff cards below the bid card
            // Collect card IDs first to avoid borrow issues
            let leftover_card_ids: Vec<i32> = self.hands[player]
                .iter()
                .filter(|c| c.is_huff() || c.is_puff())
                .map(|c| c.id)
                .collect();
            let leftover_count = leftover_card_ids.len();

            for (offset, card_id) in leftover_card_ids.iter().enumerate() {
                self.add_change(
                    show_index,
                    Change {
                        change_type: ChangeType::ShowLeftoverCards,
                        object_id: *card_id,
                        dest: Location::ScoringBelow,
                        player,
                        offset,
                        length: leftover_count,
                        ..Default::default()
                    },
                );
            }

            // Update trick counter to show tricks/bid for this player (all players during scoring)
            if let Some(bid) = self.bids[player] {
                self.add_change(
                    show_index,
                    Change {
                        change_type: ChangeType::UpdateTrickCount,
                        player,
                        trick_count: self.tricks_won[player] as i32,
                        message: Some(bid.to_display_string().to_string()),
                        ..Default::default()
                    },
                );
            }

            // Show score preview for this player
            self.add_change(
                show_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: old_scores[player],
                    end_score: self.scores[player],
                    animate_score: false,
                    ..Default::default()
                },
            );

            // Wait for input before showing next player
            self.add_change(
                show_index,
                Change {
                    change_type: ChangeType::OptionalPause,
                    object_id: -1,
                    player,
                    ..Default::default()
                },
            );

            // Hide the scoring cards before moving to next player
            let hide_index = self.new_change();
            self.add_change(
                hide_index,
                Change {
                    change_type: ChangeType::HideScoringCards,
                    player,
                    ..Default::default()
                },
            );

            // Animate score for this player
            self.add_change(
                hide_index,
                Change {
                    change_type: ChangeType::Score,
                    player,
                    start_score: old_scores[player],
                    end_score: self.scores[player],
                    animate_score: true,
                    ..Default::default()
                },
            );
        }

        // Advance to next round
        self.current_round += 1;

        // Reset for next round (if game not over)
        if self.current_round <= ROUNDS {
            self.tricks_won = [0; PLAYER_COUNT];
            self.bids = [None; PLAYER_COUNT];
            self.wolf_suit_broken = false;
            self.state = State::Bid;
            // Lead player rotates clockwise each round
            self.lead_player = (self.lead_player + 1) % PLAYER_COUNT;
            self.current_player = self.lead_player;
            // Clear hands - leftover huff/puff cards have been scored
            self.hands = Default::default();
            self.voids = Default::default();

            // Deal new hands for next round
            self.deal(true);
        } else {
            // Game over
            let max_score = self.scores.iter().max().unwrap();
            for player in 0..PLAYER_COUNT {
                if self.scores[player] == *max_score {
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
        }
    }

    /// Check if the game is over (4 rounds completed)
    pub fn is_game_over(&self) -> bool {
        self.current_round > ROUNDS
    }
}

/// ISMCTS Game trait implementation
impl ismcts::Game for ThreeTrickyPigsGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        // Force single-hand evaluation: set to last round and reset scores
        self.current_round = ROUNDS;
        self.scores = [0; PLAYER_COUNT];

        let rng = &mut thread_rng();

        // Randomize hidden bids - each non-observer player could have chosen any bid
        let all_bids = [Bid::Sleep, Bid::Play, Bid::Work, Bid::Eat];
        for player in 0..PLAYER_COUNT {
            if player != observer && self.bids[player].is_some() {
                self.bids[player] = Some(*all_bids.choose(rng).unwrap());
            }
        }

        // Shuffle hands between each pair of non-observer players, respecting void constraints
        // This properly randomizes hidden information while maintaining valid hands
        for p1 in 0..PLAYER_COUNT {
            for p2 in (p1 + 1)..PLAYER_COUNT {
                if p1 == observer || p2 == observer {
                    continue;
                }

                // Combine voids from both players - cards in these suits can't be exchanged
                let mut combined_voids = [false; 6]; // 6 suits: Straw, Sticks, Bricks, Wolf, Huff, Puff
                for suit in &self.voids[p1] {
                    combined_voids[*suit as usize] = true;
                }
                for suit in &self.voids[p2] {
                    combined_voids[*suit as usize] = true;
                }

                // Shuffle cards between these two players for non-void suits
                let mut hands = vec![self.hands[p1].clone(), self.hands[p2].clone()];
                crate::utils::shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids[c.suit as usize],
                    &mut hands,
                    rng,
                );
                self.hands[p1] = hands[0].clone();
                self.hands[p2] = hands[1].clone();
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
        // Only evaluate when the round/game is over
        if !self.is_game_over() {
            return None;
        }

        // Rank-based evaluation - tournament testing showed this performs best
        let scores = self.scores;
        let mut score_ranks: Vec<(i32, usize)> =
            scores.iter().enumerate().map(|(i, &s)| (s, i)).collect();
        score_ranks.sort_by_key(|&(score, _)| std::cmp::Reverse(score));

        let player_rank = score_ranks.iter().position(|(_, p)| *p == player).unwrap();
        match player_rank {
            0 => Some(1.0),
            1 => Some(0.33),
            2 => Some(-0.33),
            3 => Some(-1.0),
            _ => Some(0.0),
        }
    }
}

/// Get the best move using ISMCTS
pub fn get_mcts_move(game: &ThreeTrickyPigsGame, iterations: i32) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 4;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    ismcts.best_move().expect("should have a move to make")
}

pub fn deck() -> Vec<Card> {
    let distributions: Vec<(Suit, Vec<i32>)> = vec![
        (Suit::Straw, (1..=10).collect()),
        (Suit::Sticks, (1..=10).collect()),
        (Suit::Bricks, (21..=30).collect()),
        (
            Suit::Wolf,
            vec![0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 31],
        ),
        (Suit::Huff, (1..=4).collect()),
        (Suit::Puff, (2..=5).collect()),
    ];
    let mut id = 0;
    let mut cards: Vec<Card> = vec![];
    for (suit, values) in &distributions {
        for value in values {
            cards.push(Card {
                value: *value,
                suit: *suit,
                id,
            });
            id += 1;
        }
    }
    cards
}

/// Returns the player index that won the trick
fn trick_winner(
    lead_player: usize,
    trick_regular: [Option<Card>; PLAYER_COUNT],
    trick_huff: [Option<Card>; PLAYER_COUNT],
    trick_puff: [Option<Card>; PLAYER_COUNT],
) -> usize {
    let empty_card = Card {
        id: -1,
        value: 0,
        suit: Suit::Bricks,
    };
    let mut winning_player = lead_player;
    let mut winning_value = trick_regular[lead_player].unwrap().value;
    let contains_wolf = trick_regular.iter().any(|c| c.unwrap().suit == Suit::Wolf);
    for offset in 0..PLAYER_COUNT {
        let current_player = (offset + lead_player) % PLAYER_COUNT;
        let card_value = trick_regular[current_player].unwrap().value
            + trick_huff[current_player].unwrap_or(empty_card).value
            + trick_puff[current_player].unwrap_or(empty_card).value;
        let winning = if contains_wolf {
            // When at least one wolf card is in the trick the highest card wins
            // (later played cards win ties)
            card_value >= winning_value
        } else {
            // When all cards are pig cards the lowest value card wins
            // (later played cards win ties)
            card_value <= winning_value
        };
        if winning {
            winning_value = card_value;
            winning_player = current_player;
        }
    }
    winning_player
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_composition() {
        let d = deck();
        // Total card count
        assert_eq!(d.len(), 50);
    }

    // Helper to create a card
    fn card(value: i32, suit: Suit) -> Option<Card> {
        Some(Card {
            id: -1,
            value,
            suit,
        })
    }

    // Helper for no huff/puff
    fn no_modifiers() -> [Option<Card>; PLAYER_COUNT] {
        [None; PLAYER_COUNT]
    }

    // Rulebook Example 6: No wolf, lowest card wins
    // Player 0 leads 5, Player 1 plays 10+5 puff, Player 2 plays 2
    // Player 2 wins with lowest value (2)
    #[test]
    fn test_no_wolf_lowest_wins() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(10, Suit::Straw),
            card(2, Suit::Straw),
            card(7, Suit::Straw),
        ];
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 2);
    }

    // Rulebook Example (wolf present): Player 0 leads 5, Player 1 plays 10+5 puff,
    // Player 2 can't follow and plays 14 (wolf), highest card wins
    // 10+5=15 beats 14, so Player 1 wins
    #[test]
    fn test_wolf_present_highest_wins() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(10, Suit::Straw),
            card(14, Suit::Wolf),
            card(8, Suit::Bricks),
        ];
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 1); // 10+5=15 is highest
    }

    // Rulebook Example 7: No wolf, off-suit doesn't matter for winner calculation
    // Player 0 leads 21, Player 1 plays 8 (off-suit)
    // In a 2-relevant-player scenario, 8 < 21, so Player 1 would win (lowest)
    #[test]
    fn test_no_wolf_lower_value_wins_regardless_of_suit() {
        let trick_regular = [
            card(21, Suit::Bricks),
            card(8, Suit::Straw),
            card(25, Suit::Bricks),
            card(28, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 1); // 8 is lowest
    }

    // Rulebook Example 8: Tie goes to last played card
    // Player 0 leads 5, Player 1 plays 28, Player 2 plays 8, Player 3 plays 8
    // No wolf, lowest wins. 5 is lowest, Player 0 wins.
    // (Separate test below covers the tie-breaker scenario)
    #[test]
    fn test_no_wolf_example_8() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(28, Suit::Straw),
            card(8, Suit::Straw),
            card(8, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0); // 5 is lowest, Player 0 wins
    }

    // Tie-breaker test: Two players have the same lowest value
    // No wolf, lowest wins. Two 5s tie, last played wins.
    #[test]
    fn test_no_wolf_tie_goes_to_last_played() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(28, Suit::Straw),
            card(8, Suit::Straw),
            card(5, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3); // Last played 5 wins the tie
    }

    // Wolf present, tie goes to last played (highest)
    #[test]
    fn test_wolf_present_tie_goes_to_last_played() {
        let trick_regular = [
            card(15, Suit::Wolf),
            card(10, Suit::Straw),
            card(15, Suit::Sticks),
            card(8, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 2); // Second 15 wins (last played among ties)
    }

    // Test huff modifier adds to value
    #[test]
    fn test_huff_adds_to_value() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(3, Suit::Straw),
            card(6, Suit::Straw),
            card(7, Suit::Straw),
        ];
        // Player 1 has 3, but +4 huff makes it 7
        let trick_huff = [None, card(4, Suit::Huff), None, None];

        let winner = trick_winner(0, trick_regular, trick_huff, no_modifiers());
        // Without huff: 3 wins (lowest). With huff: 3+4=7, so 5 is now lowest
        assert_eq!(winner, 0);
    }

    // Test huff and puff together
    #[test]
    fn test_huff_and_puff_combined() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(2, Suit::Straw),
            card(6, Suit::Straw),
            card(7, Suit::Straw),
        ];
        // Player 1 has 2, but +3 huff +4 puff makes it 9
        let trick_huff = [None, card(3, Suit::Huff), None, None];
        let trick_puff = [None, card(4, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, trick_huff, trick_puff);
        // 2+3+4=9, so 5 is now lowest
        assert_eq!(winner, 0);
    }

    // Test with non-zero lead player
    #[test]
    fn test_lead_player_not_zero() {
        let trick_regular = [
            card(10, Suit::Straw), // Player 0
            card(8, Suit::Straw),  // Player 1
            card(5, Suit::Straw),  // Player 2 (leads)
            card(9, Suit::Straw),  // Player 3
        ];

        // Player 2 leads, play order is 2, 3, 0, 1
        let winner = trick_winner(2, trick_regular, no_modifiers(), no_modifiers());
        // No wolf, lowest wins: 5 (P2), 9 (P3), 10 (P0), 8 (P1)
        // Lowest is 5, Player 2 wins
        assert_eq!(winner, 2);
    }

    // Test lead player wrap-around with tie
    #[test]
    fn test_lead_player_wraparound_tie() {
        let trick_regular = [
            card(5, Suit::Straw),  // Player 0
            card(8, Suit::Straw),  // Player 1
            card(5, Suit::Sticks), // Player 2 (leads)
            card(9, Suit::Straw),  // Player 3
        ];

        // Player 2 leads, play order is 2, 3, 0, 1
        // Tie between P2 (5) and P0 (5), P0 plays later, P0 wins
        let winner = trick_winner(2, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0);
    }

    // Wolf card with value 0 (special wolf card)
    #[test]
    fn test_wolf_zero_value() {
        let trick_regular = [
            card(0, Suit::Wolf),
            card(5, Suit::Straw),
            card(3, Suit::Sticks),
            card(2, Suit::Bricks),
        ];

        // Wolf present, highest wins. 5 is highest.
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 1);
    }

    // Wolf card with value 31 (highest wolf)
    #[test]
    fn test_wolf_31_value() {
        let trick_regular = [
            card(31, Suit::Wolf),
            card(30, Suit::Bricks),
            card(28, Suit::Bricks),
            card(25, Suit::Bricks),
        ];

        // Wolf present, highest wins. 31 is highest.
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0);
    }

    // Multiple wolves in trick
    #[test]
    fn test_multiple_wolves() {
        let trick_regular = [
            card(11, Suit::Wolf),
            card(15, Suit::Wolf),
            card(15, Suit::Wolf),
            card(12, Suit::Wolf),
        ];

        // All wolves, highest wins, tie goes to last played
        // P1 and P2 both have 15, P2 played later
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 2);
    }

    // Huff/puff with wolf - modifiers still apply
    #[test]
    fn test_wolf_with_modifiers() {
        let trick_regular = [
            card(14, Suit::Wolf),
            card(10, Suit::Straw),
            card(8, Suit::Sticks),
            card(7, Suit::Bricks),
        ];
        // Player 1 has 10 + 5 puff = 15, beats wolf 14
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 1);
    }

    // All same values, last player wins (no wolf)
    #[test]
    fn test_all_same_value_no_wolf() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(5, Suit::Sticks),
            card(5, Suit::Bricks),
            card(5, Suit::Straw),
        ];

        // All 5s, no wolf, last played wins (Player 3)
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3);
    }

    // All same values, last player wins (with wolf)
    #[test]
    fn test_all_same_value_with_wolf() {
        let trick_regular = [
            card(15, Suit::Wolf),
            card(15, Suit::Sticks),
            card(15, Suit::Bricks),
            card(15, Suit::Straw),
        ];

        // All 15s, wolf present, last played wins (Player 3)
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3);
    }

    // ==================== get_moves tests ====================

    // Helper to create a card with id
    fn card_with_id(id: i32, value: i32, suit: Suit) -> Card {
        Card { id, value, suit }
    }

    // Helper to create a game with specific state
    fn game_with_hand(
        current_player: usize,
        lead_player: usize,
        hand: Vec<Card>,
        trick_regular: [Option<Card>; PLAYER_COUNT],
        trick_huff: [Option<Card>; PLAYER_COUNT],
        trick_puff: [Option<Card>; PLAYER_COUNT],
        wolf_suit_broken: bool,
    ) -> ThreeTrickyPigsGame {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[current_player] = hand;
        ThreeTrickyPigsGame {
            state: State::Play,
            current_player,
            lead_player,
            wolf_suit_broken,
            current_trick_regular: trick_regular,
            current_trick_huff: trick_huff,
            current_trick_puff: trick_puff,
            hands,
            current_round: 1,
            ..Default::default()
        }
    }

    // Bid state returns all 4 bid options
    #[test]
    fn test_get_moves_bid_state() {
        let game = ThreeTrickyPigsGame {
            state: State::Bid,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: false,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands: Default::default(),
            bids: [None; PLAYER_COUNT],
            tricks_won: [0; PLAYER_COUNT],
            current_round: 1,
            ..Default::default()
        };
        let moves = game.get_moves();
        assert_eq!(moves, vec![0, 1, 2, 3]);
    }

    // Leading player (no lead card) can play any card
    #[test]
    fn test_get_moves_leading_player_any_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 10, Suit::Sticks),
            card_with_id(2, 25, Suit::Bricks),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // All cards playable when leading
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
    }

    // Must follow suit when able
    #[test]
    fn test_get_moves_must_follow_suit() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 7, Suit::Straw),
            card_with_id(2, 25, Suit::Bricks),
        ];

        // Player 0 led with Straw
        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // Only Straw cards playable
        assert!(moves.contains(&0)); // Straw
        assert!(moves.contains(&1)); // Straw
        assert!(!moves.contains(&2)); // Bricks - can't play
    }

    // Can play any card if can't follow suit
    #[test]
    fn test_get_moves_cant_follow_suit() {
        let hand = vec![
            card_with_id(0, 25, Suit::Bricks),
            card_with_id(1, 26, Suit::Bricks),
            card_with_id(2, 15, Suit::Wolf),
        ];

        // Player 0 led with Straw
        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // No Straw in hand, can play anything
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
    }

    // Huff cards are playable when not already played
    #[test]
    fn test_get_moves_huff_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card
        assert!(moves.contains(&1)); // huff card
    }

    // Huff cards not playable when already played this trick
    #[test]
    fn test_get_moves_huff_already_played() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let mut trick_huff = no_modifiers();
        trick_huff[0] = Some(Card {
            id: 99,
            value: 1,
            suit: Suit::Huff,
        });

        let game = game_with_hand(0, 0, hand, no_modifiers(), trick_huff, no_modifiers(), true);
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card still playable
        assert!(!moves.contains(&1)); // huff card NOT playable
    }

    // Puff cards are playable when not already played
    #[test]
    fn test_get_moves_puff_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card
        assert!(moves.contains(&1)); // puff card
    }

    // Puff cards not playable when already played this trick
    #[test]
    fn test_get_moves_puff_already_played() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let mut trick_puff = no_modifiers();
        trick_puff[0] = Some(Card {
            id: 99,
            value: 2,
            suit: Suit::Puff,
        });

        let game = game_with_hand(0, 0, hand, no_modifiers(), no_modifiers(), trick_puff, true);
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card still playable
        assert!(!moves.contains(&1)); // puff card NOT playable
    }

    // Both huff and puff playable
    #[test]
    fn test_get_moves_huff_and_puff_both_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
            card_with_id(2, 3, Suit::Puff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular
        assert!(moves.contains(&1)); // huff
        assert!(moves.contains(&2)); // puff
    }

    // Following player with huff/puff in hand, must follow suit for regular
    #[test]
    fn test_get_moves_follow_suit_with_huff_puff() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 25, Suit::Bricks),
            card_with_id(2, 2, Suit::Huff),
            card_with_id(3, 3, Suit::Puff),
        ];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - must follow
        assert!(!moves.contains(&1)); // Bricks - can't play
        assert!(moves.contains(&2)); // Huff - always playable if not used
        assert!(moves.contains(&3)); // Puff - always playable if not used
    }

    // Player 2 following, player 0 led
    #[test]
    fn test_get_moves_different_lead_and_current_player() {
        let hand = vec![
            card_with_id(0, 5, Suit::Sticks),
            card_with_id(1, 25, Suit::Bricks),
        ];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Sticks,
        });
        trick_regular[1] = Some(Card {
            id: 98,
            value: 7,
            suit: Suit::Sticks,
        });

        let game = game_with_hand(
            2,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Sticks - follows suit
        assert!(!moves.contains(&1)); // Bricks - can't play
    }

    // Cannot lead wolf when wolf not broken
    #[test]
    fn test_get_moves_cannot_lead_wolf_when_not_broken() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 15, Suit::Wolf),
        ];

        // Leading (no cards played), wolf not broken
        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            false,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - can lead
        assert!(!moves.contains(&1)); // Wolf - cannot lead when not broken
    }

    // Can lead wolf when wolf is broken
    #[test]
    fn test_get_moves_can_lead_wolf_when_broken() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 15, Suit::Wolf),
        ];

        // Leading (no cards played), wolf IS broken
        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - can lead
        assert!(moves.contains(&1)); // Wolf - can lead when broken
    }

    // ==================== apply_move tests ====================

    // Helper to create a game in bid state
    fn game_in_bid_state() -> ThreeTrickyPigsGame {
        ThreeTrickyPigsGame {
            state: State::Bid,
            current_round: 1,
            ..Default::default()
        }
    }

    // Bidding advances current player
    #[test]
    fn test_apply_move_bid_advances_player() {
        let mut game = game_in_bid_state();
        assert_eq!(game.current_player, 0);

        game.apply_move(0); // Player 0 bids Sleep
        assert_eq!(game.current_player, 1);

        game.apply_move(1); // Player 1 bids Play
        assert_eq!(game.current_player, 2);
    }

    // All bids complete transitions to Play state
    #[test]
    fn test_apply_move_bid_transitions_to_play() {
        let mut game = game_in_bid_state();

        game.apply_move(0); // Player 0
        game.apply_move(1); // Player 1
        game.apply_move(2); // Player 2
        assert!(matches!(game.state, State::Bid)); // Still bidding

        game.apply_move(3); // Player 3
        assert!(matches!(game.state, State::Play)); // Now playing
    }

    // Playing a regular card removes it from hand
    #[test]
    fn test_apply_move_removes_card_from_hand() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 10, Suit::Sticks),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        assert_eq!(game.hands[0].len(), 2);
        game.apply_move(0); // Play Straw card
        assert_eq!(game.hands[0].len(), 1);
        assert_eq!(game.hands[0][0].id, 1); // Only Sticks card remains
    }

    // Playing a regular card places it in trick
    #[test]
    fn test_apply_move_places_regular_card_in_trick() {
        let hand = vec![card_with_id(0, 5, Suit::Straw)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(0);
        assert!(game.current_trick_regular[0].is_some());
        assert_eq!(game.current_trick_regular[0].unwrap().id, 0);
    }

    // Playing a huff card places it in huff slot
    #[test]
    fn test_apply_move_places_huff_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(1); // Play huff
        assert!(game.current_trick_huff[0].is_some());
        assert_eq!(game.current_trick_huff[0].unwrap().id, 1);
        // Player doesn't advance after huff
        assert_eq!(game.current_player, 0);
    }

    // Playing a puff card places it in puff slot
    #[test]
    fn test_apply_move_places_puff_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(1); // Play puff
        assert!(game.current_trick_puff[0].is_some());
        assert_eq!(game.current_trick_puff[0].unwrap().id, 1);
        // Player doesn't advance after puff
        assert_eq!(game.current_player, 0);
    }

    // Regular card advances to next player
    #[test]
    fn test_apply_move_regular_card_advances_player() {
        let hand = vec![card_with_id(0, 5, Suit::Straw)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        assert_eq!(game.current_player, 0);
        game.apply_move(0);
        assert_eq!(game.current_player, 1);
    }

    // Complete trick determines winner and clears slots
    #[test]
    fn test_apply_move_complete_trick() {
        // Set up a 4-player trick where player 0 leads
        // Each player has an extra card so round doesn't end
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(10, 1, Suit::Sticks),
        ];
        hands[1] = vec![
            card_with_id(1, 3, Suit::Straw),
            card_with_id(11, 2, Suit::Sticks),
        ]; // Lowest - will win
        hands[2] = vec![
            card_with_id(2, 7, Suit::Straw),
            card_with_id(12, 3, Suit::Sticks),
        ];
        hands[3] = vec![
            card_with_id(3, 9, Suit::Straw),
            card_with_id(13, 4, Suit::Sticks),
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            current_round: 1,
            ..Default::default()
        };

        game.apply_move(0); // Player 0 plays 5
        game.apply_move(1); // Player 1 plays 3
        game.apply_move(2); // Player 2 plays 7
        game.apply_move(3); // Player 3 plays 9

        // Player 1 wins (lowest card, no wolf)
        assert_eq!(game.tricks_won[1], 1);
        assert_eq!(game.lead_player, 1);
        assert_eq!(game.current_player, 1);

        // Trick slots cleared
        assert!(game.current_trick_regular.iter().all(|c| c.is_none()));
    }

    // Wolf breaks when played because can't follow suit
    #[test]
    fn test_apply_move_wolf_breaks() {
        // Player 0 leads Straw, Player 1 has no Straw so plays Wolf
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[1] = vec![card_with_id(1, 15, Suit::Wolf)];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 0,
            value: 5,
            suit: Suit::Straw,
        });

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 1,
            lead_player: 0,
            wolf_suit_broken: false,
            current_trick_regular: trick_regular,
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            current_round: 1,
            ..Default::default()
        };

        assert!(!game.wolf_suit_broken);
        game.apply_move(1); // Player 1 plays Wolf
        assert!(game.wolf_suit_broken);
    }

    // Wolf doesn't break when leading (after already broken)
    #[test]
    fn test_apply_move_wolf_lead_doesnt_rebreak() {
        let hand = vec![card_with_id(0, 15, Suit::Wolf)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true, // Already broken
        );

        game.apply_move(0); // Lead with Wolf
        assert!(game.wolf_suit_broken); // Still broken (unchanged)
    }

    // Invalid move panics
    #[test]
    #[should_panic(expected = "Invalid move")]
    fn test_apply_move_invalid_move_panics() {
        let hand = vec![card_with_id(0, 5, Suit::Straw)];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        game.apply_move(99); // Invalid card id
    }

    // Huff then puff then regular card sequence
    #[test]
    fn test_apply_move_huff_puff_regular_sequence() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
            card_with_id(2, 3, Suit::Puff),
        ];

        let mut game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );

        // Play huff - stays on same player
        game.apply_move(1);
        assert_eq!(game.current_player, 0);
        assert!(game.current_trick_huff[0].is_some());

        // Play puff - stays on same player
        game.apply_move(2);
        assert_eq!(game.current_player, 0);
        assert!(game.current_trick_puff[0].is_some());

        // Play regular - advances player
        game.apply_move(0);
        assert_eq!(game.current_player, 1);
        assert!(game.current_trick_regular[0].is_some());
    }

    // Trick winner with huff/puff modifiers
    #[test]
    fn test_apply_move_trick_winner_with_modifiers() {
        // Each player has an extra regular card so round doesn't end
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 2, Suit::Straw),
            card_with_id(10, 4, Suit::Huff),
            card_with_id(20, 5, Suit::Sticks), // Extra regular card
        ];
        hands[1] = vec![
            card_with_id(1, 8, Suit::Straw),
            card_with_id(11, 1, Suit::Sticks),
        ];
        hands[2] = vec![
            card_with_id(2, 9, Suit::Straw),
            card_with_id(12, 2, Suit::Sticks),
        ];
        hands[3] = vec![
            card_with_id(3, 7, Suit::Straw),
            card_with_id(13, 3, Suit::Sticks),
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            current_round: 1,
            ..Default::default()
        };

        // Player 0 plays huff (+4) then 2 = 6 total
        game.apply_move(10); // Huff
        game.apply_move(0); // 2 Straw (total 6)
        game.apply_move(1); // Player 1: 8
        game.apply_move(2); // Player 2: 9
        game.apply_move(3); // Player 3: 7

        // Player 0 wins with 6 (2 base + 4 huff), lowest value
        assert_eq!(game.tricks_won[0], 1);
    }

    // ==================== Round end tests ====================

    // Round ends when any player has no regular cards
    #[test]
    fn test_round_ends_when_player_has_no_regular_cards() {
        // Each player has exactly one card - round ends after one trick
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Work); PLAYER_COUNT], // Everyone bid Work
            current_round: 1,
            ..Default::default()
        };

        assert_eq!(game.current_round, 1);

        // Play the trick
        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Round should have ended and advanced
        assert_eq!(game.current_round, 2);
        assert_eq!(game.state, State::Bid);
    }

    // Scoring: +1 per trick won
    #[test]
    fn test_scoring_tricks_won() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)]; // Wins
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            current_round: 1,
            ..Default::default()
        };

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 1 won 1 trick, gets +1 point
        assert_eq!(game.scores[1], 1);
        assert_eq!(game.scores[0], 0);
    }

    // Scoring: -1 per leftover huff/puff
    #[test]
    fn test_scoring_leftover_modifiers() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(10, 2, Suit::Huff),
            card_with_id(11, 3, Suit::Puff),
        ];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            current_round: 1,
            ..Default::default()
        };

        game.apply_move(0); // Player 0 plays regular, keeps huff and puff
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 0 has 2 leftover modifiers: -2 points
        assert_eq!(game.scores[0], -2);
    }

    // Scoring: Sleep bid (+12 if 0 tricks)
    #[test]
    fn test_scoring_sleep_bid_success() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 10, Suit::Straw)]; // High card, won't win
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)]; // Wins
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Sleep), None, None, None],
            current_round: 1,
            ..Default::default()
        };

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 0 won 0 tricks with Sleep bid: +12 points
        assert_eq!(game.scores[0], 12);
    }

    // Scoring: Play bid (+7 if exactly 2 tricks)
    #[test]
    fn test_scoring_play_bid_success() {
        // Set up for 2 tricks where player 0 wins both
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 1, Suit::Straw),
            card_with_id(4, 1, Suit::Sticks),
        ];
        hands[1] = vec![
            card_with_id(1, 5, Suit::Straw),
            card_with_id(5, 5, Suit::Sticks),
        ];
        hands[2] = vec![
            card_with_id(2, 6, Suit::Straw),
            card_with_id(6, 6, Suit::Sticks),
        ];
        hands[3] = vec![
            card_with_id(3, 7, Suit::Straw),
            card_with_id(7, 7, Suit::Sticks),
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Play), None, None, None],
            current_round: 1,
            ..Default::default()
        };

        // Trick 1
        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Trick 2
        game.apply_move(4);
        game.apply_move(5);
        game.apply_move(6);
        game.apply_move(7);

        // Player 0 won 2 tricks with Play bid: 2 + 7 = 9 points
        assert_eq!(game.scores[0], 9);
    }

    // Scoring: Work bid (+3 if 3+ tricks)
    #[test]
    fn test_scoring_work_bid_success() {
        // Set up for 3 tricks where player 0 wins all
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![
            card_with_id(0, 1, Suit::Straw),
            card_with_id(4, 1, Suit::Sticks),
            card_with_id(8, 21, Suit::Bricks),
        ];
        hands[1] = vec![
            card_with_id(1, 5, Suit::Straw),
            card_with_id(5, 5, Suit::Sticks),
            card_with_id(9, 25, Suit::Bricks),
        ];
        hands[2] = vec![
            card_with_id(2, 6, Suit::Straw),
            card_with_id(6, 6, Suit::Sticks),
            card_with_id(10, 26, Suit::Bricks),
        ];
        hands[3] = vec![
            card_with_id(3, 7, Suit::Straw),
            card_with_id(7, 7, Suit::Sticks),
            card_with_id(11, 27, Suit::Bricks),
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Work), None, None, None],
            current_round: 1,
            ..Default::default()
        };

        // Play 3 tricks
        for _ in 0..3 {
            let moves = game.get_moves();
            game.apply_move(moves[0]);
            let moves = game.get_moves();
            game.apply_move(moves[0]);
            let moves = game.get_moves();
            game.apply_move(moves[0]);
            let moves = game.get_moves();
            game.apply_move(moves[0]);
        }

        // Player 0 won 3 tricks with Work bid: 3 + 3 = 6 points
        assert_eq!(game.scores[0], 6);
    }

    // Scoring: Eat bid (+2 per trick if most tricks)
    #[test]
    fn test_scoring_eat_bid_success() {
        // Player 0 wins the only trick
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 1, Suit::Straw)]; // Lowest, wins
        hands[1] = vec![card_with_id(1, 5, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 6, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 7, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: true,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands,
            bids: [Some(Bid::Eat), None, None, None],
            current_round: 1,
            ..Default::default()
        };

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        // Player 0 won 1 trick (most) with Eat bid: 1 + 2*1 = 3 points
        assert_eq!(game.scores[0], 3);
    }

    // Scoring: Eat bid fails on tie (no bonus if tied for most tricks)
    #[test]
    fn test_scoring_eat_bid_tie_no_bonus() {
        // Players 0 and 1 each win 1 trick - tied for most
        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            wolf_suit_broken: true,
            bids: [Some(Bid::Eat), Some(Bid::Eat), None, None],
            tricks_won: [1, 1, 0, 0], // Tied for most
            current_round: 1,
            ..Default::default()
        };

        game.end_round();

        // Neither player gets the Eat bonus due to tie
        // Player 0: 1 trick = 1 point (no 2x bonus)
        // Player 1: 1 trick = 1 point (no 2x bonus)
        assert_eq!(game.scores[0], 1);
        assert_eq!(game.scores[1], 1);
    }

    // Game ends after 4 rounds
    #[test]
    fn test_game_ends_after_four_rounds() {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[0] = vec![card_with_id(0, 5, Suit::Straw)];
        hands[1] = vec![card_with_id(1, 3, Suit::Straw)];
        hands[2] = vec![card_with_id(2, 7, Suit::Straw)];
        hands[3] = vec![card_with_id(3, 9, Suit::Straw)];

        let mut game = ThreeTrickyPigsGame {
            state: State::Play,
            wolf_suit_broken: true,
            hands,
            current_round: 4, // Last round
            ..Default::default()
        };

        assert!(!game.is_game_over());

        game.apply_move(0);
        game.apply_move(1);
        game.apply_move(2);
        game.apply_move(3);

        assert!(game.is_game_over());
        assert_eq!(game.current_round, 5);
    }

    #[test]
    fn test_show_playable_after_all_bids() {
        // Set up a game where human (player 0) is lead_player
        let hands: [Vec<Card>; PLAYER_COUNT] = [
            vec![
                card_with_id(0, 1, Suit::Straw),
                card_with_id(1, 2, Suit::Straw),
            ],
            vec![
                card_with_id(2, 3, Suit::Straw),
                card_with_id(3, 4, Suit::Straw),
            ],
            vec![
                card_with_id(4, 5, Suit::Straw),
                card_with_id(5, 6, Suit::Straw),
            ],
            vec![
                card_with_id(6, 7, Suit::Straw),
                card_with_id(7, 8, Suit::Straw),
            ],
        ];

        let mut game = ThreeTrickyPigsGame {
            state: State::Bid,
            current_player: 0,
            lead_player: 0,
            hands,
            undo_players: HashSet::from([0]),
            ..Default::default()
        };

        // Human selects bid
        game.apply_move(0); // Select Sleep bid
        assert_eq!(game.state, State::BidConfirm);

        // Human confirms bid
        game.apply_move(0); // Confirm Sleep bid
        assert_eq!(game.state, State::Bid);
        assert_eq!(game.current_player, 1);

        // AI player 1 bids
        game.apply_move(1); // Play bid
        assert_eq!(game.current_player, 2);

        // AI player 2 bids
        game.apply_move(2); // Work bid
        assert_eq!(game.current_player, 3);

        // AI player 3 bids - this should transition to Play state
        game.apply_move(3); // Eat bid
        assert_eq!(game.state, State::Play);
        assert_eq!(game.current_player, 0); // Human should be current player (lead_player)

        // Check that ShowPlayable changes were emitted for human's cards
        let show_playable_changes: Vec<&Change> = game
            .changes
            .iter()
            .flat_map(|group| group.iter())
            .filter(|c| c.change_type == ChangeType::ShowPlayable)
            .collect();

        assert!(
            !show_playable_changes.is_empty(),
            "ShowPlayable changes should be emitted after all bids"
        );

        // Should have ShowPlayable for each playable card in human's hand
        let playable_ids: Vec<i32> = show_playable_changes.iter().map(|c| c.object_id).collect();
        assert!(playable_ids.contains(&0), "Card 0 should be playable");
        assert!(playable_ids.contains(&1), "Card 1 should be playable");
    }
}
