/*
Game: Otter
Designer: Dylan Coyle
BoardGameGeek: https://boardgamegeek.com/boardgame/425532/otter
*/

use std::cmp::{max, min};

use enum_iterator::{all, Sequence};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

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
    Finstin = 0,
    Stardon = 1,
    Clawson = 2,
    Shelldon = 3,
    Todd = 4,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    id: i32,
    pub suit: Suit,
    value: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum HeadType {
    Higher = 100,
    Lower = 101,
    Near = 102,
    Far = 103,
    Odd = 104,
    Even = 105,
    Inside = 106,
    Outside = 107,
    Shallow = 108,
    Deep = 109,
}

impl HeadType {
    pub fn name(&self) -> String {
        match self {
            HeadType::Higher => "Higher".to_string(),
            HeadType::Lower => "Lower".to_string(),
            HeadType::Odd => "Odd".to_string(),
            HeadType::Even => "Even".to_string(),
            HeadType::Deep => "Deep".to_string(),
            HeadType::Shallow => "Shallow".to_string(),
            HeadType::Near => "Near".to_string(),
            HeadType::Far => "Far".to_string(),
            HeadType::Inside => "Inside".to_string(),
            HeadType::Outside => "Outside".to_string(),
        }
    }

    pub fn legal_play(
        &self,
        played_card: &Card,
        current_card: Card,
        other_cards: Vec<Card>,
    ) -> bool {
        match self {
            HeadType::Higher => played_card.value > current_card.value,
            HeadType::Lower => played_card.value < current_card.value,
            HeadType::Odd => played_card.value % 2 == 1,
            HeadType::Even => played_card.value % 2 == 0,
            HeadType::Deep => played_card.value + other_cards[0].value + other_cards[1].value > 20,
            HeadType::Shallow => {
                played_card.value + other_cards[0].value + other_cards[1].value < 20
            }
            HeadType::Near => {
                played_card.value >= current_card.value - 2
                    && played_card.value <= current_card.value + 2
            }
            HeadType::Far => {
                played_card.value <= current_card.value - 3
                    || played_card.value >= current_card.value + 3
            }
            HeadType::Inside => {
                played_card.value > min(other_cards[0].value, other_cards[1].value)
                    && played_card.value < max(other_cards[0].value, other_cards[1].value)
            }
            HeadType::Outside => {
                played_card.value < min(other_cards[0].value, other_cards[1].value)
                    || played_card.value > max(other_cards[0].value, other_cards[1].value)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct HeadCard {
    id: i32,
    front: HeadType,
    back: HeadType,
}

impl HeadCard {
    pub fn flip(&mut self) {
        (self.front, self.back) = (self.back, self.front);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum State {
    SelectTummyOrHead,
    SelectTummy,
    SelectHead,
    GameOverWin,
    GameOverLose,
}

// Undo functionality constants
const UNDO: i32 = -2;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Hand,
    TummyCards,
    HeadCards,
    Piles,
    Score,
    Message,
    LuckyStones,
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
    FlipHead,
    SwapHeads,
    MoveToTummy,
    UpdateLuckyStones,
    BurnCard,
    UpdateStackCount,
    OptionalPause,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    pub object_id: i32,
    pub dest: Location,
    pub startscore: i32,
    pub end_score: i32,
    pub offset: usize,
    pub player: usize,
    pub length: usize,
    pub highlight: bool,
    pub xout: bool,
    pub selected: bool,
    pub message: Option<String>,
    pub card: Option<Card>,
    pub head_card: Option<HeadCard>,
}

impl Default for Change {
    fn default() -> Self {
        Self {
            change_type: ChangeType::default(),
            object_id: 0,
            dest: Location::default(),
            startscore: 0,
            end_score: 0,
            offset: 0,
            player: 0,
            length: 0,
            highlight: false,
            xout: false,
            selected: false,
            message: None,
            card: None,
            head_card: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct OtterGame {
    pub head_cards: [HeadCard; 3],
    pub tummy_cards: [Card; 3],
    pub piles: [Vec<Card>; 4],
    last_played_head_card_index: usize,
    last_played_pile_index: usize,
    pub state: State,
    selected_pile_offset: Option<usize>,
    selected_head_offset: Option<usize>,
    pub lucky_stones: i32,
    pub changes: Vec<Vec<Change>>,
    pub no_changes: bool,
    pub score: i32,
    pub winner: Option<bool>, // true = win, false = lose
}

impl OtterGame {
    pub fn new() -> Self {
        let mut tummy_deck = OtterGame::tummy_deck();

        let start_tummy_cards: [Card; 3] = tummy_deck
            .drain(..3)
            .collect::<Vec<_>>()
            .try_into()
            .expect("wrong length");

        let mut game = OtterGame {
            head_cards: OtterGame::head_deck().try_into().expect("wrong length"),
            piles: [
                tummy_deck.drain(..15).collect::<Vec<_>>(),
                tummy_deck.drain(..15).collect::<Vec<_>>(),
                tummy_deck.drain(..16).collect::<Vec<_>>(),
                tummy_deck,
            ],
            tummy_cards: start_tummy_cards,
            last_played_pile_index: 100, // intentionally invalid index - any pile can be played at the start
            last_played_head_card_index: 100, // intentionally invalid index - any head can be played to at the start
            state: State::SelectTummyOrHead,
            selected_head_offset: None,
            selected_pile_offset: None,
            lucky_stones: 4,
            changes: Vec::new(),
            no_changes: false,
            score: 0,
            winner: None,
        };

        let card_ids_to_flip: Vec<_> = game
            .head_cards
            .iter_mut()
            .filter_map(|card| {
                if rand::random::<bool>() {
                    Some(card.id)
                } else {
                    None
                }
            })
            .collect();

        for card_id in &card_ids_to_flip {
            game.flip_head_card_animation(*card_id);
        }

        for card in game.head_cards.iter_mut() {
            if card_ids_to_flip.contains(&card.id) {
                card.flip();
            }
        }

        // Generate initial setup animation
        game.generate_setup_animation();
        game.generate_playable_animations();
        game
    }

    pub fn find_head_offset(&self, card_id: i32) -> Option<usize> {
        return self.head_cards.iter().position(|c| c.id == card_id);
    }

    pub fn find_pile_offset(&self, card_id: i32) -> Option<usize> {
        let offset = self.piles.iter().position(|p| {
            p.last()
                .unwrap_or(&Card {
                    id: -100,
                    ..Default::default()
                })
                .id
                == card_id
        });
        return offset;
    }

    pub fn apply_move(&mut self, card_id: i32) {
        if !self.get_moves().contains(&card_id) {
            panic!("invalid move");
        }

        self.changes.clear(); // Clear previous animations

        // Clear old plays out

        let mut changes = Vec::new();
        changes.push(Change {
            change_type: ChangeType::HidePlayable,
            object_id: -1,
            highlight: false,
            xout: false,
            ..Default::default()
        });
        self.changes.push(changes);

        // Handle undo move
        if card_id == UNDO {
            self.selected_head_offset = None;
            self.selected_pile_offset = None;
            self.state = State::SelectTummyOrHead;
            self.generate_playable_animations();
            return;
        }

        match self.state {
            State::GameOverLose => panic!("moves can't be made when the game is over"),
            State::GameOverWin => panic!("moves can't be made when the game is over"),
            State::SelectTummyOrHead => {
                if card_id >= 100 {
                    self.selected_head_offset = self.find_head_offset(card_id);
                    self.selected_pile_offset = None;
                    self.state = State::SelectHead;
                } else {
                    self.selected_head_offset = None;
                    self.selected_pile_offset = self.find_pile_offset(card_id);
                    self.state = State::SelectTummy;
                }
            }
            State::SelectHead => {
                let first_head_card_offset = self.selected_head_offset.unwrap();
                let second_head_card_offset = self.find_head_offset(card_id).unwrap();
                let mut first_head_card = self.head_cards[first_head_card_offset];
                let second_head_card = self.head_cards[second_head_card_offset];

                if first_head_card.id == second_head_card.id {
                    first_head_card.flip();
                    self.head_cards[first_head_card_offset] = first_head_card;
                    self.flip_head_card_animation(first_head_card.id);
                } else {
                    self.head_cards
                        .swap(first_head_card_offset, second_head_card_offset);
                }

                self.generate_show_heads_animation();

                self.lucky_stones -= 1;
                self.generate_update_lucky_stones_animation();
                self.selected_head_offset = None;
                self.selected_pile_offset = None;
                self.state = State::SelectTummyOrHead;

                if self.check_end() {
                    return;
                }
            }
            State::SelectTummy => {
                let tummy_offset = self
                    .tummy_cards
                    .iter()
                    .position(|c| c.id == card_id)
                    .unwrap();
                let card = self.piles[self.selected_pile_offset.unwrap()]
                    .pop()
                    .unwrap();

                // Update stack counts after moving a card from pile
                self.generate_stack_count_updates();

                self.generate_move_to_tummy_animation(
                    card,
                    self.selected_pile_offset.unwrap(),
                    tummy_offset,
                );

                self.tummy_cards[tummy_offset] = card;
                self.last_played_head_card_index = tummy_offset;
                self.last_played_pile_index = self.selected_pile_offset.unwrap();
                self.selected_head_offset = None;
                self.selected_pile_offset = None;
                self.state = State::SelectTummyOrHead;

                if self.check_end() {
                    return;
                }
            }
        }

        self.generate_playable_animations();
    }

    fn check_end(&mut self) -> bool {
        if self.piles.iter().all(|p| p.is_empty()) {
            self.state = State::GameOverWin;
            self.winner = Some(true);
            self.generate_game_over_animation(true);
            return true;
        }
        if self.get_moves().is_empty() {
            // No moves
            self.state = State::GameOverLose;
            self.winner = Some(false);
            self.generate_game_over_animation(false);
            return true;
        }
        return false;
    }

    pub fn get_moves(&self) -> Vec<i32> {
        let mut moves = match self.state {
            State::SelectTummyOrHead => self.get_pile_or_head_moves(),
            State::SelectHead => self.head_cards.map(|c| c.id).to_vec(),
            State::SelectTummy => self.get_tummy_moves(),
            State::GameOverLose => vec![],
            State::GameOverWin => vec![],
        };

        // Add undo move when there's a selection
        if matches!(self.state, State::SelectHead | State::SelectTummy) {
            moves.push(UNDO);
        }

        moves
    }

    fn get_pile_or_head_moves(&self) -> Vec<i32> {
        let mut moves = if self.lucky_stones > 0 {
            self.head_cards.map(|c| c.id).to_vec()
        } else {
            vec![]
        };
        for (pile_index, pile) in self.piles.iter().enumerate() {
            if pile.is_empty() || pile_index == self.last_played_pile_index {
                continue;
            }

            let top_card = pile.last().unwrap();

            for (head_index, head_card) in self.head_cards.iter().enumerate() {
                if head_index == self.last_played_head_card_index {
                    continue;
                }

                let tummy_card = self.tummy_cards[head_index]; // tummy under head
                let other_cards = self
                    .tummy_cards
                    .iter()
                    .enumerate()
                    .filter_map(|(i, tummy_card)| {
                        if i != head_index {
                            Some(tummy_card)
                        } else {
                            None
                        }
                    })
                    .copied()
                    .collect::<Vec<_>>();

                if head_card
                    .front
                    .legal_play(top_card, tummy_card, other_cards)
                {
                    moves.push(top_card.id);
                    break; // We only need to know this top card is playable somewhere
                }
            }
        }
        return moves;
    }

    fn get_tummy_moves(&self) -> Vec<i32> {
        let selected_card = self.piles[self.selected_pile_offset.unwrap()]
            .last()
            .unwrap();
        let mut moves = vec![];
        for (index, head_card) in self.head_cards.iter().enumerate() {
            if index == self.last_played_head_card_index {
                continue;
            }
            let other_cards: Vec<Card> = self
                .tummy_cards
                .iter()
                .enumerate()
                .filter_map(|(i, &c)| if i != index { Some(c) } else { None })
                .collect();
            if head_card
                .front
                .legal_play(selected_card, self.tummy_cards[index], other_cards)
            {
                moves.push(self.tummy_cards[index].id);
            }
        }
        return moves;
    }

    pub fn head_deck() -> Vec<HeadCard> {
        let mut head_cards = vec![
            HeadCard {
                id: 100,
                front: HeadType::Higher,
                back: HeadType::Lower,
            },
            HeadCard {
                id: 101,
                front: HeadType::Near,
                back: HeadType::Far,
            },
            HeadCard {
                id: 102,
                front: HeadType::Odd,
                back: HeadType::Even,
            },
            HeadCard {
                id: 103,
                front: HeadType::Inside,
                back: HeadType::Outside,
            },
            HeadCard {
                id: 104,
                front: HeadType::Shallow,
                back: HeadType::Deep,
            },
        ];

        head_cards.shuffle(&mut thread_rng());

        return head_cards[..3].to_vec();
    }

    pub fn tummy_deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        for suit in all::<Suit>() {
            for value in 1..=13 {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
    }

    // Animation generation methods
    fn generate_setup_animation(&mut self) {
        if self.no_changes {
            return;
        }

        let mut setup_changes = Vec::new();

        // Show initial tummy cards
        for (i, card) in self.tummy_cards.iter().enumerate() {
            setup_changes.push(Change {
                change_type: ChangeType::Deal,
                object_id: card.id,
                dest: Location::TummyCards,
                offset: i,
                player: 0,
                length: 3,
                card: Some(*card),
                ..Default::default()
            });
        }

        // Show head cards
        for (i, head_card) in self.head_cards.iter().enumerate() {
            setup_changes.push(Change {
                change_type: ChangeType::Deal,
                object_id: head_card.id,
                dest: Location::HeadCards,
                offset: i,
                player: 0,
                length: 3,
                head_card: Some(*head_card),
                ..Default::default()
            });
        }

        // Show all pile cards (deal all cards to their pile positions)
        for (pile_idx, pile) in self.piles.iter().enumerate() {
            for (card_idx, card) in pile.iter().enumerate() {
                setup_changes.push(Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id,
                    dest: Location::Piles,
                    offset: pile_idx,
                    player: 0,
                    length: pile.len(),
                    card: Some(*card),
                    ..Default::default()
                });
            }
        }

        // Show lucky stones
        setup_changes.push(Change {
            change_type: ChangeType::UpdateLuckyStones,
            object_id: -1,
            dest: Location::LuckyStones,
            length: self.lucky_stones as usize,
            ..Default::default()
        });

        self.changes.push(setup_changes);

        // Update stack counts
        self.generate_stack_count_updates();
    }

    fn animate_top_stack(&mut self) {
        if self.no_changes {
            return;
        }

        // We don't know the new card at the top of the deck until after the played card is moved
        if self.changes.is_empty() {
            self.changes.push(vec![]);
        }

        for pile in self.piles.iter() {
            if let Some(card) = pile.last() {
                self.changes[0].push(Change {
                    change_type: ChangeType::ShowPlayable,
                    object_id: card.id,
                    dest: Location::Piles,
                    highlight: false,
                    ..Default::default()
                });
            }
        }
    }

    fn generate_playable_animations(&mut self) {
        if self.no_changes {
            return;
        }

        self.animate_top_stack();

        let mut changes = Vec::new();

        if let Some(offset) = self.selected_pile_offset {
            changes.push(Change {
                change_type: ChangeType::ShowPlayable,
                object_id: self.piles[offset].last().unwrap().id,
                selected: true,
                ..Default::default()
            });
        }

        if let Some(offset) = self.selected_head_offset {
            changes.push(Change {
                change_type: ChangeType::ShowPlayable,
                object_id: self.head_cards[offset].id,
                selected: true,
                ..Default::default()
            });
        }

        for action in self.get_moves() {
            changes.push(Change {
                change_type: ChangeType::ShowPlayable,
                object_id: action,
                highlight: true,
                ..Default::default()
            });
        }

        // Cross out last played pile
        if let Some(pile) = self.piles.get(self.last_played_pile_index) {
            if let Some(card) = pile.last() {
                changes.push(Change {
                    change_type: ChangeType::ShowPlayable,
                    object_id: card.id,
                    highlight: false,
                    xout: true,
                    ..Default::default()
                });
            }
        }

        // Cross out last played head
        if let Some(card) = self.tummy_cards.get(self.last_played_head_card_index) {
            changes.push(Change {
                change_type: ChangeType::ShowPlayable,
                object_id: card.id,
                highlight: false,
                xout: true,
                ..Default::default()
            });
        }

        self.changes.push(changes);
    }

    fn flip_head_card_animation(&mut self, card_id: i32) {
        if self.no_changes {
            return;
        }

        let mut changes = Vec::new();

        changes.push(Change {
            change_type: ChangeType::FlipHead,
            object_id: card_id,
            dest: Location::HeadCards,
            player: 0,
            ..Default::default()
        });

        self.changes.push(changes);
    }

    fn generate_show_heads_animation(&mut self) {
        if self.no_changes {
            return;
        }

        let mut changes = Vec::new();

        for (idx, card) in self.head_cards.iter().enumerate() {
            changes.push(Change {
                change_type: ChangeType::Deal,
                object_id: card.id,
                dest: Location::HeadCards,
                offset: idx,
                player: 0,
                length: 3,
                ..Default::default()
            })
        }

        self.changes.push(changes);
    }

    fn generate_move_to_tummy_animation(&mut self, card: Card, pile_idx: usize, tummy_idx: usize) {
        if self.no_changes {
            return;
        }

        let changes = vec![Change {
            change_type: ChangeType::MoveToTummy,
            object_id: card.id,
            dest: Location::TummyCards,
            offset: tummy_idx,
            card: Some(card),
            ..Default::default()
        }];
        self.changes.push(changes);
    }

    fn generate_update_lucky_stones_animation(&mut self) {
        if self.no_changes {
            return;
        }

        if self.changes.is_empty() {
            self.changes.push(vec![]);
        }

        let offset = self.changes.len() - 1;
        self.changes[offset].push(Change {
            change_type: ChangeType::UpdateLuckyStones,
            object_id: -1,
            dest: Location::LuckyStones,
            length: self.lucky_stones as usize,
            ..Default::default()
        });
    }

    fn generate_game_over_animation(&mut self, won: bool) {
        if self.no_changes {
            return;
        }

        self.generate_playable_animations();

        let message = if won {
            "You Win! All piles cleared!".to_string()
        } else {
            "Game Over! No more valid moves.".to_string()
        };

        let score: i32 = self.piles.iter().map(|pile| pile.len()).sum::<usize>() as i32;

        let changes = vec![
            Change {
                change_type: ChangeType::Message,
                object_id: -1,
                dest: Location::Message,
                message: Some(message),
                ..Default::default()
            },
            Change {
                change_type: ChangeType::OptionalPause,
                object_id: -1,
                dest: Location::Score,
                ..Default::default()
            },
            Change {
                change_type: ChangeType::GameOver,
                object_id: -1,
                dest: Location::Score,
                end_score: score,
                ..Default::default()
            },
        ];
        self.changes.push(changes);
    }

    fn generate_stack_count_updates(&mut self) {
        if self.no_changes {
            return;
        }

        let mut stack_changes = Vec::new();

        // Update pile stack counts
        for (pile_index, pile) in self.piles.iter().enumerate() {
            stack_changes.push(Change {
                change_type: ChangeType::UpdateStackCount,
                object_id: pile_index as i32,
                dest: Location::Piles,
                offset: pile_index,
                length: pile.len(),
                end_score: pile.len() as i32,
                ..Default::default()
            });
        }

        if !stack_changes.is_empty() {
            self.changes.push(stack_changes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(id: i32, value: i32, suit: Suit) -> Card {
        Card { id, value, suit }
    }

    fn make_head_card(id: i32, front: HeadType, back: HeadType) -> HeadCard {
        HeadCard { id, front, back }
    }

    #[test]
    fn test_legal_play_higher() {
        let top_card = make_card(1, 5, Suit::Clawson);
        let tummy_card = make_card(2, 3, Suit::Clawson);
        let head = HeadType::Higher;
        let legal = head.legal_play(
            &top_card,
            tummy_card,
            vec![
                make_card(3, 4, Suit::Clawson),
                make_card(4, 2, Suit::Clawson),
            ],
        );
        assert!(legal);
    }

    #[test]
    fn test_legal_play_odd_even() {
        let odd = HeadType::Odd;
        let even = HeadType::Even;
        let card = make_card(1, 7, Suit::Finstin);
        assert!(odd.legal_play(&card, card, vec![]));
        assert!(!even.legal_play(&card, card, vec![]));
    }

    #[test]
    fn test_deep_shallow_split() {
        let deep = HeadType::Deep;
        let shallow = HeadType::Shallow;
        let top = make_card(1, 10, Suit::Todd);
        let o1 = make_card(2, 6, Suit::Todd);
        let o2 = make_card(3, 6, Suit::Todd);
        assert!(deep.legal_play(&top, o1, vec![o1, o2]));
        assert!(!shallow.legal_play(&top, o1, vec![o1, o2]));
    }

    #[test]
    fn test_near_far_logic() {
        let current = make_card(1, 5, Suit::Finstin);
        let near = HeadType::Near;
        let far = HeadType::Far;
        assert!(near.legal_play(&make_card(2, 4, Suit::Todd), current, vec![]));
        assert!(!near.legal_play(&make_card(2, 8, Suit::Todd), current, vec![]));
        assert!(far.legal_play(&make_card(2, 8, Suit::Todd), current, vec![]));
    }

    #[test]
    fn test_new_game_inits_properly() {
        let game = OtterGame::new();
        assert_eq!(game.head_cards.len(), 3);
        assert_eq!(game.tummy_cards.len(), 3);
        assert_eq!(game.piles.len(), 4);
        assert_eq!(game.lucky_stones, 4);
        assert_eq!(game.state, State::SelectTummyOrHead);
    }

    #[test]
    fn test_head_flip_changes_front_and_back() {
        let mut card = HeadCard {
            id: 200,
            front: HeadType::Odd,
            back: HeadType::Even,
        };
        card.flip();
        assert_eq!(card.front, HeadType::Even);
        assert_eq!(card.back, HeadType::Odd);
    }

    #[test]
    fn test_apply_head_swap_or_flip() {
        let mut game = OtterGame::new();
        let h0 = game.head_cards[0].id;
        game.apply_move(h0); // select head
        game.apply_move(h0); // flip head (same head clicked)
        assert_eq!(game.state, State::SelectTummyOrHead);
        assert_eq!(game.lucky_stones, 3);
    }

    #[test]
    fn test_game_over_when_no_moves_and_piles_empty() {
        let mut game = OtterGame::new();
        game.piles.iter_mut().for_each(|pile| pile.clear());
        game.lucky_stones = 0;
        game.check_end();
        assert_eq!(game.state, State::GameOverWin);
    }

    #[test]
    fn test_game_over_when_no_moves_but_piles_not_empty() {
        let mut game = OtterGame::new();
        game.piles[0] = vec![make_card(99, 13, Suit::Todd)];
        game.piles[1] = vec![];
        game.piles[2] = vec![];
        game.piles[3] = vec![];
        game.tummy_cards = [make_card(1, 1, Suit::Todd); 3];
        game.head_cards = [
            make_head_card(100, HeadType::Lower, HeadType::Higher),
            make_head_card(101, HeadType::Lower, HeadType::Higher),
            make_head_card(102, HeadType::Lower, HeadType::Higher),
        ];
        game.lucky_stones = 0;
        game.check_end();
        assert_eq!(game.state, State::GameOverLose);
    }

    #[test]
    fn test_get_tummy_moves() {
        let mut game = OtterGame::new();

        // Manually set the state to SelectTummy and configure tummy/head/pile data
        game.state = State::SelectTummy;
        game.selected_pile_offset = Some(0);
        game.last_played_head_card_index = 2; // Ignore this index for playability

        // Setup pile: top card = 10
        game.piles[0] = vec![make_card(99, 10, Suit::Todd)];

        // Tummy cards under each head
        game.tummy_cards = [
            make_card(1, 5, Suit::Todd),  // not valid for Lower
            make_card(2, 12, Suit::Todd), // valid for Lower
            make_card(3, 11, Suit::Todd), // should be ignored
        ];

        // Matching head cards
        game.head_cards = [
            make_head_card(100, HeadType::Lower, HeadType::Higher), // 5 < 10 → valid
            make_head_card(101, HeadType::Lower, HeadType::Higher), // 12 > 10 → not valid
            make_head_card(102, HeadType::Lower, HeadType::Higher), // ignored due to last_played_head_card_index
        ];

        let tummy_moves = game.get_moves();
        assert_eq!(tummy_moves, vec![2, -2]); // Card 2 is legal play, plus undo option
    }
}
