/*
Game: Short Zoot Suit
Designer: Taylor Reiner
BoardGameGeek: https://boardgamegeek.com/boardgame/366458/short-zoot-suit
*/

use crate::utils::shuffle_and_divide_matching_cards;
use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::cmp::{min, Ordering};
use std::collections::{HashMap, HashSet};

const DRAW: i32 = 0;
const PASS: i32 = 1;
const DISCARD_OFFSET: i32 = 2; // 2-50 discards
const PLAY_OFFSET: i32 = 51; // 51-99 plays

#[derive(Debug, Clone, Copy, Default, PartialEq, Sequence, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
enum State {
    #[default]
    Play,
    Discard,
    OptionalDraw,
}

#[derive(
    Debug,
    PartialOrd,
    Ord,
    Clone,
    Copy,
    Sequence,
    Default,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
    #[default]
    Red,
    Blue,
    Yellow,
    Green,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i32,
    value: i32,
    pub suit: Suit,
}

fn move_offset(state: State, card: &Card) -> i32 {
    match state {
        State::OptionalDraw => 0,
        State::Discard => card.id + DISCARD_OFFSET,
        State::Play => card.id + PLAY_OFFSET,
    }
}

fn card_offset(state: State, offset: i32) -> i32 {
    match state {
        State::OptionalDraw => panic!("impossible move"),
        State::Discard => offset - DISCARD_OFFSET,
        State::Play => offset - PLAY_OFFSET,
    }
}

pub fn deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    let mut id = 0;
    for suit in all::<Suit>() {
        for value in 0..12 {
            deck.push(Card {
                id,
                value: value + 1,
                suit,
            });
            id += 1;
        }
    }
    deck.shuffle(&mut thread_rng());
    deck
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Discard,
    Play,
    TricksToWinner,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    TrickToShortsPile,
    Reorder,
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    TricksTaken,
    DrawDeck,
    Score,
    ShortsPile,
    ReorderHand,
    StageDrawDeck,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    player: i32,
    object_id: i32,
    source_offset: i32,
    dest: Location,
    dest_offset: i32,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    hand_offset: i32,
    length: i32,
    cards_remaining: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    undo_players: HashSet<i32>,
    action_size: i32,
    hands: [Vec<Card>; 3],
    pub draw_decks: [Vec<Card>; 3],
    shorts_piles: [Vec<Card>; 3],
    pub changes: Vec<Vec<Change>>,
    tricks_taken: [i32; 3],
    current_trick: [Option<Card>; 3],
    lead_suit: Option<Suit>,
    pub round: i32,
    pub scores: Vec<i32>,
    pub voids: Vec<HashSet<Suit>>,
    pub current_player: i32,
    pub winner: Option<i32>,
    pub dealer: i32,
    state: State,
    draw_players_remaining: Vec<i32>,
    lead_player: i32,
    #[serde(default)]
    pub no_changes: bool,
    pub experiment: bool,
}

impl Game {
    /// Factory to create a default game
    pub fn new() -> Game {
        let mut game = Game::default();
        game.deal();
        game.scores = vec![0, 0, 0];
        game
    }

    /// Set which players can undo their moves when discarding
    /// (The human player (0) is set as an undo player on
    /// Trickster's Table)
    pub fn with_undo_players(self: &mut Game, undo_players: HashSet<i32>) {
        self.undo_players = undo_players;
    }

    // Skip adding changes which are used to manipulate the UI
    // This is used to increase the speed of simulations
    pub fn with_no_changes(self: &mut Game) {
        self.no_changes = true;
    }

