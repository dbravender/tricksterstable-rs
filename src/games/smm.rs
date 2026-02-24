/*
Game: Slot Machine Millionaire
Designer: Jon Barron
Artist: Racknar Teyssier

A shedding/climbing game where players empty their hands by playing melds.
Meld power is determined by a dynamic hierarchy that changes during play.
*/

use enum_iterator::Sequence;
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

const PLAYER_COUNT: usize = 3;
const CARDS_PER_PLAYER: usize = 15;
const CARDS_PER_SUIT: usize = 8;
const SUIT_COUNT: usize = 7;
const ROUNDS: usize = 3;

const PASS: i32 = -1;
const USE_LUCKY_COIN: i32 = -2;

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
    Cherry = 0,
    Diamond = 1,
    Bell = 2,
    Clover = 3,
    Horseshoe = 4,
    Bar = 5,
    Seven = 6,
}

impl Suit {
    pub fn all() -> [Suit; SUIT_COUNT] {
        [
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ]
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i32,
    pub suit: Suit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Play,
    GameOver,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    Discard,
    Hierarchy,
    Score,
    Message,
    LuckyCoin,
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
    Message,
    Score,
    GameOver,
    OptionalPause,
    Reorder,
    ClearMeld,
    UpdateHierarchy,
    UseLuckyCoin,
    PlayerOut,
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
    pub suits: Option<Vec<Suit>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct SMMGame {
    pub hands: [Vec<Card>; PLAYER_COUNT],
    pub state: State,
    pub changes: Vec<Vec<Change>>,
    pub no_changes: bool,
    pub scores: [i32; PLAYER_COUNT],
    pub winner: Option<usize>,
    pub current_player: usize,
    pub round: i32,

    // Hierarchy: index 0 = highest power, index 6 = lowest
    pub hierarchy: Vec<Suit>,

    // Current meld on table
    pub current_meld: Vec<Card>,
    pub current_meld_suit: Option<Suit>,
    pub meld_leader: Option<usize>,

    // Pass tracking (soft pass)
    pub passed_this_round: [bool; PLAYER_COUNT],
    pub consecutive_passes: usize,

    // Lucky coin per player
    pub has_lucky_coin: [bool; PLAYER_COUNT],

    // Shed out tracking
    pub shed_out_order: Vec<usize>,
}

impl SMMGame {
    pub fn new() -> Self {
        let mut game = SMMGame {
            hierarchy: Suit::all().to_vec(),
            has_lucky_coin: [true; PLAYER_COUNT],
            ..Default::default()
        };
        game.deal(true);
        game
    }

    fn deal(&mut self, animate: bool) {
        let mut deck = SMMGame::deck();

        self.hands = [
            deck.drain(..CARDS_PER_PLAYER).collect(),
            deck.drain(..CARDS_PER_PLAYER).collect(),
            deck.drain(..CARDS_PER_PLAYER).collect(),
        ];

        self.current_meld.clear();
        self.current_meld_suit = None;
        self.meld_leader = None;
        self.passed_this_round = [false; PLAYER_COUNT];
        self.consecutive_passes = 0;
        self.shed_out_order.clear();
        self.state = State::Play;

        // First round: random start. Later: lowest score starts
        if self.round == 0 {
            self.current_player = thread_rng().gen_range(0..PLAYER_COUNT);
        } else {
            let min_score = *self.scores.iter().min().unwrap();
            let lowest: Vec<usize> = self
                .scores
                .iter()
                .enumerate()
                .filter(|(_, &s)| s == min_score)
                .map(|(i, _)| i)
                .collect();
            self.current_player = *lowest.choose(&mut thread_rng()).unwrap();
        }

        self.sort_hand(0);

        if !animate {
            return;
        }

        let shuffle_index = self.new_change();
        self.add_change(
            shuffle_index,
            Change {
                change_type: ChangeType::Shuffle,
                dest: Location::Deck,
                ..Default::default()
            },
        );

        let deal_index = self.new_change();
        for hand_index in 0..CARDS_PER_PLAYER {
            for player in 0..PLAYER_COUNT {
                let card = self.hands[player][hand_index];
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        offset: hand_index,
                        length: CARDS_PER_PLAYER,
                        ..Default::default()
                    },
                );
            }
        }

        self.update_hierarchy_display();
        self.round += 1;
        self.show_playable();
        self.show_message();
    }

    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::with_capacity(SUIT_COUNT * CARDS_PER_SUIT);
        let mut id = 0;

