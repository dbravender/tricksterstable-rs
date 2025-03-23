/*
Game: Torchlit
Designer: David Spalinski
BoardGameGeek: https://boardgamegeek.com/boardgame/433205/torchlit
*/

use core::panic;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::thread_rng;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

// Used to indicate when a player does not want to spawn more monsters
const CONFIRM_SPAWN: i32 = -2;
const UNDO_SPAWN: i32 = -3;
// Max points
// 2 - 1 point for each card of that number (6 suits)
// 2 points for a dragon (one of each number)
// 6 2 points for each opponent torch
// 3 points for lighting one's own torch
const MAX_POINTS_PER_HAND: f64 = 13.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    // Select a card from hand as torch
    LightTorch,
    // Trick play
    Play,
    // Select cards in the current trick to play on dungeons
    SpawnMonsters,
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
    Goblin = 0,
    Ghost = 1,
    Mimic = 2,
    Beholder = 3,
    Skull = 4,
    Dragon = 5,
    Ooze = 6,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    TricksTaken,
    Dungeon,
    Score,
    ReorderHand,
    Message,
    CardsBurned,
    Torch,
    // Cards which will be played to the dungeon
    WardenStaged,
    // Cards which will not be played to the dungeon
    WardenRemoved,
    // Cards that have been put in a dungeon
    DungeonMonsters,
    // Cards that are being scored
    ScoredCards,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    pub suit: Suit,
    value: i32,
    dropped_torch: bool,
}