    fn deal(self: &mut Game) {
        self.state = State::Discard;
        self.current_trick = [None, None, None];
        self.draw_players_remaining = (0..3).collect();
        self.tricks_taken = [0, 0, 0];
        self.hands = [vec![], vec![], vec![]];
        self.draw_decks = [vec![], vec![], vec![]];
        self.shorts_piles = [vec![], vec![], vec![]];
        self.dealer = (self.dealer + 1) % 3;
        self.current_player = self.dealer;
        self.voids = vec![HashSet::new(), HashSet::new(), HashSet::new()];
        let mut cards = deck();
        let deal_index: usize = self.changes.len();
        let reorder_index = deal_index + 1;
        self.changes.push(vec![]); // deal_index
        self.changes.push(vec![]); // reorder_index
        self.hands = [vec![], vec![], vec![]];
        for y in 0..16 {
            for player in 0..3 {
                let card = cards.pop().expect("cards should be available here");
                self.changes[deal_index].push(Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id,
                    dest: Location::Hand,
                    dest_offset: player,
                    player,
                    hand_offset: y,
                    length: 16,
                    ..Default::default()
                });
                self.hands[player as usize].push(card);
            }
        }
        self.hands[0].sort_by(card_sorter);
        self.changes[reorder_index].append(&mut reorder_hand(0, &self.hands[0]));
        let playable_changes = self.show_playable();
        self.changes.push(playable_changes);
    }

    pub fn apply_move(self: &mut Game, action: i32) {
        self.changes = vec![vec![]]; // card from player to table or discard to draw deck
        if !self.get_moves().contains(&action) {
            panic!("illegal move");
        }
        if self.state == State::OptionalDraw {
            if action == DRAW {
                // Once a player draws a card we don't know what their voids are
                self.voids[self.current_player as usize] = HashSet::new();
                let new_card: Card = self.draw_decks[self.current_player as usize].remove(0);
                self.hands[self.current_player as usize].push(new_card);
                self.hands[self.current_player as usize].sort_by(card_sorter);
                self.changes[0].append(
                    reorder_hand(
                        self.current_player,
                        &self.hands[self.current_player as usize].to_vec(),
                    )
                    .as_mut(),
                );
                for card in &self.draw_decks[self.current_player as usize] {
                    self.changes[0].push(Change {
                        change_type: ChangeType::Discard,
                        object_id: card.id,
                        source_offset: self.current_player,
                        dest: Location::DrawDeck,
                        player: self.current_player,
                        cards_remaining: self.draw_decks[self.current_player as usize].len() as i32,
                        ..Default::default()
                    });
                }
            }
            let mut new_players_remaining = self.draw_players_remaining.clone();
            new_players_remaining.retain(|&x| x != self.current_player);
            self.draw_players_remaining = new_players_remaining;
            if self.draw_players_remaining.is_empty() {
                if self.check_hand_end() {
                    return;
                }
                self.current_player = self.lead_player;
                self.state = State::Play;
            } else {
                self.current_player = *self
                    .draw_players_remaining
                    .first()
                    .expect("draw_players_remaining cannot be empty here");
            }
            let change_offset = &self.changes.len() - 1;
            let mut new_changes = self.show_playable();
            self.changes[change_offset].append(&mut new_changes);
            return;
        }
        if self.state == State::Discard {
            let mut all_cards = self.hands[self.current_player as usize].clone();
            all_cards.append(&mut self.draw_decks[self.current_player as usize].clone());
            let card_id = action - DISCARD_OFFSET;
            let card = all_cards
                .iter()
                .find(|c| c.id == card_id)
                .expect("player played a card that should exist");
            if self.draw_decks[self.current_player as usize].contains(card) {
                // Allows undo
                self.draw_decks[self.current_player as usize].retain(|c| c != card);
                self.hands[self.current_player as usize].push(*card);
            } else {
                self.hands[self.current_player as usize].retain(|c| c != card);
                self.draw_decks[self.current_player as usize].push(*card);
            }
            let mut offset: i32 = 0;
            if self.current_player == 0 {
                for card in &self.draw_decks[self.current_player as usize] {
                    self.changes[0].push(Change {
                        change_type: ChangeType::Discard,
                        object_id: card.id,
                        source_offset: self.current_player,
                        dest: Location::StageDrawDeck,
                        dest_offset: offset,
                        player: self.current_player,
                        cards_remaining: self.draw_decks[self.current_player as usize].len() as i32,
                        ..Default::default()
                    });
                    offset += 1;
                }
            } else {
                self.changes[0].push(Change {
                    change_type: ChangeType::Discard,
                    object_id: card.id,
                    source_offset: self.current_player,
                    dest: Location::DrawDeck,
                    dest_offset: offset,
                    player: self.current_player,
                    cards_remaining: self.draw_decks[self.current_player as usize].len() as i32,
                    ..Default::default()
                });
            }
            self.hands[self.current_player as usize].sort_by(card_sorter);
            self.changes[0].append(
                reorder_hand(
                    self.current_player,
                    &self.hands[self.current_player as usize],
                )
                .as_mut(),
            );
            if self.draw_decks[self.current_player as usize].len() == 5 {
                if !self.no_changes {
                    if self.current_player == 0 {
                        let mut cards_remaining_changes: Vec<Change> = vec![];
                        for card in &self.draw_decks[0] {
                            cards_remaining_changes.push(Change {
                                object_id: card.id,
                                change_type: ChangeType::Discard,
                                dest: Location::DrawDeck,
                                cards_remaining: 5,
                                ..Default::default()
                            });
                        }
                        self.changes.push(cards_remaining_changes);
                    }
                }
                self.current_player = (self.current_player + 1) % 3;
            }
            if self.draw_decks[self.current_player as usize].len() == 5 {
                for player in 0..3 {
                    self.draw_decks[player].shuffle(&mut thread_rng());
                }
                self.state = State::OptionalDraw;
            }
            let change_offset = &self.changes.len() - 1;
            let mut new_changes = self.show_playable();
            self.changes[change_offset].append(&mut new_changes);
            return;
        }
        let card_id = action - PLAY_OFFSET;
        let card = &self.hands[self.current_player as usize]
            .iter()
            .find(|c| c.id == card_id)
            .expect("this card has to be in the player's hand")
            .clone();
        self.hands[self.current_player as usize].retain(|c| c.id != card_id);
        if !self.no_changes {
            self.changes[0].push(Change {
                change_type: ChangeType::Play,
                object_id: card_id,
                source_offset: self.current_player,
                dest: Location::Play,
                dest_offset: self.current_player,
                player: self.current_player,
                ..Default::default()
            });
            self.changes[0].append(
                reorder_hand(
                    self.current_player,
                    &self.hands[self.current_player as usize],
                )
                .as_mut(),
            );
        }
        let last_change = self.changes.len() - 1;
        let mut changes = self.hide_playable();
        self.changes[last_change].append(&mut changes);
        self.current_trick[self.current_player as usize] = Some(*card);
        if self.lead_suit.is_none() {
            self.lead_suit = Some(card.suit);
        } else {
            if Some(card.suit) != self.lead_suit {
                // Player has revealed a void
                self.voids[self.current_player as usize].insert(card.suit);
            }
        }
        self.current_player = (self.current_player + 1) % 3;
        // end trick
        if self.current_trick.iter().flatten().count() == 3 {
            let trick_winner = get_winner(self.lead_suit, self.current_trick);
            let winning_card = self.current_trick[trick_winner as usize]
                .expect("there has to be a trick_winner card");
            self.tricks_taken[trick_winner as usize] += 1;
            // winner of the trick leads
            self.current_player = trick_winner;
            self.lead_player = trick_winner;
            if !self.no_changes {
                self.changes.push(vec![
                    Change {
                        change_type: ChangeType::ShowWinningCard,
                        object_id: winning_card.id,
                        dest: Location::Play,
                        ..Default::default()
                    },
                    Change {
                        change_type: ChangeType::OptionalPause,
                        object_id: 0,
                        dest: Location::Play,
                        ..Default::default()
                    },
                    Change {
                        object_id: winning_card.id,
                        change_type: ChangeType::HidePlayable,
                        dest: Location::Hand,
                        dest_offset: self.current_player,
                        ..Default::default()
                    },
                ]);
            }
            self.changes.push(vec![]); // trick back to player
            let offset: usize = self.changes.len() - 1;
            for player in 0..3 {
                let card =
                    self.current_trick[player].expect("each player should have played a card");
                if Some(card.suit) == self.lead_suit {
                    self.changes[offset].push(Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.id,
                        source_offset: player as i32,
                        dest: Location::TricksTaken,
                        player: trick_winner,
                        tricks_taken: self.tricks_taken[trick_winner as usize],
                        ..Default::default()
                    });
                } else {
                    self.shorts_piles[player].push(card);
                    self.changes[offset].push(Change {
                        change_type: ChangeType::TrickToShortsPile,
                        object_id: card.id,
                        source_offset: player as i32,
                        dest: Location::ShortsPile,
                        player: player as i32,
                        dest_offset: trick_winner,
                        tricks_taken: self.shorts_piles[player].len() as i32,
                        ..Default::default()
                    });
                }
            }
            self.draw_players_remaining = vec![];
            for player_offset in 0..3 {
                let player = (player_offset + self.lead_player) % 3;
                if !self.draw_decks[player as usize].is_empty() {
                    self.draw_players_remaining.push(player);
                }
            }
            if !self.draw_players_remaining.is_empty() {
                self.current_player = *self
                    .draw_players_remaining
                    .first()
                    .expect("draw_players_remaining unexpectedly empty");
                self.state = State::OptionalDraw;
            } else {
                if self.check_hand_end() {
                    return;
                }

                self.current_player = self.lead_player;
                self.state = State::Play;
            }
            self.current_trick = [None, None, None];
            self.lead_suit = None;
        }
        let change_offset = &self.changes.len() - 1;
        let mut new_changes = self.show_playable();
        self.changes[change_offset].append(&mut new_changes);
    }

    pub fn get_moves(self: &Game) -> Vec<i32> {
        if self.state == State::OptionalDraw {
            if !self.draw_decks[self.current_player as usize].is_empty() {
                return vec![DRAW, PASS];
            }
            return vec![PASS];
        }
        let allow_undoes = self.undo_players.contains(&self.current_player);
        let mut actions: Vec<i32>;
        if self.state == State::Discard {
            actions = self.hands[self.current_player as usize]
                .iter()
                .map(|c| move_offset(self.state, c))
                .collect();
            if allow_undoes {
                actions.append(
                    &mut self.draw_decks[self.current_player as usize]
                        .iter()
                        .map(|c| move_offset(self.state, c))
                        .collect(),
                );
            }
            return actions;
        }
        if self.lead_suit.is_some() {
            actions = self.hands[self.current_player as usize]
                .iter()
                .filter(|c| Some(c.suit) == self.lead_suit)
                .map(|c| move_offset(self.state, c))
                .collect();
            if !actions.is_empty() {
                return actions;
            }
        }
        return self.hands[self.current_player as usize]
            .iter()
            .map(|c| move_offset(self.state, c))
            .collect();
    }

    fn check_hand_end(self: &mut Game) -> bool {
        if !self.hands.iter().any(|x| x.is_empty()) {
            return false;
        }

        let original_scores: Vec<i32> = self.scores.clone();
        let last_change = self.changes.len() - 1;
        let mut changes = self.hide_playable();
        self.changes[last_change].append(&mut changes);
        self.scores = score_game(
            self.scores.clone(),
            &self.tricks_taken,
            self.shorts_piles.iter().map(|sp| sp.len() as i32).collect(),
        );
        let mut max_score = 0;
        for player in 0..3 {
            if self.scores[player] > max_score {
                max_score = self.scores[player];
            }
        }
        if !self.no_changes {
            for player in 0..3 {
                self.changes.push(vec![Change {
                    change_type: ChangeType::Score,
                    object_id: player,
                    player,
                    dest: Location::Score,
                    start_score: original_scores[player as usize],
                    end_score: self.scores[player as usize],
                    ..Default::default()
                }]);
            }
        }
        let mut high_score: i32 = 0;
        let mut winners: Vec<i32> = vec![];
        for player in 0..3 {
            let score = self.scores[player];
            if score > high_score {
                high_score = score;
            }
        }
        for player in 0..3 {
            let score = self.scores[player];
            if score == high_score {
                winners.push(player as i32);
            }
        }
        if self.round >= 3 {
            self.winner = Some(winners[0]);
            self.changes.push(vec![Change {
                change_type: ChangeType::GameOver,
                dest: Location::Deck,
                ..Default::default()
            }]);
            return true;
        } else {
            self.round += 1;
            self.changes.push(vec![Change {
                change_type: ChangeType::Shuffle,
                object_id: 0,
                source_offset: 0,
                dest: Location::Deck,
                dest_offset: 0,
                ..Default::default()
            }]);
            self.deal();
        }
        return true;
    }

    fn show_playable(self: &mut Game) -> Vec<Change> {
        if self.no_changes {
            return vec![];
        }
        let mut changes: Vec<Change> = vec![];

        if self.current_player == 0 {
            if self.state == State::OptionalDraw {
                changes.push(Change {
                    object_id: -1,
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::Hand,
                    dest_offset: self.current_player,
                    ..Default::default()
                });
                changes.push(Change {
                    object_id: -2,
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::Hand,
                    dest_offset: self.current_player,
                    ..Default::default()
                });
            } else {
                changes.push(Change {
                    object_id: -1,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    dest_offset: self.current_player,
                    ..Default::default()
                });
                changes.push(Change {
                    object_id: -2,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    dest_offset: self.current_player,
                    ..Default::default()
                });
                for action in self.get_moves() {
                    changes.push(Change {
                        object_id: card_offset(self.state, action),
                        change_type: ChangeType::ShowPlayable,
                        dest: Location::Hand,
                        dest_offset: self.current_player,
                        ..Default::default()
                    });
                }
            }
            changes
        } else {
            let mut hide_changes = self.hide_playable();
            changes.append(&mut hide_changes);
            changes
        }
    }

    fn hide_playable(self: &Game) -> Vec<Change> {
        if self.no_changes {
            return vec![];
        }
        let mut changes: Vec<Change> = vec![];
        for card in &self.hands[0] {
            changes.push(Change {
                object_id: card.id,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                dest_offset: self.current_player,
                ..Default::default()
            });
        }
        changes.push(Change {
            object_id: -1,
            change_type: ChangeType::HidePlayable,
            dest: Location::Hand,
            dest_offset: self.current_player,
            ..Default::default()
        });
        changes.push(Change {
            object_id: -2,
            change_type: ChangeType::HidePlayable,
            dest: Location::Hand,
            dest_offset: self.current_player,
            ..Default::default()
        });
        changes
    }
}

fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

pub fn get_winner(lead_suit: Option<Suit>, trick: [Option<Card>; 3]) -> i32 {
    let mut card_id_to_player: HashMap<i32, i32> = HashMap::new();
    for (player, card) in trick.iter().enumerate() {
        if let Some(card) = card {
            card_id_to_player.insert(card.id, player as i32);
        }
    }
    let mut cards: Vec<Card> = trick
        .iter() // Convert the Vec into an Iterator
        .filter_map(|&x| x) // filter_map will only pass through the Some values
        .collect();
    cards.sort_by_key(|c| std::cmp::Reverse(value_for_card(lead_suit, c)));
    return *card_id_to_player
        .get(&cards.first().expect("there should be a winning card").id)
        .expect("cards_to_player missing card");
}

pub fn value_for_card(lead_suit: Option<Suit>, card: &Card) -> i32 {
    let mut lead_bonus: i32 = 0;
    if Some(card.suit) == lead_suit {
        lead_bonus += 100;
    }
    card.value + lead_bonus
}

pub fn score_game(
    original_scores: Vec<i32>,
    tricks_taken: &[i32; 3],
    shorts_pile_lengths: Vec<i32>,
) -> Vec<i32> {
    let mut scores = original_scores.clone();
    for player in 0..3 {
        scores[player] += tricks_taken[player];
        let mut score_per_match = 3;
        if shorts_pile_lengths[player] == tricks_taken[player] {
            score_per_match = 5;
        }
        let match_count = min(shorts_pile_lengths[player], tricks_taken[player]);
        scores[player] += match_count * score_per_match;
    }
    scores
}