        for suit in Suit::all() {
            for _ in 0..CARDS_PER_SUIT {
                deck.push(Card { id, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());
        deck
    }

    /// Get the power of a suit (lower = more powerful)
    pub fn suit_power(&self, suit: Suit) -> usize {
        self.hierarchy.iter().position(|&s| s == suit).unwrap_or(6)
    }

    /// Returns the number of cards that will be played when a given
    /// card is selected. The card's position within its suit group
    /// determines the meld size: last card = max available (up to 3),
    /// second-to-last = one fewer, etc. When following, beating
    /// requires exactly meld_size so only the trigger card at
    /// suit_cards.len() - meld_size is offered. Adding is always 1.
    pub fn cards_to_play(&self, card_id: i32) -> usize {
        let hand = &self.hands[self.current_player];
        let card_suit = hand.iter().find(|c| c.id == card_id).unwrap().suit;

        if !self.current_meld.is_empty() {
            let meld_suit = self.current_meld_suit.unwrap();
            if card_suit == meld_suit {
                // Adding to meld: always 1
                return 1;
            }
            // Beating: always exactly meld_size
            return self.current_meld.len();
        }

        // Leading: position to end of suit group, capped at 3
        let suit_cards: Vec<&Card> = hand.iter().filter(|c| c.suit == card_suit).collect();
        let pos = suit_cards.iter().position(|c| c.id == card_id).unwrap();
        (suit_cards.len() - pos).min(3)
    }

    pub fn get_moves(&self) -> Vec<i32> {
        if self.state == State::GameOver {
            return vec![];
        }

        if self.shed_out_order.contains(&self.current_player) {
            return vec![];
        }

        let mut moves = Vec::new();
        let hand = &self.hands[self.current_player];

        // Lucky coin (before any play - leading or following)
        if self.has_lucky_coin[self.current_player] {
            moves.push(USE_LUCKY_COIN);
        }

        if self.current_meld.is_empty() {
            // Leading: for each suit, the last min(count, 3) cards are playable.
            // Tapping a card plays it and all cards after it in the suit group.
            for suit in Suit::all() {
                let suit_cards: Vec<&Card> = hand.iter().filter(|c| c.suit == suit).collect();
                let playable_count = suit_cards.len().min(3);
                // Offer the last `playable_count` cards in the suit
                for card in suit_cards.iter().skip(suit_cards.len() - playable_count) {
                    moves.push(card.id);
                }
            }
        } else {
            let meld_size = self.current_meld.len();
            let meld_suit = self.current_meld_suit.unwrap();
            let meld_power = self.suit_power(meld_suit);

            // Beat with higher power suit: need exactly meld_size cards,
            // so only the card at position len - meld_size is the trigger
            for suit in Suit::all() {
                if self.suit_power(suit) < meld_power {
                    let suit_cards: Vec<&Card> = hand.iter().filter(|c| c.suit == suit).collect();
                    if suit_cards.len() >= meld_size {
                        let trigger_idx = suit_cards.len() - meld_size;
                        moves.push(suit_cards[trigger_idx].id);
                    }
                }
            }

            // Add to meld if size < 3: any card of meld suit, plays 1
            if meld_size < 3 {
                for card in hand {
                    if card.suit == meld_suit {
                        moves.push(card.id);
                    }
                }
            }

            // Can always pass
            moves.push(PASS);
        }

        moves.sort();
        moves.dedup();
        moves
    }

    pub fn apply_move(&mut self, mov: i32) {
        self.changes = vec![vec![]];

        if !self.get_moves().contains(&mov) {
            panic!(
                "Invalid move: {} (valid: {:?}, state: {:?})",
                mov,
                self.get_moves(),
                self.state
            );
        }

        match self.state {
            State::GameOver => return,
            State::Play => self.apply_play_move(mov),
        }

        self.show_playable();
        self.show_message();
    }

    fn apply_play_move(&mut self, mov: i32) {
        if mov == USE_LUCKY_COIN {
            self.use_lucky_coin();
            return;
        }

        if mov == PASS {
            self.passed_this_round[self.current_player] = true;
            self.consecutive_passes += 1;

            let active: Vec<usize> = (0..PLAYER_COUNT)
                .filter(|&p| !self.shed_out_order.contains(&p))
                .collect();

            let passed_count = active
                .iter()
                .filter(|&&p| self.passed_this_round[p])
                .count();

            if passed_count >= active.len() - 1 && self.meld_leader.is_some() {
                self.meld_wins();
            } else {
                self.advance_player();
            }
            return;
        }

        // Determine how many cards to play based on card position in suit
        let count = self.cards_to_play(mov);
        let suit = self.hands[self.current_player]
            .iter()
            .find(|c| c.id == mov)
            .unwrap()
            .suit;

        // Take the last `count` cards of this suit from the hand
        let suit_card_ids: Vec<i32> = self.hands[self.current_player]
            .iter()
            .filter(|c| c.suit == suit)
            .collect::<Vec<&Card>>()
            .iter()
            .rev()
            .take(count)
            .rev()
            .map(|c| c.id)
            .collect();

        let mut meld_cards = Vec::new();
        for id in suit_card_ids {
            if let Some(pos) = self.hands[self.current_player]
                .iter()
                .position(|c| c.id == id)
            {
                meld_cards.push(self.hands[self.current_player].remove(pos));
            }
        }

        self.play_meld(meld_cards);
    }

    fn play_meld(&mut self, cards: Vec<Card>) {
        let suit = cards[0].suit;
        let is_adding =
            self.current_meld_suit == Some(suit) && cards.len() == 1 && self.current_meld.len() < 3;

        if is_adding {
            self.meld_leader = Some(self.current_player);
            self.current_meld.extend(cards.iter().cloned());
            let total = self.current_meld.len();
            let meld_ids: Vec<i32> = self.current_meld.iter().map(|c| c.id).collect();
            // Re-emit all meld cards so frontend re-spaces them
            for (i, id) in meld_ids.iter().enumerate() {
                self.add_change(
                    0,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: *id,
                        dest: Location::Play,
                        player: self.current_player,
                        offset: i,
                        length: total,
                        ..Default::default()
                    },
                );
            }
        } else {
            if !self.current_meld.is_empty() {
                let clear_index = self.new_change();
                let old_ids: Vec<i32> = self.current_meld.iter().map(|c| c.id).collect();
                for card_id in old_ids {
                    self.add_change(
                        clear_index,
                        Change {
                            change_type: ChangeType::ClearMeld,
                            object_id: card_id,
                            dest: Location::Discard,
                            ..Default::default()
                        },
                    );
                }
                self.current_meld.clear();
            }

            let play_index = self.new_change();
            self.current_meld = cards.clone();
            self.current_meld_suit = Some(suit);
            self.meld_leader = Some(self.current_player);

            for (i, card) in cards.iter().enumerate() {
                self.add_change(
                    play_index,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: card.id,
                        dest: Location::Play,
                        player: self.current_player,
                        offset: i,
                        length: cards.len(),
                        ..Default::default()
                    },
                );
            }
        }

        self.reorder_hand(self.current_player);
        self.passed_this_round = [false; PLAYER_COUNT];
        self.consecutive_passes = 0;

        if self.hands[self.current_player].is_empty() {
            self.player_sheds_out(self.current_player);
            if self.shed_out_order.len() >= 2 {
                return;
            }
        }

        self.advance_player();
    }

