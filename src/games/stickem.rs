/*
Game: Stick 'Em (Sticheln)
Designer:  Klaus Palesch
BoardGameGeek: https://boardgamegeek.com/boardgame/354/stick-em
*/

use std::cmp::{max, min};

use enum_iterator::{all, Sequence};
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::{thread_rng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

const PLAYER_COUNT: usize = 4;

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
    Red = 0,
    Yellow = 1,
    Green = 2,
    Blue = 3,
    Purple = 4,
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
    SelectPainColor,
    Play,
    GameOver,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    PainColor,
    Hand,
    Score,
    Message,
    Play,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
}

pub struct TrickResult {
    // In the rare case that all cards played are 0 they are not scored and the lead player leads again
    score_hand: bool,
    winning_player: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub struct StickEmGame {
    pub hands: [Vec<Card>; PLAYER_COUNT],
    pub winning_player: Option<usize>,
    pub pain_cards: [Option<Card>; PLAYER_COUNT],
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
}

impl StickEmGame {
    pub fn new() -> Self {
        let mut game = StickEmGame {
            ..Default::default()
        };
        // Randomly select a start player each game
        game.dealer = thread_rng().gen_range(0..=PLAYER_COUNT);
        game.deal();
        game
    }

    fn deal(&mut self) {
        let mut deck = StickEmGame::deck();
        self.hands = [
            deck.drain(..15).collect::<Vec<_>>(),
            deck.drain(..15).collect::<Vec<_>>(),
            deck.drain(..15).collect::<Vec<_>>(),
            deck,
        ];
        self.round += 1;
    }

    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::new();
        let mut id = 0;

        for suit in all::<Suit>() {
            // Four players: 0-11 in red, yellow, green, blue, and purple
            for value in 0..=11 {
                deck.push(Card { id, value, suit });
                id += 1;
            }
        }

        deck.shuffle(&mut thread_rng());

        return deck;
    }

    pub fn get_moves(&self) -> Vec<i32> {
        // Any card can be played at any time
        return self.hands[self.current_player]
            .iter()
            .map(|c| c.id)
            .collect();
    }

    pub fn apply_move(&mut self, card_id: i32) {
        if !self.get_moves().contains(&card_id) {
            panic!("invalid move");
        }

        match self.state {
            State::GameOver => panic!("Cannot play when the game is over"),
            State::SelectPainColor => self.select_pain_color(card_id),
            State::Play => self.play(card_id),
        }
        // TODO: show playable
    }

    pub fn select_pain_color(&mut self, card_id: i32) {
        let card = self.pop_card(card_id);
        self.pain_cards[self.current_player] = Some(card);
        // TODO: Animate pain card selection
    }

