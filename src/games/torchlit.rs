/*
Game: Torchlit
Designer: David Spalinski
BoardGameGeek: https://boardgamegeek.com/boardgame/433205/torchlit
*/

use core::panic;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    ops::RangeInclusive,
};

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::thread_rng;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

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
    Goblins = 0,
    Ghosts = 1,
    TreasureChests = 2,
    FlamingEyes = 3,
    Skulls = 4,
    Dragons = 5,
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
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    pub suit: Suit,
    value: i32,
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
    ShowWinningCard,
    GameOver,
    Reorder,
    PassCard,
    Message,
    // Move undealt cards off the table
    CardsBurned,
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
pub struct TorchlitGame {
    // Current game state
    pub state: State,
    // Which player is making a move now
    pub current_player: usize, // 0 - 2
    // Player who led the current trick
    pub lead_player: usize,
    // Cards each player has played in the current trick
    pub current_trick: [Option<Card>; 3],
    // Cards in each player's hand
    pub hands: [Vec<Card>; 3],
    // Voids revealed when a player couldn't follow a lead card (used during determination)
    pub voids: [Vec<Suit>; 3],
    // Player who starts the next hand
    pub dealer: usize,
    // List of list of animations to run after a move is made to get from the current state to the next state
    pub changes: Vec<Vec<Change>>,
    // When running simulations we save time by not creating vecs and structs to be added to the change animation list
    pub no_changes: bool,
    // Current score of the game (per team)
    pub scores: [i32; 3],
    // Game winner
    pub winner: Option<usize>,
    // Use experimental reward function for comparison
    pub experiment: bool,
    // Current round
    pub round: usize,
    // Which player is the human player
    pub human_player: Option<usize>,
    // 3 cards that were not dealt to players (used during determination)
    pub burned_cards: Vec<Card>,
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
        self.hands = [vec![], vec![], vec![]];
        self.current_player = self.dealer;
        self.lead_player = self.current_player;
        self.current_trick = [None; 3];
        self.dealer = (self.dealer + 1) % 3;
        self.voids = [vec![], vec![], vec![]];
        let mut cards = Torchlit::deck();
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
            for player in 0..3 {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        offset: hand_index,
                        length: 15,
                        ..Default::default()
                    },
                );
                self.hands[player].push(card);
            }
        }
        // Keep track of the remaining cards for distribution during the simulation
        // so the bots don't have the unfair advantage of knowing the exact cards in play
        self.burned_cards = cards;
        for card in self.burned_cards.clone() {
            self.add_change(
                deal_index,
                Change {
                    change_type: ChangeType::CardsBurned,
                    object_id: card.id,
                    dest: Location::CardsBurned,
                    ..Default::default()
                },
            );
        }
        for player in 0..3 {
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
            for value in 0..=6 {
                deck.push(Card { id, value, suit });
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
            State::SpawnMonsters => self.spawn_monster_card_ids(),
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
        // Must follow
        if let Some(lead_suit) = self.get_lead_suit() {
            let mut moves: Vec<i32> = self.hands[self.current_player]
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
            State::LightTorch => {
                todo!();
            }
            State::Play => {
                todo!(); // Review all this code
                let lead_suit = self.get_lead_suit();

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
                        // Zeroes and the King card don't always reveal voids
                        && card.value != 0
                        && card.id != KING_ID
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

                    self.lead_player = trick_winner;

                    self.score_trick();
                }
            }
            State::SpawnMonsters => {
                todo!();
            }
        }
    }

    pub fn team_can_play_church_of_england(&self, trick_losing_team: usize) -> bool {
        if self.church_of_england_played {
            // The Church of England ability has already been used this hand
            return false;
        }
        // The Church of England card cannot be used on a trick that includes the King
        if self
            .current_trick
            .iter()
            .filter_map(|c| *c)
            .any(|c| c.id == KING_ID)
        {
            return false;
        }
        if self.current_trump == Suit::Black {
            // The Church of England ability can only be used when Red or beyond is trump
            return false;
        }
        // Only the team with the lower score at the start of a hand can use the
        // Church of England ability
        return self.scores[trick_losing_team] < self.scores[(trick_losing_team + 1) % 2];
    }

    pub fn score_trick(&mut self, trick_annulled: bool) {
        if trick_annulled {
            self.church_of_england_played = true;
        }
        self.state = State::Play;
        let trick_winner = self.lead_player;
        self.current_player = self.lead_player;

        // Animate tricks to winning team or offscrean if annulled
        let change_index = self.new_change();

        for card in self.current_trick {
            self.add_change(
                change_index,
                Change {
                    change_type: if trick_annulled {
                        ChangeType::TricksAnnulled
                    } else {
                        ChangeType::TricksToWinner
                    },
                    object_id: card.unwrap().id,
                    dest: if trick_annulled {
                        Location::TricksAnnulled
                    } else {
                        Location::TricksTaken
                    },
                    player: trick_winner,
                    tricks_taken: if trick_annulled {
                        1
                    } else {
                        (self.cards_taken[trick_winner % 2].len() / 4) as i32
                    },
                    ..Default::default()
                },
            );
        }

        // Clear trick
        self.current_trick = [None; 4];

        // Set trump
        let trick_number = 15 - self.hands[0].len() as i32;
        self.current_trump = trick_number_to_trump(trick_number);
        // Reset the trump track
        self.add_change(
            change_index,
            Change {
                change_type: ChangeType::TrumpChange,
                trick_number: Some(trick_number % 15),
                dest: Location::TrumpTrack,
                ..Default::default()
            },
        );

        if self.hands.iter().all(|x| x.is_empty()) {
            // The hand is over

            // Score the hand
            let mut earned_this_hand = [0; 2];
            // Each trick of 4 cards is worth 1 point
            for team in 0..2 {
                earned_this_hand[team] += self.cards_taken[team].len() as i32 / 4;
            }
            // Add all the points on the cards to the team's score
            for (team, cards) in self.cards_taken.iter().enumerate() {
                for card in cards {
                    earned_this_hand[team] += card.points;
                }
            }

            // Animate the scores
            let score_change_index = self.new_change();
            for team in 0..2 {
                self.add_change(
                    score_change_index,
                    Change {
                        change_type: ChangeType::Score,
                        object_id: 0,
                        dest: Location::Score,
                        player: team,
                        start_score: self.scores[team],
                        end_score: self.scores[team] + earned_this_hand[team],
                        ..Default::default()
                    },
                );
                self.scores[team] += earned_this_hand[team];
            }

            if self.round >= 4 {
                // The game is over
                if self.scores[0] == self.scores[1] {
                    // Tiebreaker
                    // If there is a tie, the team that does not have the
                    // King card wins. If neither team had the King card,
                    // the team that won the last trick wins.
                    if let Some(team_with_king) = self.team_with_king {
                        self.winner = Some((team_with_king + 1) % 2);
                    } else {
                        self.winner = Some(trick_winner);
                    }
                } else {
                    let max_score = self.scores.iter().max().unwrap();
                    self.winner = Some(self.scores.iter().position(|&x| x == *max_score).unwrap());
                }
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
        if self.human_player.is_some() && self.current_player == self.human_player.unwrap() {
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
            State::OptionallyPlayChurchOfEngland => Some(format!(
                "{} must decide whether to play the Church of England",
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
            2 => "Your partner".to_string(),
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
        // Hide the Church of England and pass action cards
        for id in [PASS, ANNUL_TRICK].iter() {
            self.add_change(
                change_index,
                Change {
                    object_id: *id,
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
        for card in cards.iter() {
            println!("{:?} = {}", card, self.value_for_card(&card));
        }
        cards.sort_by_key(|c| std::cmp::Reverse(self.value_for_card(c)));
        *card_id_to_player
            .get(&cards.first().expect("there should be a winning card").id)
            .expect("cards_to_player missing card")
    }

    pub fn value_for_card(&self, card: &Card) -> i32 {
        let lead_suit = self.get_lead_suit().unwrap();
        let mut bonus: i32 = 0;
        let mut treated_value = card.value;
        let mut treated_suit = card.suit;
        if card.value == 0 {
            match card.suit {
                Suit::Black => {
                    if lead_suit == Suit::Red
                        || (lead_suit != Suit::Black && self.current_trump == Suit::Red)
                    {
                        treated_suit = Suit::Red;
                        treated_value = 13;
                    }
                }
                Suit::Red => {
                    if lead_suit == Suit::Black
                        || (lead_suit != Suit::Red && self.current_trump == Suit::Black)
                    {
                        treated_suit = Suit::Black;
                        treated_value = 13;
                    }
                }
                _ => {}
            }
        }
        if treated_suit == lead_suit {
            bonus += 100;
        }
        if treated_suit == self.current_trump {
            bonus += 200;
        }
        if card.value == KING {
            bonus += 1000;
        }
        treated_value + bonus
    }
}

impl ismcts::Game for SixOfVIIIGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        for p1 in 0..4 {
            if p1 != self.current_player() {
                // randomly swap each player's hand with the burned cards
                let mut new_hands =
                    vec![self.hands[p1 as usize].clone(), self.burned_cards.clone()];

                // only swap cards that aren't in the current players void set
                shuffle_and_divide_matching_cards(
                    |c: &Card| !self.voids[p1].contains(&c.suit),
                    &mut new_hands,
                    rng,
                );

                self.hands[p1] = new_hands[0].clone();
                self.burned_cards = new_hands[1].clone();
            }

            for p2 in 0..4 {
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

                // allow swapping of any cards that are not in the combined void set
                shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids.contains(&c.suit),
                    &mut new_hands,
                    rng,
                );

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();
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
        let team = player % 2;
        if self.winner.is_none() {
            None
        } else {
            if !self.experiment {
                let total_score_ratio = self.scores[team] as f64 / MAX_POINTS_PER_HAND;

                // Scale the total score to a range between -1.0 and 1.0
                let final_score = (total_score_ratio * 2.0) - 1.0;

                Some(final_score)
            } else {
                todo!("No experiment implemented");
            }
        }
    }
}

pub fn get_mcts_move(game: &SixOfVIIIGame, iterations: i32, debug: bool) -> i32 {
    let mut new_game = game.clone();
    new_game.no_changes = true;
    // reset scores for the simulation
    new_game.scores = [0; 2];
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

fn trick_number_to_trump(trick_number: i32) -> Suit {
    match trick_number {
        0..=3 => Suit::Black,
        4..=6 => Suit::Red,
        7..=8 => Suit::Orange,
        9..=9 => Suit::Yellow,
        10..=11 => Suit::Green,
        12..=15 => Suit::Blue,
        _ => unreachable!(),
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
                description: "Red 0 is highest black card when black is trump and red is not led"
                    .to_string(),
                lead_player: 3,
                trump: Suit::Black,
                current_trick: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 0,
                        points: 0,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 12,
                        points: 0,
                        suit: Suit::Black,
                    }),
                    Some(Card {
                        id: 3,
                        value: 7,
                        points: 0,
                        suit: Suit::Black,
                    }),
                    Some(Card {
                        suit: Suit::Black,
                        value: 2,
                        points: 0,
                        id: 0,
                    }),
                ],
                expected_winner: 0,
            },
            TrickWinnerTestCase {
                description: "Red 0 is not trump when red is led".to_string(),
                lead_player: 0,
                trump: Suit::Black,
                current_trick: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        points: 0,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 0,
                        points: 0,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 3,
                        points: 0,
                        suit: Suit::Black,
                    }),
                    Some(Card {
                        id: 3,
                        value: 2,
                        points: 0,
                        suit: Suit::Green,
                    }),
                ],
                expected_winner: 2,
            },
            TrickWinnerTestCase {
                description: "Red 0 is highest red card when black is trump".to_string(),
                lead_player: 0,
                trump: Suit::Red,
                current_trick: [
                    Some(Card {
                        suit: Suit::Orange,
                        value: 3,
                        points: 0,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 0,
                        points: 0,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 3,
                        points: 0,
                        suit: Suit::Black,
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
                description: "Black 0 is the highest red card when red is trump".to_string(),
                lead_player: 0,
                trump: Suit::Red,
                current_trick: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        points: 0,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Black,
                        value: 0,
                        points: 0,
                        id: 1,
                    }),
                    Some(Card {
                        id: 2,
                        value: 10,
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
                description: "Black 0 is highest red card when red is trump".to_string(),
                lead_player: 0,
                trump: Suit::Red,
                current_trick: [
                    Some(Card {
                        suit: Suit::Orange,
                        value: 3,
                        points: 0,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Black,
                        value: 0,
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
            TrickWinnerTestCase {
                description: "King is higher than the highest trump card".to_string(),
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
                        value: KING,
                        points: 0,
                        suit: Suit::Purple,
                    }),
                    Some(Card {
                        id: 3,
                        value: 12,
                        points: 0,
                        suit: Suit::Blue,
                    }),
                ],
                expected_winner: 2,
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
