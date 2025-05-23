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
    Todd = 3,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct OtterGame {
    head_cards: [HeadCard; 3],
    tummy_cards: [Card; 3],
    piles: [Vec<Card>; 4],
    last_played_head_card_index: usize,
    last_played_pile_index: usize,
    pub state: State,
    selected_pile_offset: Option<usize>,
    selected_head_offset: Option<usize>,
    pub lucky_stones: i32,
}

impl OtterGame {
    pub fn new() -> Self {
        let mut tummy_deck = OtterGame::tummy_deck();

        let start_tummy_cards: [Card; 3] = tummy_deck
            .drain(..3)
            .collect::<Vec<_>>()
            .try_into()
            .expect("wrong length");

        return OtterGame {
            head_cards: OtterGame::head_deck().try_into().expect("wrong length"),
            piles: [
                tummy_deck.drain(..15).collect::<Vec<_>>(),
                tummy_deck.drain(..15).collect::<Vec<_>>(),
                tummy_deck.drain(..16).collect::<Vec<_>>(),
                tummy_deck,
            ],
            tummy_cards: start_tummy_cards,
            last_played_pile_index: 100, // intentionally invalid index
            last_played_head_card_index: 100, // intentionally invalid index
            state: State::SelectTummyOrHead,
            selected_head_offset: None,
            selected_pile_offset: None,
            lucky_stones: 4,
        };
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
                } else {
                    self.head_cards
                        .swap(first_head_card_offset, second_head_card_offset);
                }
                self.lucky_stones -= 1;
                self.selected_head_offset = None;
                self.selected_pile_offset = None;
                self.state = State::SelectTummyOrHead;
                self.check_end();
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
                self.tummy_cards[tummy_offset] = card;
                self.last_played_head_card_index = tummy_offset;
                self.last_played_pile_index = self.selected_pile_offset.unwrap();
                self.selected_head_offset = None;
                self.selected_pile_offset = None;
                self.state = State::SelectTummyOrHead;
                self.check_end();
            }
        }
        return;
    }

    fn check_end(&mut self) {
        if self.get_moves().is_empty() {
            // No moves
            self.state = if self.piles.iter().all(|p| p.is_empty()) {
                State::GameOverWin
            } else {
                State::GameOverLose
            }
        }
    }

    pub fn get_moves(&self) -> Vec<i32> {
        match self.state {
            State::SelectTummyOrHead => self.get_pile_or_head_moves(),
            State::SelectHead => self.head_cards.map(|c| c.id).to_vec(),
            State::SelectTummy => self.get_tummy_moves(),
            State::GameOverLose => vec![],
            State::GameOverWin => vec![],
        }
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
                front: HeadType::Lower,
                back: HeadType::Higher,
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

        for card in head_cards.iter_mut() {
            // 50% chance of flipping
            if rand::random::<bool>() {
                card.flip();
            }
        }

        return head_cards[..3].to_vec();
    }

    pub fn tummy_deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        for suit in all::<Suit>() {
            for value in 0..=13 {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
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
        assert_eq!(tummy_moves, vec![2]); // Only card 1 should be a legal play
    }
}