    pub fn play(&mut self, card_id: i32) {
        let card = self.pop_card(card_id);
        self.current_hand[self.current_player] = Some(card);
        // TODO: Animate played cards
        // TODO: Check for the end of the hand and the end of the game
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
    ) -> TrickResult {
        if current_hand.iter().all(|c| c.unwrap().value == 0) {
            // In the rare case that all cards played are 0 they are not scored
            // and the lead player leads again
            return TrickResult {
                score_hand: false,
                winning_player: lead_player,
            };
        }

        let mut winning_player = 0;
        let mut winning_card = current_hand[lead_player].unwrap();
        let lead_suit = winning_card.suit;
        for i in 0..PLAYER_COUNT {
            let current_player = (lead_player + i) % PLAYER_COUNT;
            let card = current_hand[current_player].unwrap();

            if card.value == 0 {
                // Zeroes can never win tricks
                continue;
            }

            let card_wins = match (card.suit == lead_suit, winning_card.suit == lead_suit) {
                // Card suit is lead suit and winning card is still in lead suit
                (true, true) => card.value > winning_card.value,
                // Card suit is lead suit but another suit is already winning
                (true, false) => false,
                // Card suit is not lead suit and winning card is in lead suit
                (false, true) => true, // Any non-lead suit card will beat any
                // Card suit is not lead suit and winning card is not lead suit
                (false, false) => card.value > winning_card.value, // Highest value wins (ties go to earlier plays)
            };

            if card_wins {
                winning_player = current_player;
                winning_card = card;
            }
        }

        TrickResult {
            score_hand: true,
            winning_player,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let game = StickEmGame::new();
        assert!(
            game.hands.iter().all(|h| h.len() == 15),
            "Every player should have 15 cards in their hand"
        );
        assert_eq!(
            game.state,
            State::SelectPainColor,
            "Every player selects a pain color first"
        );
        assert_eq!(game.round, 1, "The game starts in round 1");
    }

    struct TrickWinnerScenario {
        lead_player: usize,
        current_hand: [Option<Card>; 4],
        expected_winning_player: usize,
        expected_score_hand: bool,
        name: String,
    }

    #[test]
    fn test_trick_winner() {
        let scenarios = [
            TrickWinnerScenario {
                name: "All zeroes should skip scoring and keep lead player".to_string(),
                lead_player: 2,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 0,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Green,
                        value: 0,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Yellow,
                        value: 0,
                    }),
                ],
                expected_winning_player: 2,
                expected_score_hand: false,
            },
            TrickWinnerScenario {
                name: "Only non-lead suit should win".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 1,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Purple,
                        value: 1,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Purple,
                        value: 2,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "All lead suit - highest value wins".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Blue,
                        value: 3,
                    }), // lead
                    Some(Card {
                        id: 1,
                        suit: Suit::Blue,
                        value: 10,
                    }), // winner
                    Some(Card {
                        id: 2,
                        suit: Suit::Blue,
                        value: 7,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 5,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Mix of suits - highest off-suit wins".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Yellow,
                        value: 9,
                    }), // lead
                    Some(Card {
                        id: 1,
                        suit: Suit::Green,
                        value: 5,
                    }),
                    Some(Card {
                        id: 2,
                        suit: Suit::Red,
                        value: 6,
                    }), // winner (highest off-suit)
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 4,
                    }),
                ],
                expected_winning_player: 2,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Tie in off-suit - first played wins".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Green,
                        value: 11,
                    }), // lead
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 5,
                    }), // winner (first of tied highest)
                    Some(Card {
                        id: 2,
                        suit: Suit::Yellow,
                        value: 5,
                    }), // same value but played later
                    Some(Card {
                        id: 3,
                        suit: Suit::Green,
                        value: 8,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Zeroes ignored - off-suit beats lead suit".to_string(),
                lead_player: 2,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Blue,
                        value: 7,
                    }), // winner (only off-suit)
                    Some(Card {
                        id: 2,
                        suit: Suit::Green,
                        value: 11,
                    }), // lead
                    Some(Card {
                        id: 3,
                        suit: Suit::Green,
                        value: 0,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "High lead card loses to low off-suit".to_string(),
                lead_player: 0,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Blue,
                        value: 14,
                    }), // lead (highest overall)
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 1,
                    }), // winner (lowest off-suit)
                    Some(Card {
                        id: 2,
                        suit: Suit::Blue,
                        value: 12,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Blue,
                        value: 10,
                    }),
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
            TrickWinnerScenario {
                name: "Multiple off-suits with zeroes mixed in".to_string(),
                lead_player: 3,
                current_hand: [
                    Some(Card {
                        id: 0,
                        suit: Suit::Yellow,
                        value: 0,
                    }),
                    Some(Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 8,
                    }), // winner
                    Some(Card {
                        id: 2,
                        suit: Suit::Purple,
                        value: 0,
                    }),
                    Some(Card {
                        id: 3,
                        suit: Suit::Green,
                        value: 5,
                    }), // lead
                ],
                expected_winning_player: 1,
                expected_score_hand: true,
            },
        ];
        for scenario in scenarios {
            let trick_result =
                StickEmGame::trick_winner(scenario.lead_player, scenario.current_hand);
            assert_eq!(
                scenario.expected_winning_player, trick_result.winning_player,
                "winning player for scenario {}",
                scenario.name
            );
            assert_eq!(
                scenario.expected_score_hand, trick_result.score_hand,
                "score hand for scenario {}",
                scenario.name
            );
        }
    }
}