    fn meld_wins(&mut self) {
        let leader = self.meld_leader.unwrap();
        let winning_suit = self.current_meld_suit.unwrap();

        // Move winning suit to bottom of hierarchy
        self.hierarchy.retain(|&s| s != winning_suit);
        self.hierarchy.push(winning_suit);

        let clear_index = self.new_change();
        let meld_ids: Vec<i32> = self.current_meld.iter().map(|c| c.id).collect();
        for card_id in meld_ids {
            self.add_change(
                clear_index,
                Change {
                    change_type: ChangeType::ClearMeld,
                    object_id: card_id,
                    dest: Location::Discard,
                    ..Default::default()
                },
            );
        }
        self.current_meld.clear();
        self.current_meld_suit = None;
        self.meld_leader = None;

        self.update_hierarchy_display();

        // Resort and reposition all hands after hierarchy change
        for player in 0..PLAYER_COUNT {
            self.sort_hand(player);
        }
        self.reorder_hand(0);

        self.passed_this_round = [false; PLAYER_COUNT];
        self.consecutive_passes = 0;

        if self.shed_out_order.contains(&leader) {
            self.advance_from_player(leader);
        } else {
            self.current_player = leader;
        }
    }

    fn use_lucky_coin(&mut self) {
        self.has_lucky_coin[self.current_player] = false;
        self.hierarchy.reverse();

        self.add_change(
            0,
            Change {
                change_type: ChangeType::UseLuckyCoin,
                player: self.current_player,
                dest: Location::LuckyCoin,
                ..Default::default()
            },
        );

        self.update_hierarchy_display();

        // Resort and reposition all hands after hierarchy change
        for player in 0..PLAYER_COUNT {
            self.sort_hand(player);
        }
        self.reorder_hand(0);
    }

