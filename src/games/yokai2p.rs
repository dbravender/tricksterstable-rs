/*
Game: Yokai Septet (2-player variant)
Yokai Septet Designers: yio, Muneyuki Yokouchi (横内宗幸)
2-player variant designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/251433/yokai-septet
*/

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

#[derive(
    Debug, PartialOrd, Ord, Clone, Copy, Sequence, Serialize, Deserialize, Hash, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
    Green,
    Purple,
    Pink,
    Yellow,
    Black,
    Red,
    Blue,
}

pub fn suit_offset(suit: Suit) -> i32 {
    match suit {
        Suit::Green => 0,
        Suit::Purple => 1,
        Suit::Pink => 2,
        Suit::Yellow => 3,
        Suit::Black => 4,
        Suit::Red => 5,
        Suit::Blue => 6,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: i32,
    pub value: i32,
    pub suit: Suit,
}

pub fn deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    let mut id = 0;
    for suit in all::<Suit>() {
        for value in 1..=7 {
            deck.push(Card {
                id,
                value: value + suit_offset(suit),
                suit,
            });
            id += 1;
        }
    }
    deck.shuffle(&mut thread_rng());
    deck
}

static ID_TO_CARD: Lazy<HashMap<i32, Card>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for card in deck().iter() {
        m.insert(card.id, *card);
    }
    m
});

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Faceup,
    Trump,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    CaptureSeven,
    GameOver,
    RevealCard,
    Discard,
    Reorder,
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Hand,
    Trump,
    Play,
    Faceup,
    TricksTaken,
    SevensPile,
    Score,
    StrawTop,
    StrawBottom,
    Discard,
    ReorderHand,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    #[serde(rename(serialize = "id", deserialize = "id"))]
    object_id: i32,
    dest: Location,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    hand_offset: usize,
    player: usize,
    length: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Discard,
    PlayCard,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Yokai2pGame {
    pub state: State,
    pub trump_card: Option<Card>,
    pub hands: [Vec<Card>; 2],
    pub changes: Vec<Vec<Change>>,
    pub current_trick: [Option<Card>; 2],
    pub tricks_taken: [i32; 2],
    pub lead_suit: Option<Suit>,
    pub scores: [i32; 2],
    pub hand_scores: [i32; 2],
    #[serde(skip)]
    pub voids: [HashSet<Suit>; 2],
    pub captured_sevens: [Vec<Card>; 2],
    pub straw_bottom: [Vec<Option<Card>>; 2],
    pub straw_top: [Vec<Option<Card>>; 2],
    pub current_player: usize,
    pub winner: Option<usize>,
    pub overall_winner: Option<usize>,
    pub lead_player: usize,
    pub round: i32,
    pub no_changes: bool, // save time when running simulations by skipping animation metadata
}

impl Yokai2pGame {
    pub fn new() -> Self {
        let mut game = Self {
            no_changes: false,
            ..Default::default()
        };
        game.deal();
        game
    }