pub fn reorder_hand(player: i32, hand: &Vec<Card>) -> Vec<Change> {
    let mut changes: Vec<Change> = vec![];
    for (offset_in_hand, card) in hand.iter().enumerate() {
        changes.push(Change {
            object_id: card.id,
            player,
            dest: Location::ReorderHand,
            change_type: ChangeType::Reorder,
            hand_offset: offset_in_hand as i32,
            length: hand.len() as i32,
            ..Default::default()
        });
    }
    changes
}

impl ismcts::Game for Game {
    type Move = i32;
    type PlayerTag = i32;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        for p1 in 0..3 {
            for p2 in 0..3 {
                if p1 == self.current_player() || p2 == self.current_player() || p1 == p2 {
                    continue;
                }

                let mut combined_voids: HashSet<Suit> =
                    HashSet::from_iter(self.voids[p1 as usize].iter().cloned());
                combined_voids.extend(self.voids[p2 as usize].iter());

                let mut new_hands = vec![
                    self.hands[p1 as usize].clone(),
                    self.hands[p2 as usize].clone(),
                ];
                //println!("original hands: {:?}", new_hands);

                // allow swapping of any cards that are not in the combined void set
                shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids.contains(&c.suit),
                    &mut new_hands,
                    &mut thread_rng(),
                );

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();
                //println!("new hands: {:?} {:?}", self.hands[p1 as usize], self.hands[p2 as usize]);

                // Draw deck shuffling

                let mut new_draw_decks = vec![
                    self.draw_decks[p1 as usize].clone(),
                    self.draw_decks[p2 as usize].clone(),
                ];

                // allow swapping of any cards
                shuffle_and_divide_matching_cards(
                    |_c: &Card| true,
                    &mut new_draw_decks,
                    &mut thread_rng(),
                );

                self.draw_decks[p1 as usize] = new_draw_decks[0].clone();
                self.draw_decks[p2 as usize] = new_draw_decks[1].clone();
            }
        }
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player + 1) % 3
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.apply_move(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        const MAX_POINTS_PER_HAND: f64 = 25.0;
        if self.winner == None {
            None
        } else {
            if self.experiment {
                println!("experiment");
                let mut sorted = self.scores.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                let rank = sorted
                    .iter()
                    .position(|&s| s == self.scores[player as usize])
                    .unwrap();
                let reward = 1.0 - (rank as f64 / (sorted.len() - 1) as f64); // Top player: 1.0, bottom: 0.0
                return Some(reward);
            }
            println!("non experiment");

            let mut sorted_scores = self.scores.clone();
            sorted_scores.sort();
            sorted_scores.reverse();
            let high_score = sorted_scores[0] as f64;
            let score = self.scores[player as usize] as f64;
            let mut winners = 0;
            for score in sorted_scores {
                if score == high_score as i32 {
                    winners += 1;
                }
            }
            if score != high_score {
                let normalized_score = (score.abs() as f64) / 25.0;
                // Normalizing the score to 0 - .2
                Some(0.2 * (1.0 - normalized_score))
            } else {
                // divide by number of > 0 scoring players to incentivize
                // minimizing the number of other winners
                let score = (score as f64 / high_score as f64) / winners as f64;
                Some(0.2 + (0.8 * score))
            }
        }
    }
}