    fn update_hierarchy_display(&mut self) {
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::UpdateHierarchy,
                dest: Location::Hierarchy,
                suits: Some(self.hierarchy.clone()),
                ..Default::default()
            },
        );
    }

    fn player_sheds_out(&mut self, player: usize) {
        self.shed_out_order.push(player);

        let points = match self.shed_out_order.len() {
            1 => 3,
            2 => 2,
            _ => 0,
        };

        let old_score = self.scores[player];
        self.scores[player] += points;

        self.add_change(
            0,
            Change {
                change_type: ChangeType::PlayerOut,
                player,
                start_score: old_score,
                end_score: self.scores[player],
                ..Default::default()
            },
        );

        // Round ends when 2 players shed out (3 player game)
        if self.shed_out_order.len() >= 2 {
            self.end_round();
        }
    }

    fn end_round(&mut self) {
        if self.round >= ROUNDS as i32 {
            self.state = State::GameOver;
            let max_score = *self.scores.iter().max().unwrap();
            for player in 0..PLAYER_COUNT {
                if self.scores[player] == max_score {
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
        } else {
            self.deal(true);
        }
    }

    fn advance_player(&mut self) {
        self.advance_from_player(self.current_player);
    }

    fn advance_from_player(&mut self, from: usize) {
        let active: Vec<usize> = (0..PLAYER_COUNT)
            .filter(|&p| !self.shed_out_order.contains(&p))
            .collect();

        if active.len() <= 1 {
            self.end_round();
            return;
        }

        let mut next = (from + 1) % PLAYER_COUNT;
        while self.shed_out_order.contains(&next) {
            next = (next + 1) % PLAYER_COUNT;
        }
        self.current_player = next;
    }

    fn sort_hand(&mut self, player: usize) {
        let hierarchy = self.hierarchy.clone();
        self.hands[player].sort_by(|a, b| {
            let a_power = hierarchy.iter().position(|&s| s == a.suit).unwrap_or(6);
            let b_power = hierarchy.iter().position(|&s| s == b.suit).unwrap_or(6);
            match a_power.cmp(&b_power) {
                std::cmp::Ordering::Equal => a.id.cmp(&b.id),
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

    fn reorder_hand(&mut self, player: usize) {
        if self.no_changes {
            return;
        }
        if self.changes.is_empty() {
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
        let change_index = self.new_change();

        if self.current_player == 0 && self.state != State::GameOver {
            let moves = self.get_moves();
            for id in &moves {
                if *id >= 0 && *id < 1000 {
                    self.add_change(
                        change_index,
                        Change {
                            object_id: *id,
                            change_type: ChangeType::ShowPlayable,
                            dest: Location::Hand,
                            player: 0,
                            ..Default::default()
                        },
                    );
                }
            }
        } else {
            self.hide_playable();
        }
    }

    fn show_message(&mut self) {
        let index = self.new_change();
        self.add_change(
            index,
            Change {
                change_type: ChangeType::Message,
                message: None,
                object_id: -1,
                dest: Location::Message,
                ..Default::default()
            },
        );
    }

    fn hide_playable(&mut self) {
        let change_index = self.new_change();
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
    }
}

impl ismcts::Game for SMMGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        // Shuffle cards between non-observer players
        for p1 in 0..PLAYER_COUNT {
            for p2 in (p1 + 1)..PLAYER_COUNT {
                if p1 == observer || p2 == observer {
                    continue;
                }
                if self.shed_out_order.contains(&p1) || self.shed_out_order.contains(&p2) {
                    continue;
                }

                let mut combined: Vec<Card> = self.hands[p1]
                    .iter()
                    .chain(self.hands[p2].iter())
                    .cloned()
                    .collect();
                combined.shuffle(rng);

                let p1_count = self.hands[p1].len();
                self.hands[p1] = combined.drain(..p1_count).collect();
                self.hands[p2] = combined;
            }
        }
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        let mut next = (self.current_player + 1) % PLAYER_COUNT;
        while self.shed_out_order.contains(&next) {
            next = (next + 1) % PLAYER_COUNT;
            if next == self.current_player {
                break;
            }
        }
        next
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.apply_move(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.state != State::GameOver {
            None
        } else {
            let player_score = self.scores[player];
            let max_score = *self.scores.iter().max().unwrap();

            if player_score == max_score {
                if self.scores.iter().filter(|&&s| s == player_score).count() > 1 {
                    Some(0.0)
                } else {
                    Some(1.0)
                }
            } else {
                Some(-1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_game() {
        let game = SMMGame::new();
        assert_eq!(game.hands.len(), PLAYER_COUNT);
        assert!(game.hands.iter().all(|h| h.len() == CARDS_PER_PLAYER));
        assert_eq!(game.state, State::Play);
        assert_eq!(game.round, 1);
        assert_eq!(game.hierarchy.len(), SUIT_COUNT);
    }

    #[test]
    fn test_deck_composition() {
        let deck = SMMGame::deck();
        assert_eq!(deck.len(), SUIT_COUNT * CARDS_PER_SUIT);
        assert_eq!(deck.len(), 56);

        for suit in Suit::all() {
            let count = deck.iter().filter(|c| c.suit == suit).count();
            assert_eq!(
                count, CARDS_PER_SUIT,
                "Suit {:?} should have {} cards",
                suit, CARDS_PER_SUIT
            );
        }
    }

    #[test]
    fn test_hierarchy_power() {
        let game = SMMGame::new();
        // Default hierarchy order
        assert_eq!(game.suit_power(Suit::Cherry), 0);
        assert_eq!(game.suit_power(Suit::Diamond), 1);
        assert_eq!(game.suit_power(Suit::Bell), 2);
        assert_eq!(game.suit_power(Suit::Clover), 3);
        assert_eq!(game.suit_power(Suit::Horseshoe), 4);
        assert_eq!(game.suit_power(Suit::Bar), 5);
        assert_eq!(game.suit_power(Suit::Seven), 6);
    }

    #[test]
    fn test_hierarchy_change_after_meld_wins() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        // Setup: Cherry meld wins
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(0);

        // Simulate meld winning
        game.meld_wins();

        // Cherry should now be at the bottom (least powerful)
        assert_eq!(game.suit_power(Suit::Cherry), 6);
        assert_eq!(game.hierarchy[6], Suit::Cherry);
    }

    #[test]
    fn test_lucky_coin_reverses_hierarchy() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        let original_first = game.hierarchy[0];
        let original_last = game.hierarchy[6];

        game.use_lucky_coin();

        assert_eq!(game.hierarchy[0], original_last);
        assert_eq!(game.hierarchy[6], original_first);
        assert!(!game.has_lucky_coin[0]);
    }

    #[test]
    fn test_pass_available_when_meld_exists() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        // Setup meld
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(1);
        game.current_player = 0;

        let moves = game.get_moves();
        assert!(moves.contains(&PASS));
    }

    #[test]
    fn test_pass_not_available_when_leading() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;
        game.current_meld.clear();
        game.current_meld_suit = None;

        let moves = game.get_moves();
        assert!(!moves.contains(&PASS));
    }

    #[test]
    fn test_lucky_coin_available_when_leading() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;
        game.current_meld.clear();
        game.has_lucky_coin[0] = true;

        let moves = game.get_moves();
        assert!(moves.contains(&USE_LUCKY_COIN));
    }

    #[test]
    fn test_lucky_coin_not_available_after_use() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;
        game.current_meld.clear();
        game.has_lucky_coin[0] = false;

        let moves = game.get_moves();
        assert!(!moves.contains(&USE_LUCKY_COIN));
    }

    #[test]
    fn test_lucky_coin_available_when_following() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(1);
        game.has_lucky_coin[0] = true;
        // Give player some cards so they have moves
        game.hands[0] = vec![Card {
            id: 48,
            suit: Suit::Seven,
        }];

        let moves = game.get_moves();
        // Lucky coin should be available before ANY play, not just when leading
        assert!(moves.contains(&USE_LUCKY_COIN));
    }

    #[test]
    fn test_lucky_coin_while_following_changes_hierarchy() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;
        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // There's a meld on the table
        game.current_meld = vec![Card {
            id: 8,
            suit: Suit::Diamond,
        }];
        game.current_meld_suit = Some(Suit::Diamond);
        game.meld_leader = Some(1);
        game.has_lucky_coin[0] = true;
        game.hands[0] = vec![
            Card {
                id: 48,
                suit: Suit::Seven,
            },
            Card {
                id: 49,
                suit: Suit::Seven,
            },
        ];

        // Seven is power 6 (weakest), can't beat Diamond (power 1)
        let moves_before = game.get_moves();
        assert!(!moves_before.contains(&48)); // Can't play Seven to beat Diamond

        // Use lucky coin to reverse hierarchy
        game.apply_move(USE_LUCKY_COIN);

        // Now Seven is power 0 (strongest), Diamond is power 5
        assert_eq!(game.hierarchy[0], Suit::Seven);
        assert!(!game.has_lucky_coin[0]);

        // Still same player's turn, still same meld
        assert_eq!(game.current_player, 0);
        assert_eq!(game.current_meld.len(), 1);

        // Now Seven cards can beat Diamond
        let moves_after = game.get_moves();
        assert!(moves_after.contains(&48) || moves_after.contains(&49));
    }

    #[test]
    fn test_lucky_coin_apply_move_on_new_game() {
        let mut game = SMMGame::new();
        game.current_player = 0;
        assert!(game.get_moves().contains(&USE_LUCKY_COIN));
        game.apply_move(USE_LUCKY_COIN);
        assert!(!game.has_lucky_coin[0]);
    }

    #[test]
    fn test_can_beat_meld_with_higher_power() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        // Set hierarchy: Cherry most powerful, Seven least
        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is Diamond (power 1)
        game.current_meld = vec![Card {
            id: 10,
            suit: Suit::Diamond,
        }];
        game.current_meld_suit = Some(Suit::Diamond);
        game.meld_leader = Some(1);

        // Player 0 has Cherry cards (power 0, can beat)
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
        ];
        game.current_player = 0;

        let moves = game.get_moves();
        // Only the last Cherry (id:1) is the trigger to beat a 1-card meld
        assert!(moves.contains(&1));
    }

    #[test]
    fn test_cannot_beat_meld_with_lower_power() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is Cherry (power 0, most powerful)
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(1);

        // Player 0 only has Seven cards (power 6, cannot beat)
        game.hands[0] = vec![
            Card {
                id: 48,
                suit: Suit::Seven,
            },
            Card {
                id: 49,
                suit: Suit::Seven,
            },
        ];
        game.current_player = 0;
        game.has_lucky_coin[0] = false;

        let moves = game.get_moves();
        // Should only be able to pass (can't beat or add, no lucky coin)
        assert_eq!(moves, vec![PASS]);
    }

    #[test]
    fn test_can_add_to_meld() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        // Current meld is 1 Cherry
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(1);

        // Player 0 has a Cherry
        game.hands[0] = vec![Card {
            id: 1,
            suit: Suit::Cherry,
        }];
        game.current_player = 0;

        let moves = game.get_moves();
        assert!(moves.contains(&1)); // Can add Cherry
    }

    #[test]
    fn test_adding_to_meld_changes_leader() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        // Player 1 leads a Cherry meld
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(1);

        // Player 0 has a Cherry and adds to the meld
        game.hands[0] = vec![
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 8,
                suit: Suit::Diamond,
            },
        ];
        game.current_player = 0;
        game.has_lucky_coin[0] = false;

        game.apply_move(1); // Add cherry to meld

        // Player 0 should now be the meld leader
        assert_eq!(game.meld_leader, Some(0));
    }

    #[test]
    fn test_cannot_add_to_full_meld() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        // Current meld is 3 Cherries (full)
        game.current_meld = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 2,
                suit: Suit::Cherry,
            },
        ];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(1);

        // Player 0 has a Cherry but can't add (meld full)
        game.hands[0] = vec![Card {
            id: 3,
            suit: Suit::Cherry,
        }];
        game.current_player = 0;
        game.has_lucky_coin[0] = false;

        let moves = game.get_moves();
        // Can only pass (can't add to full meld, can't beat Cherry with Cherry, no lucky coin)
        assert_eq!(moves, vec![PASS]);
    }

    #[test]
    fn test_scoring_first_out() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.scores = [0, 0, 0];

        game.player_sheds_out(0);

        assert_eq!(game.scores[0], 3);
        assert!(game.shed_out_order.contains(&0));
    }

    #[test]
    fn test_scoring_second_out() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.scores = [0, 0, 0];
        game.shed_out_order = vec![1]; // Player 1 already out

        game.player_sheds_out(0);

        assert_eq!(game.scores[0], 2);
    }

    #[test]
    fn test_round_ends_after_two_shed_out() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.round = 1;
        game.shed_out_order = vec![1];

        game.player_sheds_out(0);

        // After 2 players shed out in 3-player game, new round starts
        // (round increments in deal)
        assert_eq!(game.round, 2);
    }

    #[test]
    fn test_game_over_after_three_rounds() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.round = 3;
        game.scores = [5, 3, 2];
        game.shed_out_order = vec![1];

        game.player_sheds_out(0);

        assert_eq!(game.state, State::GameOver);
        assert_eq!(game.winner, Some(0)); // Player 0 has highest score
    }

    #[test]
    fn test_cards_to_play_leading() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        // Hand has 3 Cherries (ids 0,1,2) and 1 Diamond (id 8)
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 2,
                suit: Suit::Cherry,
            },
            Card {
                id: 8,
                suit: Suit::Diamond,
            },
        ];

        // Last Cherry (id:2) = play 1, second-to-last (id:1) = play 2, third-to-last (id:0) = play 3
        assert_eq!(game.cards_to_play(2), 1);
        assert_eq!(game.cards_to_play(1), 2);
        assert_eq!(game.cards_to_play(0), 3);

        // Only Diamond = play 1
        assert_eq!(game.cards_to_play(8), 1);

        // All 4 cards should be valid moves when leading
        let moves = game.get_moves();
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
        assert!(moves.contains(&8));
    }

    #[test]
    fn test_cards_to_play_leading_four_of_suit() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        // Hand has 4 Cherries - only last 3 are playable (max meld is 3)
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 2,
                suit: Suit::Cherry,
            },
            Card {
                id: 3,
                suit: Suit::Cherry,
            },
        ];

        let moves = game.get_moves();
        // Card 0 is not offered (4th from end, beyond max 3)
        assert!(!moves.contains(&0));
        // Cards 1,2,3 are playable
        assert!(moves.contains(&1)); // play 3
        assert!(moves.contains(&2)); // play 2
        assert!(moves.contains(&3)); // play 1
    }

    #[test]
    fn test_cards_to_play_beating() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is 2 Diamonds
        game.current_meld = vec![
            Card {
                id: 8,
                suit: Suit::Diamond,
            },
            Card {
                id: 9,
                suit: Suit::Diamond,
            },
        ];
        game.current_meld_suit = Some(Suit::Diamond);
        game.meld_leader = Some(1);

        // Player has 3 Cherries - can beat 2-card meld
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 2,
                suit: Suit::Cherry,
            },
        ];

        let moves = game.get_moves();
        // Trigger card for beating 2-card meld is at index len-2 = 1, which is card id:1
        assert!(moves.contains(&1));
        // Card 0 and 2 are not triggers for beating
        assert!(!moves.contains(&0));
        assert!(!moves.contains(&2));
    }

    #[test]
    fn test_meld_must_match_size_to_beat() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is 2 Diamonds
        game.current_meld = vec![
            Card {
                id: 8,
                suit: Suit::Diamond,
            },
            Card {
                id: 9,
                suit: Suit::Diamond,
            },
        ];
        game.current_meld_suit = Some(Suit::Diamond);
        game.meld_leader = Some(1);

        // Player has only 1 Cherry - can't beat 2-card meld
        game.hands[0] = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_player = 0;
        game.has_lucky_coin[0] = false;

        let moves = game.get_moves();
        // Can only pass - not enough Cherries to beat
        assert_eq!(moves, vec![PASS]);
    }

    #[test]
    fn test_adding_same_suit_always_plays_one() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is 1 Horseshoe
        game.current_meld = vec![Card {
            id: 32,
            suit: Suit::Horseshoe,
        }];
        game.current_meld_suit = Some(Suit::Horseshoe);
        game.meld_leader = Some(1);

        // Player has 3 Horseshoes
        game.hands[0] = vec![
            Card {
                id: 33,
                suit: Suit::Horseshoe,
            },
            Card {
                id: 34,
                suit: Suit::Horseshoe,
            },
            Card {
                id: 35,
                suit: Suit::Horseshoe,
            },
        ];

        // Each horseshoe should play exactly 1 (adding, not beating)
        assert_eq!(game.cards_to_play(33), 1);
        assert_eq!(game.cards_to_play(34), 1);
        assert_eq!(game.cards_to_play(35), 1);

        // All 3 should be available moves (each adds 1)
        let moves = game.get_moves();
        assert!(moves.contains(&33));
        assert!(moves.contains(&34));
        assert!(moves.contains(&35));
    }

    #[test]
    fn test_adding_to_two_card_meld_plays_one() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is 2 Horseshoes
        game.current_meld = vec![
            Card {
                id: 32,
                suit: Suit::Horseshoe,
            },
            Card {
                id: 33,
                suit: Suit::Horseshoe,
            },
        ];
        game.current_meld_suit = Some(Suit::Horseshoe);
        game.meld_leader = Some(1);

        // Player has 2 Horseshoes
        game.hands[0] = vec![
            Card {
                id: 34,
                suit: Suit::Horseshoe,
            },
            Card {
                id: 35,
                suit: Suit::Horseshoe,
            },
        ];

        // Adding to a 2-card meld: each plays 1
        assert_eq!(game.cards_to_play(34), 1);
        assert_eq!(game.cards_to_play(35), 1);

        let moves = game.get_moves();
        assert!(moves.contains(&34));
        assert!(moves.contains(&35));
    }

    #[test]
    fn test_beating_with_higher_suit_plays_meld_size() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is 2 Bars (power 5)
        game.current_meld = vec![
            Card {
                id: 40,
                suit: Suit::Bar,
            },
            Card {
                id: 41,
                suit: Suit::Bar,
            },
        ];
        game.current_meld_suit = Some(Suit::Bar);
        game.meld_leader = Some(1);

        // Player has 3 Cherries (power 0, beats Bar)
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 2,
                suit: Suit::Cherry,
            },
        ];

        // Beating a 2-card meld: trigger card plays exactly 2
        // Trigger is at index len-meld_size = 3-2 = 1, card id:1
        assert_eq!(game.cards_to_play(1), 2);

        let moves = game.get_moves();
        assert!(moves.contains(&1));
        // Cards 0 and 2 are not valid beat triggers
        assert!(!moves.contains(&0));
        assert!(!moves.contains(&2));
    }

    #[test]
    fn test_can_add_or_beat_different_suits() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;

        game.hierarchy = vec![
            Suit::Cherry,
            Suit::Diamond,
            Suit::Bell,
            Suit::Clover,
            Suit::Horseshoe,
            Suit::Bar,
            Suit::Seven,
        ];

        // Current meld is 1 Horseshoe (power 4)
        game.current_meld = vec![Card {
            id: 32,
            suit: Suit::Horseshoe,
        }];
        game.current_meld_suit = Some(Suit::Horseshoe);
        game.meld_leader = Some(1);

        // Player has 3 Horseshoes (for adding) and 2 Cherries (for beating)
        game.hands[0] = vec![
            Card {
                id: 0,
                suit: Suit::Cherry,
            },
            Card {
                id: 1,
                suit: Suit::Cherry,
            },
            Card {
                id: 33,
                suit: Suit::Horseshoe,
            },
            Card {
                id: 34,
                suit: Suit::Horseshoe,
            },
            Card {
                id: 35,
                suit: Suit::Horseshoe,
            },
        ];

        // Horseshoes: adding, each plays 1
        assert_eq!(game.cards_to_play(33), 1);
        assert_eq!(game.cards_to_play(34), 1);
        assert_eq!(game.cards_to_play(35), 1);

        // Cherry: beating 1-card meld, plays 1
        // Trigger is last Cherry (id:1)
        assert_eq!(game.cards_to_play(1), 1);

        let moves = game.get_moves();
        // All horseshoes available for adding
        assert!(moves.contains(&33));
        assert!(moves.contains(&34));
        assert!(moves.contains(&35));
        // Last Cherry available for beating
        assert!(moves.contains(&1));
        // First Cherry not a trigger for beating a 1-card meld
        assert!(!moves.contains(&0));
    }

    #[test]
    fn test_all_suits_exist() {
        assert_eq!(Suit::all().len(), 7);
        let suits = Suit::all();
        assert!(suits.contains(&Suit::Cherry));
        assert!(suits.contains(&Suit::Diamond));
        assert!(suits.contains(&Suit::Bell));
        assert!(suits.contains(&Suit::Clover));
        assert!(suits.contains(&Suit::Horseshoe));
        assert!(suits.contains(&Suit::Bar));
        assert!(suits.contains(&Suit::Seven));
    }

    #[test]
    fn test_player_count_is_three() {
        assert_eq!(PLAYER_COUNT, 3);
    }

    #[test]
    fn test_cards_per_player_is_fifteen() {
        assert_eq!(CARDS_PER_PLAYER, 15);
    }

    #[test]
    fn test_total_cards_dealt() {
        // 3 players * 15 cards = 45 cards dealt
        // 56 total cards - 45 = 11 cards remaining (not used)
        assert_eq!(PLAYER_COUNT * CARDS_PER_PLAYER, 45);
        assert_eq!(
            SUIT_COUNT * CARDS_PER_SUIT - PLAYER_COUNT * CARDS_PER_PLAYER,
            11
        );
    }

    #[test]
    fn test_advance_player_skips_shed_out() {
        let mut game = SMMGame::new();
        game.no_changes = true;
        game.current_player = 0;
        game.shed_out_order = vec![1]; // Player 1 is out

        game.advance_player();

        assert_eq!(game.current_player, 2); // Should skip to player 2
    }

    #[test]
    fn test_soft_pass() {
        let mut game = SMMGame::new();
        game.no_changes = true;

        // Setup: meld exists, player 0's turn
        game.current_meld = vec![Card {
            id: 0,
            suit: Suit::Cherry,
        }];
        game.current_meld_suit = Some(Suit::Cherry);
        game.meld_leader = Some(2);
        game.current_player = 0;
        game.hands[0] = vec![Card {
            id: 48,
            suit: Suit::Seven,
        }]; // Can't beat
        game.has_lucky_coin[0] = false;

        // Player passes
        game.apply_move(PASS);

        assert!(game.passed_this_round[0]);
        assert_eq!(game.current_player, 1); // Moved to next player

        // Verify player can still play later (soft pass)
        // Reset to player 0's turn
        game.current_player = 0;
        game.passed_this_round = [false; PLAYER_COUNT];

        let moves = game.get_moves();
        assert!(moves.contains(&PASS)); // Can still participate
    }

    #[test]
    fn test_full_game_simulation() {
        // Run multiple full games to verify no panics or infinite loops
        for _ in 0..10 {
            let mut game = SMMGame::new();
            game.no_changes = true;
            let mut moves_count = 0;

            while game.state != State::GameOver {
                let moves = game.get_moves();
                if moves.is_empty() {
                    break;
                }

                // Pick a random valid move
                let mov = moves[moves_count % moves.len()];
                game.apply_move(mov);
                moves_count += 1;

                // Safety: prevent infinite loops
                assert!(
                    moves_count < 10000,
                    "Game seems stuck after {} moves",
                    moves_count
                );
            }

            assert_eq!(game.state, State::GameOver);
            assert_eq!(game.round, ROUNDS as i32);
            // Verify someone has points
            let total: i32 = game.scores.iter().sum();
            assert!(total > 0, "Someone should have scored");
        }
    }
}