    pub fn deal(&mut self) {
        self.lead_suit = None;
        self.round += 1;
        self.tricks_taken = [0, 0];
        self.hands = [vec![], vec![]];
        self.state = State::Discard;
        self.current_player = self.lead_player;
        self.lead_player = (self.lead_player + 1) % 2;
        self.captured_sevens = [vec![], vec![]];
        self.voids = [HashSet::new(), HashSet::new()];
        let mut cards = deck();
        self.trump_card = cards.pop();
        let deal_index = self.new_change();
        self.add_change(
            deal_index,
            Change {
                change_type: ChangeType::Trump,
                object_id: self.trump_card.unwrap().id,
                dest: Location::Trump,
                ..Default::default()
            },
        );
        self.straw_bottom = [vec![], vec![]];
        for y in 0..7 {
            for player in 0..2 as usize {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::StrawBottom,
                        player,
                        hand_offset: y,
                        length: 7,
                        ..Default::default()
                    },
                );
                self.straw_bottom[player].push(Some(card));
            }
        }
        // To avoid having to deal with moving 7s - okayed with Sean
        let removed_sevens: Vec<Card> = cards.iter().filter(|c| c.value == 7).cloned().collect();
        cards.retain(|c| c.value != 7);
        // End dealing with sevens
        self.straw_top = [vec![], vec![]];
        for y in 0..6 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::StrawTop,
                        player,
                        hand_offset: y,
                        length: 6,
                        ..Default::default()
                    },
                );
                self.straw_top[player].push(Some(card));
            }
        }
        // To avoid having to deal with moving 7s - okayed with Sean
        cards.extend(removed_sevens);
        cards.shuffle(&mut thread_rng());
        // End dealing with sevens
        self.hands = [vec![], vec![]];
        for y in 0..11 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.hands[player].push(card);
            }
        }
        self.hands[0].sort_by(card_sorter);
        for y in 0..11 {
            for player in 0..2 {
                let card = self.hands[player][y];
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        hand_offset: y,
                        length: 11,
                        ..Default::default()
                    },
                );
            }
        }
        self.show_playable();
    }

    #[inline]
    fn new_change(&mut self) -> usize {
        self.changes.push(vec![]);
        self.changes.len() - 1
    }

    #[inline]
    fn add_change(&mut self, index: usize, change: Change) {
        if !self.no_changes {
            self.changes[index].push(change);
        }
    }

    fn show_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.changes.len() - 1;
        if self.current_player == 0 {
            let moves = self.get_moves();
            //moves.sort(); - used to end-to-end verify changes against Dart version
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
        let mut cards = self.hands[0].clone();
        cards.extend(self.exposed_straw_bottoms(0));
        cards.extend(self.straw_top[0].iter().flatten());
        //cards.sort_by_key(|c| c.id); - needed for verification against Dart engine
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

    fn exposed_straw_bottoms(&self, player: usize) -> HashSet<Card> {
        let mut exposed_cards: HashSet<Card> = HashSet::new();
        for (i, card) in self.straw_bottom[player].iter().cloned().enumerate() {
            if card.is_none() {
                continue;
            }
            let left_open: bool;
            let right_open: bool;
            if i == 0 {
                left_open = true;
            } else {
                left_open = self.straw_top[player][i - 1].is_none();
            }
            if i == 6 {
                right_open = true;
            } else {
                right_open = self.straw_top[player][i].is_none();
            }
            if left_open && right_open {
                exposed_cards.insert(card.unwrap());
            }
        }
        return exposed_cards;
    }

    fn get_moves(&self) -> Vec<i32> {
        if self.state == State::Discard {
            return self.hands[self.current_player]
                .iter()
                .filter(|c| c.value != 7) // can't discard 7s
                .map(|c| c.id)
                .collect();
        }
        // must follow
        let mut playable_cards = self.visible_straw(self.current_player);
        playable_cards.extend(self.hands[self.current_player].clone());

        if let Some(lead_suit) = self.lead_suit {
            let moves: Vec<i32> = playable_cards
                .iter()
                .filter(|c| c.suit == lead_suit)
                .map(|c| c.id)
                .collect();
            if !moves.is_empty() {
                return moves;
            }
        }
        return playable_cards.iter().map(|c| c.id).collect();
    }

    fn visible_straw(&self, player: usize) -> Vec<Card> {
        let mut visible: Vec<Card> = self.straw_top[player].iter().filter_map(|x| *x).collect();
        visible.extend(self.exposed_straw_bottoms(player));
        return visible;
    }

    fn hidden_straw(&self, player: usize) -> HashSet<Card> {
        let exposed_straw_bottoms = self.exposed_straw_bottoms(player);
        return self.straw_bottom[player]
            .iter()
            .filter_map(|x| *x)
            .filter(|x| !exposed_straw_bottoms.contains(x))
            .collect();
    }

    pub fn reveal_straw_bottoms(&mut self, player: usize) {
        if self.no_changes {
            return;
        }
        let index = self.changes.len() - 1;
        let exposed_straw_bottoms = self.exposed_straw_bottoms(player);
        let sorted_straw_bottoms: Vec<Card> = exposed_straw_bottoms.iter().cloned().collect();
        //sorted_straw_bottoms.sort_by_key(|c| c.id); - needed to verify against Dart engine
        self.changes[index].extend(sorted_straw_bottoms.iter().map(|c| Change {
            change_type: ChangeType::RevealCard,
            dest: Location::Hand,
            object_id: c.id,
            ..Default::default()
        }));
    }

    #[inline]
    pub fn reorder_hand(&mut self, player: usize) {
        if self.no_changes {
            return;
        }
        if self.changes.is_empty() {
            self.new_change();
        }
        let length = self.hands[self.current_player].len();
        let index = self.changes.len() - 1;
        self.changes[index].extend(self.hands[self.current_player].iter().enumerate().map(
            |(hand_offset, card)| Change {
                change_type: ChangeType::Reorder,
                dest: Location::ReorderHand,
                object_id: card.id,
                player,
                hand_offset,
                length,
                ..Default::default()
            },
        ));
    }

    pub fn apply_move(&mut self, action: &i32) {
        // reset per-hand scores after a move is made
        self.hand_scores = [0, 0];
        if !self.get_moves().contains(action) {
            for card in self.hands[self.current_player].iter() {
                println!("card: {:?}", card)
            }
            for card in self.hands[(self.current_player + 1) % 2].iter() {
                println!("card p2: {:?}", card)
            }
            println!("currentPlayer: {:?}", self.current_player);
            println!("moves: {:?}", self.get_moves());
            println!("move: {:?}", action);
            panic!("illegal move");
        }
        self.changes = vec![vec![]]; // card from player to table
        let card: &Card = ID_TO_CARD.get(&action).unwrap();
        match self.state {
            State::Discard => {
                self.hands[self.current_player].retain(|c| c.id != card.id);
                self.add_change(
                    0,
                    Change {
                        object_id: card.id,
                        change_type: ChangeType::Discard,
                        dest: Location::Discard,
                        ..Default::default()
                    },
                );
                self.reorder_hand(self.current_player);

                if self.hands.iter().all(|h| h.len() == 10) {
                    self.state = State::PlayCard;
                }
                self.current_player = (self.current_player + 1) % 2;
                self.show_playable();
                return;
            }
            State::PlayCard => {
                if let Some(index) =
                    self.straw_bottom[self.current_player]
                        .iter()
                        .position(|c| match c {
                            Some(c_inner) => c_inner.id == card.id,
                            None => false,
                        })
                {
                    // card played was from straw_bottom
                    self.straw_bottom[self.current_player][index] = None;
                } else if let Some(index) =
                    self.straw_top[self.current_player]
                        .iter()
                        .position(|c| match c {
                            Some(c_inner) => c_inner.id == card.id,
                            None => false,
                        })
                {
                    // card played was from straw_top
                    self.straw_top[self.current_player][index] = None;
                } else {
                    // card played was from hand
                    self.hands[self.current_player].retain(|c| c.id != card.id);
                }

                self.add_change(
                    0,
                    Change {
                        change_type: ChangeType::Play,
                        object_id: *action,
                        dest: Location::Play,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
                self.reorder_hand(self.current_player);
                self.current_trick[self.current_player] = Some(*card);

                if let Some(lead_suit) = self.lead_suit {
                    if card.suit != lead_suit {
                        // Player has revealed a void
                        self.voids[self.current_player].insert(lead_suit);
                    }
                }

                if self.lead_suit.is_none() {
                    self.lead_suit = Some(card.suit);
                }

                self.current_player = (self.current_player + 1) % 2;
                self.hide_playable();

                if self.current_trick.iter().flatten().count() == 2 {
                    // end trick

                    let trick_winner = get_winner(
                        self.lead_suit.unwrap(),
                        self.trump_card.unwrap(),
                        self.current_trick,
                    );
                    let winning_card = self.current_trick[trick_winner].unwrap();
                    self.tricks_taken[trick_winner] = self.tricks_taken[trick_winner] + 1;
                    // winner of the trick leads
                    self.current_player = trick_winner;
                    let index = self.new_change();
                    self.add_change(
                        index,
                        Change {
                            change_type: ChangeType::ShowWinningCard,
                            object_id: winning_card.id,
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

                    self.reveal_straw_bottoms(0);
                    self.reveal_straw_bottoms(1);

                    self.changes.extend([
                        vec![], // sevens
                        vec![], // trick to team
                    ]);

                    let seven_changes_index = self.changes.len() - 2;
                    let trick_to_team_index = self.changes.len() - 1;

                    for card in self.current_trick.clone().iter().flatten() {
                        if card.value == 7 {
                            self.captured_sevens[trick_winner].push(*card);
                            self.add_change(
                                seven_changes_index,
                                Change {
                                    change_type: ChangeType::CaptureSeven,
                                    object_id: card.id,
                                    dest: Location::SevensPile,
                                    hand_offset: self.captured_sevens[trick_winner]
                                        .iter()
                                        .position(|c| c.id == card.id)
                                        .unwrap(),
                                    player: trick_winner,
                                    ..Default::default()
                                },
                            );
                        } else {
                            self.add_change(
                                trick_to_team_index,
                                Change {
                                    change_type: ChangeType::TricksToWinner,
                                    object_id: card.id,
                                    dest: Location::TricksTaken,
                                    player: trick_winner,
                                    tricks_taken: self.tricks_taken[trick_winner],
                                    ..Default::default()
                                },
                            );
                        }
                    }

                    self.current_trick = [None; 2];
                    self.lead_suit = None;

                    let mut hand_winning_player: Option<usize> = None;

                    // player with >= 4 sevens wins the round
                    for player in 0..2 {
                        if self.captured_sevens[player].len() >= 4 {
                            hand_winning_player = Some(player);
                            break;
                        }
                    }

                    // player with >= 13 tricks loses

                    if hand_winning_player.is_none() {
                        let mut overall_hands: [Vec<Card>; 2] = [vec![], vec![]];
                        for player in 0..2 {
                            overall_hands[player].extend(self.hands[player].clone());
                            overall_hands[player]
                                .extend(self.straw_bottom[player].iter().flatten());
                            overall_hands[player].extend(self.straw_top[player].iter().flatten());
                        }

                        if overall_hands.iter().all(|h| h.is_empty()) {
                            // player that played last wins if there are no cards left
                            // and the other win conditions are not met
                            hand_winning_player = Some(self.current_player);
                        }
                        for player in 0..2 {
                            if self.tricks_taken[player] >= 13 {
                                // the other player won
                                hand_winning_player = Some((player + 1) % 2);
                            }
                        }
                        if let Some(hand_winning_player) = hand_winning_player {
                            let mut sevens: Vec<Card> = vec![];
                            for hand in self.hands.iter() {
                                sevens.extend(hand.iter().filter(|c| c.value == 7));
                            }
                            for pile in self.straw_top.iter() {
                                sevens.extend(pile.iter().flatten().filter(|c| c.value == 7));
                            }
                            for pile in self.straw_bottom.iter() {
                                sevens.extend(pile.iter().flatten().filter(|c| c.value == 7));
                            }
                            self.captured_sevens[hand_winning_player].extend(sevens.iter());
                            for seven in sevens.iter() {
                                self.add_change(
                                    seven_changes_index,
                                    Change {
                                        change_type: ChangeType::CaptureSeven,
                                        object_id: seven.id,
                                        dest: Location::SevensPile,
                                        hand_offset: self.captured_sevens[hand_winning_player]
                                            .iter()
                                            .position(|c| c.id == seven.id)
                                            .unwrap(),
                                        player: hand_winning_player,
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                    }

                    if let Some(hand_winning_player) = hand_winning_player {
                        let c7s = self.captured_sevens[hand_winning_player].clone();
                        let points = score_sevens(&c7s, &self.trump_card.unwrap());
                        self.scores[hand_winning_player] += points;
                        self.hand_scores[hand_winning_player] += points;
                        let index = self.new_change();
                        self.add_change(
                            index,
                            Change {
                                object_id: 0,
                                change_type: ChangeType::Score,
                                dest: Location::Score,
                                start_score: self.scores[hand_winning_player],
                                end_score: self.scores[hand_winning_player],
                                player: hand_winning_player,
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

                        let mut game_winner: Option<usize> = None;

                        for player in 0..2 {
                            if self.scores[player] >= 7 {
                                game_winner = Some(player);
                                self.winner = Some(player);
                            }
                        }

                        let index = self.new_change();

                        if game_winner.is_some() {
                            self.winner = game_winner;
                            self.add_change(
                                index,
                                Change {
                                    change_type: ChangeType::GameOver,
                                    object_id: 0,
                                    dest: Location::Deck,
                                    ..Default::default()
                                },
                            );
                            return;
                        } else {
                            self.add_change(
                                index,
                                Change {
                                    change_type: ChangeType::Shuffle,
                                    object_id: 0,
                                    dest: Location::Deck,
                                    ..Default::default()
                                },
                            );
                            self.deal();
                            return;
                        }
                    }
                }

                self.show_playable();
                return;
            }
        }
    }
}

impl ismcts::Game for Yokai2pGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();
        let mut remaining_cards: Vec<Card> = vec![];
        let mut hidden_straw_bottoms: [HashSet<Card>; 2] = [HashSet::new(), HashSet::new()];

        for player in 0..2 {
            if player != self.current_player {
                remaining_cards.extend(self.hands[player].iter());
            }

            hidden_straw_bottoms[player] =
                HashSet::from_iter(self.straw_bottom[player].iter().filter_map(|&x| x))
                    .difference(&self.exposed_straw_bottoms(player))
                    .cloned()
                    .collect();

            remaining_cards.extend(hidden_straw_bottoms[player].iter());
        }

        remaining_cards.shuffle(rng);

        for player in 0..2 {
            let original_hand_length: usize = self.hands[player].len();
            if player != self.current_player {
                let mut pc = extract_short_suited_cards(&remaining_cards, &self.voids[player]);
                self.hands[player] = vec![];
                pc.cards.shuffle(rng);
                for _ in 0..original_hand_length {
                    let card = pc.cards.pop().unwrap();
                    self.hands[player].push(card);
                }
                remaining_cards.extend(pc.leftovers);
                remaining_cards.extend(pc.cards);
            }
            assert!(original_hand_length == self.hands[player].len());
        }

        remaining_cards.shuffle(rng);
        for player in 0..2 {
            for i in 0..self.straw_bottom[player].len() {
                let card = self.straw_bottom[player][i];
                if !card.is_none() && hidden_straw_bottoms[player].contains(&card.unwrap()) {
                    self.straw_bottom[player][i] = remaining_cards.pop();
                }
            }
        }
        // FIXME: something isn't right
        //assert!(remaining_cards.is_empty());
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
        self.apply_move(mov);
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
            if self.hand_scores == [0, 0] {
                // the hand is not over
                None
            } else {
                let current_player_score = self.hand_scores[player] as f64;
                let other_player_score = self.hand_scores[(player + 1) % 2] as f64;
                if current_player_score > other_player_score {
                    Some(0.2 + ((current_player_score / 7.0) * 0.8))
                } else {
                    Some((1.0 - (other_player_score / 7.0)) * 0.2)
                }
            }
        }
    }
}

pub fn get_winner(lead_suit: Suit, trump_card: Card, trick: [Option<Card>; 2]) -> usize {
    if value_for_card(lead_suit, trump_card, trick[0].unwrap())
        > value_for_card(lead_suit, trump_card, trick[1].unwrap())
    {
        0
    } else {
        1
    }
}

pub fn value_for_card(lead_suit: Suit, trump_card: Card, card: Card) -> i32 {
    if card.value == 1 && card.suit == Suit::Green {
        return 1000;
    }
    if card.suit == lead_suit {
        return card.value + 50;
    }
    if card.suit == trump_card.suit {
        return card.value + 100;
    }
    return card.value;
}

pub fn seven_value(suit: &Suit) -> i32 {
    match suit {
        Suit::Green => 0,
        Suit::Purple => 0,
        Suit::Pink => 1,
        Suit::Yellow => 1,
        Suit::Black => 1,
        Suit::Red => 2,
        Suit::Blue => 2,
    }
}

pub fn score_sevens(sevens: &Vec<Card>, trump_card: &Card) -> i32 {
    sevens
        .iter()
        .filter(|&card| card.suit != trump_card.suit)
        .map(|card| seven_value(&card.suit))
        .sum()
}

pub fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

pub struct PossibleCards {
    cards: Vec<Card>,
    leftovers: Vec<Card>,
}

pub fn extract_short_suited_cards(
    remaining_cards: &Vec<Card>,
    voids: &HashSet<Suit>,
) -> PossibleCards {
    let mut leftovers: Vec<Card> = vec![];

    let mut possible_cards = remaining_cards.clone();

    for suit in voids {
        possible_cards.retain(|card| {
            let belongs_to_suit = card.suit == *suit;
            if belongs_to_suit {
                leftovers.push(*card);
            }
            !belongs_to_suit
        });
    }
    return PossibleCards {
        cards: possible_cards,
        leftovers,
    };
}

pub fn get_mcts_move(game: &Yokai2pGame, iterations: i32) -> i32 {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Yokai2pDartFormat {
    pub state: State,
    pub trump_card: Option<Card>,
    pub hands: [Vec<Card>; 2],
    pub changes: Vec<Vec<Change>>,
    pub current_trick: HashMap<i32, Card>,
    pub tricks_taken: HashMap<i32, i32>,
    pub lead_suit: Option<Suit>,
    pub scores: HashMap<i32, i32>,
    pub captured_sevens: [Vec<Card>; 2],
    pub straw_bottom: [Vec<Option<Card>>; 2],
    pub straw_top: [Vec<Option<Card>>; 2],
    pub current_player: usize,
    pub winner: Option<usize>,
    pub overall_winner: Option<usize>,
    pub lead_player: usize,
    pub round: i32,
}

impl Yokai2pDartFormat {
    pub fn to_rust(&self) -> Yokai2pGame {
        let trick1: Option<Card> = self.current_trick.get(&0).cloned();
        let trick2: Option<Card> = self.current_trick.get(&1).cloned();
        let mut changes = self.changes.clone();
        // remove empty changes
        changes.retain(|x| !x.is_empty());
        Yokai2pGame {
            state: self.state.clone(),
            trump_card: self.trump_card.clone(),
            hands: self.hands.clone(),
            changes,
            current_trick: [trick1, trick2],
            tricks_taken: [
                *self.tricks_taken.get(&0).unwrap_or(&0),
                *self.tricks_taken.get(&1).unwrap_or(&0),
            ],
            lead_suit: self.lead_suit.clone(),
            scores: [
                *self.scores.get(&0).unwrap_or(&0),
                *self.scores.get(&1).unwrap_or(&0),
            ],
            hand_scores: [0, 0],
            voids: [HashSet::new(), HashSet::new()],
            captured_sevens: self.captured_sevens.clone(),
            straw_bottom: self.straw_bottom.clone(),
            straw_top: self.straw_top.clone(),
            current_player: self.current_player,
            winner: self.winner,
            overall_winner: self.overall_winner,
            lead_player: self.lead_player,
            round: self.round,
            no_changes: false,
        }
    }
}