pub fn get_mcts_move(game: &Game, iterations: i32) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    new_game.scores = vec![0, 0, 0];
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
    fn test_deck() {
        let d = deck();
        assert_eq!(d.len(), 48);
    }

    #[test]
    fn test_game_initialization() {
        let mut game = Game::new();
        for hand in game.hands.iter() {
            assert_eq!(hand.len(), 16);
        }
        for draw_deck in game.draw_decks.iter() {
            assert_eq!(draw_deck.len(), 0);
        }
        // Move the game through the discard phase
        for _ in 0..15 {
            assert_eq!(game.state, State::Discard);
            let action = *game.get_moves().first().unwrap();
            game.apply_move(action);
        }
        assert_eq!(game.state, State::OptionalDraw);
        assert!(game.draw_decks.iter().all(|dd| dd.len() == 5));
        assert!(game.hands.iter().all(|h| h.len() == 11));
        for _ in 0..3 {
            assert_eq!(game.state, State::OptionalDraw);
            game.apply_move(DRAW);
        }
        // each player drew a card so we should have 4 left in
        // each draw deck
        assert!(game.draw_decks.iter().all(|dd| dd.len() == 4));
        assert!(game.hands.iter().all(|h| h.len() == 12));
        assert_eq!(game.state, State::Play);
        for _ in 0..3 {
            let action = *game.get_moves().first().unwrap();
            game.apply_move(action);
        }
        assert_eq!(game.tricks_taken.iter().sum::<i32>(), 1);

        assert!(game.hands.iter().all(|dd| dd.len() == 11));
    }

    #[test]
    fn test_get_winner() {
        assert_eq!(
            get_winner(
                Some(Suit::Blue),
                [
                    Some(Card {
                        id: 0,
                        value: 7,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 1,
                        value: 8,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 2,
                        value: 9,
                        suit: Suit::Blue
                    }),
                ]
            ),
            2
        );
        assert_eq!(
            get_winner(
                Some(Suit::Blue),
                [
                    Some(Card {
                        id: 0,
                        value: 9,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 1,
                        value: 8,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 2,
                        value: 1,
                        suit: Suit::Red
                    }),
                ]
            ),
            0
        );
    }

    #[test]
    fn test_random_playthrough() {
        let mut game = Game::new();
        game.round = 4;
        while game.winner.is_none() {
            let action = *game.get_moves().first().unwrap();
            game.apply_move(action);
        }
    }

    struct ScoreCase {
        tricks_taken: [i32; 3],
        shorts: Vec<i32>,
        expected_scores: Vec<i32>,
    }

    #[test]
    fn test_score_game() {
        let cases = vec![
            // 0: Brother Barenstain won 1 trick and has 1 short: 1 point for 1 won
            //    trick and 5 points for the 1 equal pair: 6 total points.
            // 1: Sister Barenstain won 3 tricks and has 3 shorts: 3 points for 3
            //    won tricks and 15 points for the 3 equal pairs: 18 total.
            // 2: Ditka won 3 tricks and has 2 shorts: 3 points for 3 won tricks
            //    and 6 points for the 2 unequal pairs: 9 total.
            ScoreCase {
                tricks_taken: [1, 3, 3],
                shorts: vec![1, 3, 2],
                expected_scores: vec![6, 18, 9],
            },
            // 0: Smokey won 1 trick and shorted 6 times: 1 point for 1 won trick
            //    and 3 points for the 1 pair: 4 total
            ScoreCase {
                tricks_taken: [1, 0, 0],
                shorts: vec![6, 0, 0],
                expected_scores: vec![4, 0, 0],
            },
        ];
        for test_case in cases.iter() {
            let scores = score_game(
                vec![0, 0, 0],
                &test_case.tricks_taken,
                test_case.shorts.clone(),
            );
            assert_eq!(scores, *test_case.expected_scores);
        }
        // scores should be added to existing scores
        for test_case in cases {
            let scores = score_game(
                vec![1, 1, 1],
                &test_case.tricks_taken,
                test_case.shorts.clone(),
            );
            let expected_scores: Vec<i32> =
                test_case.expected_scores.iter().map(|s| s + 1).collect();
            assert_eq!(scores, expected_scores);
        }
    }
}