impl Card {
    pub fn get_points(&self) -> i32 {
        if self.dropped_torch {
            2
        } else if self.suit == Suit::Dragon {
            2
        } else {
            1
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    TricksToDungeon,
    Faceup,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCards,
    GameOver,
    Reorder,
    PassCard,
    Message,
    // Move undealt cards off the table
    CardsBurned,
    LightTorch,
    MoveAdventurer,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    object_id: i32,
    dest: Location,
    start_score: i32,
    end_score: i32,
    offset: usize,
    player: usize,
    length: usize,
    highlight: bool,
    message: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TrickResult {
    pub dungeon_warden: usize,
    pub movers: Vec<usize>,
    pub winning_value: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchlitGame {
    // Current game state
    pub state: State,
    // Which player is making a move now
    pub current_player: usize, // 0 - 2
    // Player who led the current trick
    pub lead_player: usize,
    // Cards each player has played in the current trick
    pub current_trick: [Option<Card>; 4],
    // Cards in each player's hand
    pub hands: [Vec<Card>; 4],
    // Voids revealed when a player couldn't follow a lead card (used during determination)
    pub voids: [Vec<Suit>; 4],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    pub changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    pub no_changes: bool,
    // Current score of the game (per team)
    pub scores: [i32; 4],
    // Game winner
    pub winner: Option<usize>,
    // Use experimental reward function for comparison
    pub experiment: bool,
    // Current round
    pub round: usize,
    // Which player is the human player
    pub human_player: Option<usize>,
    // Cards selected as the torch card
    pub torches: [Option<Card>; 4],
    // Cards at each dungeon space
    pub dungeon_cards: [Vec<Card>; 8],
    // Dungeon card player is on
    pub player_dungeon_offset: [i32; 4],
    // Cards that have not been selected to be spawned - initially set to the trick
    pub spawnable_cards: Vec<Card>,
    // Cards that have been staged to be spawned
    pub spawnable_staged: Vec<Card>,
}

impl TorchlitGame {
    pub fn new() -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..4);
        game.deal();
        game
    }

    pub fn new_with_human_player(human_player: usize) -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        let mut rng = rand::thread_rng();
        game.dealer = rng.gen_range(0..4);
        game.human_player = Some(human_player);
        game.deal();
        game
    }

    // Called at the start of a game and when a new hand is dealt
    pub fn deal(&mut self) {
        self.state = State::LightTorch;
        self.round += 1;
        self.hands = [vec![], vec![], vec![], vec![]];
        self.player_dungeon_offset = [0; 4];
        self.torches = [None; 4];
        self.current_player = self.dealer;
        self.lead_player = self.current_player;
        self.current_trick = [None; 4];
        self.dealer = (self.dealer + 1) % 4;
        self.voids = [vec![], vec![], vec![], vec![]];
        self.dungeon_cards = [
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ];
        self.spawnable_cards = vec![];
        self.spawnable_staged = vec![];
        let mut cards = TorchlitGame::deck();
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
        for player in 0..4 {
            self.add_change(
                shuffle_index,
                Change {
                    change_type: ChangeType::MoveAdventurer,
                    player,
                    offset: 0,
                    dest: Location::Dungeon,
                    ..Default::default()
                },
            );
        }
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

        for suit in all::<Suit>() {
            for value in 0..=7 {
                deck.push(Card {
                    id,
                    value,
                    suit,
                    dropped_torch: false,
                });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
    }

    pub fn get_moves(self: &TorchlitGame) -> Vec<i32> {
        match self.state {
            State::LightTorch => self.playable_card_ids(),
            State::Play => self.playable_card_ids(),
            State::SpawnMonsters => self.spawn_actions(),
        }
    }

    pub fn current_player_card_ids(&self) -> Vec<i32> {
        self.hands[self.current_player]
            .iter()
            .map(|c| c.id)
            .collect()
    }

    pub fn get_lead_suit(&self) -> Option<Suit> {
        self.current_trick[self.lead_player].map(|card| card.suit)
    }

    pub fn playable_card_ids(&self) -> Vec<i32> {
        if self.torches[self.current_player].is_none() {
            // Any card can be played on the last trick when the torch card
            // is placed back in the players' hands (may follow)
            return self.current_player_card_ids();
        }
        let lead_suit = self.get_lead_suit();
        // Must follow except when a dragon is led
        if lead_suit.is_some() && lead_suit != Some(Suit::Dragon) {
            let moves: Vec<i32> = self.hands[self.current_player]
                .iter()
                .filter(|c| Some(c.suit) == lead_suit)
                .map(|c| c.id)
                .collect();
            if !moves.is_empty() {
                return moves;
            }
        }
        self.current_player_card_ids()
    }

    pub fn spawn_actions(&self) -> Vec<i32> {
        let mut playable_actions: Vec<i32> = self.spawnable_cards.iter().map(|c| c.id).collect();
        let trick_suits: HashSet<Suit> = self
            .current_trick
            .iter()
            .flatten()
            .map(|c| c.suit)
            .collect();

        if trick_suits.len() == 4 {
            playable_actions.push(CONFIRM_SPAWN);
        } else if playable_actions.is_empty() {
            playable_actions.push(CONFIRM_SPAWN);
        }

        if Some(self.current_player) == self.human_player && !self.spawnable_staged.is_empty() {
            playable_actions.push(UNDO_SPAWN);
        }

        playable_actions
    }

    fn pop_card(&mut self, id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == id)
            .unwrap();
        let card = self.hands[self.current_player].remove(pos);
        return card;
    }

    fn apply_move_internal(&mut self, action: i32) {
        match self.state {
            State::LightTorch => {
                let card = self.pop_card(action);
                self.torches[self.current_player] = Some(card);
                let change_index = self.new_change();
                self.add_change(
                    change_index,
                    Change {
                        change_type: ChangeType::LightTorch,
                        object_id: action,
                        dest: Location::Torch,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
                if self.current_player == 0 {
                    self.reorder_hand(0, false);
                }
                self.current_player = (self.current_player + 1) % 4;
                if self.torches[self.current_player].is_some() {
                    // When everyone has played a torch begin the trick taking phase
                    self.state = State::Play;
                }
            }
            State::Play => {
                let card = self.pop_card(action);
                let lead_suit = self.get_lead_suit();
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

                // Dragons do not need to be followed
                if lead_suit.is_some() && lead_suit != Some(Suit::Dragon) {
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

                    if self.hands.iter().map(|h| h.len()).all(|l| l == 1) {
                        // When there is only one card left in each player's hand each player's torch
                        // card is moved back into their hand
                        for (player, torch) in self.torches.iter_mut().enumerate() {
                            if let Some(torch) = torch.take() {
                                self.hands[player].push(torch);
                            }
                        }
                        self.reorder_hand(0, false);
                    }

                    let trick_result = self.get_trick_result();
                    let movers = trick_result.movers;
                    let index = self.new_change();
                    for trick_winner in &movers {
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::ShowWinningCards,
                                object_id: self.current_trick[*trick_winner].unwrap().id,
                                dest: Location::Play,
                                ..Default::default()
                            },
                        );
                    }
                    for trick_winner in &movers {
                        let index = self.new_change();
                        self.player_dungeon_offset[*trick_winner] =
                            (self.player_dungeon_offset[*trick_winner] + 1) % 7;
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::MoveAdventurer,
                                player: *trick_winner,
                                offset: self.player_dungeon_offset[*trick_winner] as usize,
                                highlight: true,
                                dest: Location::Dungeon,
                                ..Default::default()
                            },
                        );
                    }

                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::Message,
                            message: Some(format!(
                                "All players that played {} advance",
                                trick_result.winning_value
                            )),
                            ..Default::default()
                        },
                    );

                    let index = self.new_change();
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::OptionalPause,
                            object_id: 0,
                            dest: Location::Play,
                            ..Default::default()
                        },
                    );

                    let index = self.new_change();
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::Message,
                            ..Default::default()
                        },
                    );
                    // Remove highlight from adventurers
                    for trick_winner in &movers {
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::MoveAdventurer,
                                player: *trick_winner,
                                offset: self.player_dungeon_offset[*trick_winner] as usize,
                                highlight: false,
                                dest: Location::Dungeon,
                                ..Default::default()
                            },
                        );
                    }

                    self.current_player = trick_result.dungeon_warden;
                    self.lead_player = self.current_player;

                    self.state = State::SpawnMonsters;
                    self.reset_spawn();
                }
            }
            State::SpawnMonsters => {
                if action == CONFIRM_SPAWN {
                    let index = self.new_change();
                    if Some(self.current_player) != self.human_player {
                        let player_name = self.player_name_string();
                        let message = format!("{} selected which monsters to spawn", player_name,);
                        self.set_message(Some(message), index);
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::OptionalPause,
                                ..Default::default()
                            },
                        );
                    }
                    for card in self.spawnable_staged.clone().iter() {
                        self.dungeon_cards[card.value as usize].push(*card);
                        let height = self.dungeon_cards[card.value as usize].len() - 1;
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::TricksToDungeon,
                                object_id: card.id,
                                offset: card.value as usize,
                                length: height,
                                dest: Location::DungeonMonsters,
                                ..Default::default()
                            },
                        );
                    }

                    self.end_trick();

                    return;
                }
                if action == UNDO_SPAWN {
                    self.reset_spawn();
                    return;
                }
                let pos = self
                    .spawnable_cards
                    .iter()
                    .position(|c| c.id == action)
                    .unwrap();
                let card = self.spawnable_cards.remove(pos);
                let index = self.new_change();
                let mut height = self.dungeon_cards[card.value as usize].len();
                // Remove all cards that share the suit of the staged card from spawnable cards
                for c in self.spawnable_staged.clone().iter() {
                    if c.value == card.value {
                        height += 1;
                    }
                    if c.suit == card.suit {
                        self.add_change(
                            index,
                            Change {
                                change_type: ChangeType::Play,
                                object_id: card.id,
                                dest: Location::WardenRemoved,
                                ..Default::default()
                            },
                        );
                    }
                }
                self.spawnable_cards.retain(|c| c.suit != card.suit);
                self.spawnable_staged.push(card);
                self.add_change(
                    index,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: card.id,
                        dest: Location::DungeonMonsters,
                        offset: card.value as usize,
                        highlight: true,
                        length: height,
                        ..Default::default()
                    },
                );
            }
        }
    }

    pub fn reset_spawn(&mut self) {
        self.spawnable_staged = vec![];
        self.spawnable_cards = self.current_trick.iter().flatten().cloned().collect();
        let index = self.new_change();
        for (player, card) in self.current_trick.clone().iter().flatten().enumerate() {
            self.add_change(
                index,
                Change {
                    object_id: card.id,
                    change_type: ChangeType::Play,
                    dest: Location::Play,
                    player,
                    ..Default::default()
                },
            );
        }
        self.show_playable();
    }

    pub fn end_trick(&mut self) {
        self.state = State::Play;
        self.current_player = self.lead_player;

        // Animate spawned cards from the staging area to the dungeons
        let change_index = self.new_change();

        for card in self.current_trick {
            if self.spawnable_staged.contains(&card.unwrap()) {
                continue;
            }
            self.add_change(
                change_index,
                Change {
                    change_type: ChangeType::CardsBurned,
                    object_id: card.unwrap().id,
                    dest: Location::CardsBurned,
                    ..Default::default()
                },
            );
        }

        // Clear trick
        self.current_trick = [None; 4];
        self.spawnable_staged = vec![];
        self.spawnable_cards = vec![];

        if self.hands.iter().all(|h| h.len() == 1) {
            // Score the hand
            let mut earned_this_hand = [0; 4];
            let mut players_per_space: HashMap<i32, i32> = HashMap::new();

            for player in 0..4 {
                let mut torch_card = self.hands[player].first().unwrap().clone();
                if torch_card.value == self.player_dungeon_offset[player] {
                    earned_this_hand[player] = 3;
                } else {
                    torch_card.dropped_torch = true;
                    self.dungeon_cards[torch_card.value as usize].push(torch_card.clone());
                    // TODO: Animate torch to dungeon space
                }
                *players_per_space
                    .entry(self.player_dungeon_offset[player])
                    .or_insert(0) += 1;
            }

            for player in 0..4 {
                let total_players_at_space = players_per_space
                    .get(&self.player_dungeon_offset[player])
                    .unwrap_or(&1);
                let score: i32 = self.dungeon_cards[self.player_dungeon_offset[player] as usize]
                    .iter()
                    .map(|c| c.get_points())
                    .sum();
                earned_this_hand[player] +=
                    ((score / total_players_at_space) as f64).floor() as i32;
            }

            // Animate the scores
            let score_change_index = self.new_change();
            for player in 0..4 {
                self.add_change(
                    score_change_index,
                    Change {
                        change_type: ChangeType::Score,
                        object_id: 0,
                        dest: Location::Score,
                        player,
                        start_score: self.scores[player],
                        end_score: self.scores[player] + earned_this_hand[player],
                        ..Default::default()
                    },
                );
                self.scores[player] += earned_this_hand[player];
            }

            if self.round >= 3 {
                // The game is over
                let max_score = self.scores.iter().max().unwrap();
                // FIXME: implement tiebreaker
                // Tiebreaker
                // If there is a tie, the player who scored the most
                // points in the last round wins
                // If a tie persists, the victory is shared
                self.winner = Some(self.scores.iter().position(|&x| x == *max_score).unwrap());
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
            self.deal();
            return;
        }
    }

    pub fn apply_move(&mut self, action: i32) {
        self.changes = vec![vec![]]; // card from player to table
        if !self.get_moves().contains(&action) {
            println!("Invalid move: {}", action);
            println!("Moves: {:?}", self.get_moves());
            panic!("Invalid move");
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
        self.add_change(
            change_index,
            Change {
                object_id: CONFIRM_SPAWN,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                player: self.current_player,
                ..Default::default()
            },
        );
        self.add_change(
            change_index,
            Change {
                object_id: UNDO_SPAWN,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                player: self.current_player,
                ..Default::default()
            },
        );
        if self.human_player.is_some() && self.current_player == self.human_player.unwrap() {
            if self.state == State::SpawnMonsters {
                for card in self.current_trick.clone().iter().flatten() {
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
                for card in self.spawnable_staged.clone().iter() {
                    self.add_change(
                        change_index,
                        Change {
                            object_id: card.id,
                            change_type: ChangeType::ShowPlayable,
                            dest: Location::Hand,
                            player: self.current_player,
                            ..Default::default()
                        },
                    );
                }
            }
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
        let player_name = self.player_name_string();
        let player_possessive = match self.current_player {
            0 => "your",
            _ => "their",
        };
        let message = match self.state {
            State::LightTorch => Some(format!(
                "{} must select a card as {} torch",
                player_name, player_possessive
            )),
            State::Play => None,
            State::SpawnMonsters => Some(format!(
                "{} must select where to spawn monsters",
                player_name
            )),
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
        self.add_change(
            change_index,
            Change {
                object_id: UNDO_SPAWN,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                player: self.current_player,
                ..Default::default()
            },
        );
        self.add_change(
            change_index,
            Change {
                object_id: CONFIRM_SPAWN,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                player: self.current_player,
                ..Default::default()
            },
        );
    }

    pub fn get_trick_result(&self) -> TrickResult {
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
        let winning_value = cards.first().expect("there should be a winning card").value;
        let lowest_value = cards.iter().map(|c| c.value).min().unwrap();
        // Iterate over all cards played starting with the lead player
        // The player that played the lowest card last becomes the dungeon warden
        let mut dungeon_warden: usize = 0;
        for offset in 0..4 {
            let player = (self.lead_player + offset) % 4;
            let card = self.current_trick[player].unwrap();
            if card.value == lowest_value {
                dungeon_warden = player;
            }
        }

        TrickResult {
            dungeon_warden,
            movers: cards
                .iter()
                .filter(|c| c.value == winning_value)
                .map(|c| card_id_to_player[&c.id])
                .collect(),
            winning_value,
        }
    }

    pub fn value_for_card(&self, card: &Card) -> i32 {
        let lead_suit = self.get_lead_suit().unwrap();
        let mut bonus: i32 = 0;
        if card.suit == lead_suit {
            bonus += 100;
        }
        if card.suit == Suit::Dragon {
            bonus += 200;
        }
        card.value + bonus
    }
}

impl ismcts::Game for TorchlitGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        for p1 in 0..4 {
            for p2 in 0..4 {
                if p1 == self.current_player() || p2 == self.current_player() || p1 == p2 {
                    continue;
                }

                let mut combined_voids: HashSet<Suit> =
                    HashSet::from_iter(self.voids[p1 as usize].iter().cloned());
                combined_voids.extend(self.voids[p2 as usize].iter());

                let mut cards1 = self.hands[p1 as usize].clone();
                let mut cards2 = self.hands[p2 as usize].clone();

                if let Some(torch) = self.torches[p1] {
                    cards1.push(torch);
                }
                if let Some(torch) = self.torches[p2] {
                    cards2.push(torch);
                }

                let mut new_hands = vec![cards1, cards2];

                // allow swapping of any cards that are not in the combined void set
                shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids.contains(&c.suit),
                    &mut new_hands,
                    rng,
                );

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();
                if self.torches[p1].is_some() {
                    let torch = self.hands[p1 as usize].pop();
                    self.torches[p1] = torch;
                }
                if self.torches[p2].is_some() {
                    let torch = self.hands[p2 as usize].pop();
                    self.torches[p2] = torch;
                }
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
        if self.winner.is_none() {
            None
        } else {
            if !self.experiment {
                let total_score_ratio = self.scores[player] as f64 / MAX_POINTS_PER_HAND;

                // Scale the total score to a range between -1.0 and 1.0
                let final_score = (total_score_ratio * 2.0) - 1.0;

                Some(final_score)
            } else {
                let max_score = *self.scores.iter().max().unwrap() as f64;
                let player_score = self.scores[player] as f64;

                if player_score == max_score {
                    // Winner gets a positive score scaled between 0 and 1
                    let final_score = player_score / MAX_POINTS_PER_HAND;
                    Some(final_score)
                } else {
                    // Losers get a negative score between -1.0 and 0
                    let loss_penalty = (player_score - max_score) / max_score;
                    Some(loss_penalty) // This will be negative
                }
            }
        }
    }
}

pub fn get_mcts_move(game: &TorchlitGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    // reset scores for the simulation
    new_game.scores = [0; 4];
    new_game.round = 4; // force evaluation of a single hand
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
        let d = TorchlitGame::deck();
        assert_eq!(d.len(), 56);
    }

    #[derive(Debug)]
    struct TrickResultTestCase {
        description: String,
        current_trick: [Option<Card>; 4],
        lead_player: usize,
        expected_result: TrickResult,
    }

    #[test]
    fn test_trick_result() {
        let test_cases = [
            TrickResultTestCase {
                description: "Highest dragon wins the trick".to_string(),
                lead_player: 2,
                current_trick: [
                    Some(Card {
                        suit: Suit::Dragon,
                        value: 0,
                        id: 1,
                        dropped_torch: false,
                    }),
                    Some(Card {
                        id: 2,
                        value: 5,
                        suit: Suit::Beholder,
                        dropped_torch: false,
                    }),
                    Some(Card {
                        id: 3,
                        value: 6,
                        suit: Suit::Goblin,
                        dropped_torch: false,
                    }),
                    Some(Card {
                        id: 3,
                        value: 1,
                        suit: Suit::Goblin,
                        dropped_torch: false,
                    }),
                ],
                expected_result: TrickResult {
                    dungeon_warden: 0,
                    movers: vec![0],
                    winning_value: 0,
                },
            },
            TrickResultTestCase {
                description: "Highest lead card wins, last player to play lowest card is warden"
                    .to_string(),
                lead_player: 1,
                current_trick: [
                    Some(Card {
                        suit: Suit::Ghost,
                        value: 1,
                        id: 0,
                        dropped_torch: false,
                    }),
                    Some(Card {
                        suit: Suit::Goblin,
                        value: 1,
                        id: 1,
                        dropped_torch: false,
                    }),
                    Some(Card {
                        id: 2,
                        value: 1,
                        suit: Suit::Skull,
                        dropped_torch: false,
                    }),
                    Some(Card {
                        id: 3,
                        value: 1,
                        suit: Suit::Goblin,
                        dropped_torch: false,
                    }),
                ],
                expected_result: TrickResult {
                    dungeon_warden: 0,
                    movers: vec![1, 3, 0, 2],
                    winning_value: 1,
                },
            },
        ];
        for test_case in test_cases {
            let mut game = TorchlitGame::new();
            game.lead_player = test_case.lead_player;
            game.current_trick = test_case.current_trick;
            assert_eq!(
                game.get_trick_result(),
                test_case.expected_result,
                "{} {:?}",
                test_case.description,
                test_case
            );
        }
    }
}
