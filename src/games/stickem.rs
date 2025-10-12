/*
Game: Stick 'Em (Sticheln)
Designer:  Klaus Palesch
BoardGameGeek: https://boardgamegeek.com/boardgame/354/stick-em
*/

use std::collections::BTreeSet;

use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::utils::shuffle_and_divide_matching_cards;

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
    pub cards_won: [Vec<Card>; PLAYER_COUNT],
    pub experiment: bool, // Set to true when testing new reward functions
}

impl StickEmGame {
    pub fn new() -> Self {
        let mut game = StickEmGame {
            ..Default::default()
        };
        // Randomly select a start player each game
        game.dealer = thread_rng().gen_range(0..PLAYER_COUNT);
        game.deal();
        game
    }

    fn deal(&mut self) {
        let mut deck = StickEmGame::deck();
        self.cards_won = [vec![], vec![], vec![], vec![]];
        self.pain_cards = [None; PLAYER_COUNT];
        self.state = State::SelectPainColor;
        self.dealer = (self.dealer + 1) % PLAYER_COUNT;
        self.current_player = self.dealer;
        self.lead_player = self.dealer;
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
        self.current_player = (self.current_player + 1) % PLAYER_COUNT;
        if self.pain_cards.iter().all(|c| c.is_some()) {
            self.current_player = self.dealer;
            self.state = State::Play;
        }
    }

    pub fn play(&mut self, card_id: i32) {
        let card = self.pop_card(card_id);
        self.current_hand[self.current_player] = Some(card);
        // TODO: Animate played cards
        if self.current_hand.iter().any(|c| c.is_none()) {
            self.current_player = (self.current_player + 1) % PLAYER_COUNT;
            return;
        }
        // The trick is over
        let trick_result = StickEmGame::trick_winner(self.lead_player, self.current_hand);
        self.current_player = trick_result.winning_player;
        self.lead_player = self.current_player;
        if trick_result.score_hand {
            // Add won cards to the winner's won cards
            self.cards_won[self.current_player].extend(self.current_hand.iter().flatten().copied())
            // TODO: Animate trick won to winner
        } else {
            // Animate trick off screen - not to winner
        }

        // Reset the trick
        self.current_hand = [None; 4];

        if self.hands.iter().any(|h| h.len() > 0) {
            // Hand continues
            return;
        }

        // Round is over
        for player in 0..PLAYER_COUNT {
            let score = StickEmGame::score_cards_won(
                self.pain_cards[player].unwrap(),
                &self.cards_won[player],
            );
            // TODO: animate score for player from scores[player] to score
            self.scores[player] += score;
        }

        if self.round >= PLAYER_COUNT as i32 {
            self.state = State::GameOver;
            let max_score = self.scores.iter().max().unwrap();
            for player in 0..PLAYER_COUNT {
                if self.scores[player] == *max_score {
                    // Ties go to player 0 (human player)
                    self.winner = Some(player);
                    break;
                }
            }
            return;
        } else {
            self.deal();
        }
    }

    pub fn pop_card(&mut self, card_id: i32) -> Card {
        let pos = self.hands[self.current_player]
            .iter()
            .position(|c| c.id == card_id)
            .unwrap();
        self.hands[self.current_player].remove(pos)
    }

    pub fn score_cards_won(pain_card: Card, cards_won: &Vec<Card>) -> i32 {
        let mut score = -pain_card.value;
        for card in cards_won {
            score += if card.suit == pain_card.suit {
                -card.value
            } else {
                1
            }
        }
        score
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

impl ismcts::Game for StickEmGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        // Pain cards are played face down until all are played
        let mut pain_card_played = [false; PLAYER_COUNT];

        if self.pain_cards.contains(&None) {
            for (player, card) in self.pain_cards.iter().enumerate() {
                if let Some(card) = card {
                    pain_card_played[player] = true;
                    self.hands[player].push(*card);
                }
            }
        }

        for p1 in 0..PLAYER_COUNT {
            for p2 in 0..PLAYER_COUNT {
                if p1 == p2 {
                    continue;
                }
                if p1 == self.current_player() || p2 == self.current_player() {
                    // Don't swap current player's cards - player knows exactly what they have
                    continue;
                }
                let mut new_hands = vec![
                    self.hands[p1 as usize].clone(),
                    self.hands[p2 as usize].clone(),
                ];

                shuffle_and_divide_matching_cards(|_: &Card| true, &mut new_hands, rng);

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();
            }
        }

        for player in 0..PLAYER_COUNT {
            if pain_card_played[player] {
                let card = self.hands[player].pop();
                self.pain_cards[player] = card;
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
        if self.state != State::GameOver {
            // the hand is not over
            None
        } else {
            let max_score = *self.scores.iter().max().unwrap() as f64;
            let min_score = *self.scores.iter().min().unwrap() as f64;
            let player_score = self.scores[player] as f64;

            if max_score == min_score {
                // Everyone tied - neutral result
                return Some(0.0);
            }

            // Exponential reward function (based on experiments showing improved performance)
            // Linear interpolation between worst and best score, then apply exponential transformation
            let linear = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;

            // Apply exponential transformation: sign(x) * (|x|^2)
            // This amplifies reward differences while preserving direction
            let exponential_reward = linear.signum() * linear.abs().powf(2.0);

            Some(exponential_reward)
        }
    }
}

pub fn get_mcts_move(game: &StickEmGame, iterations: i32, debug: bool) -> i32 {
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

    struct ScoreScenario {
        pain_card: Card,
        cards_won: Vec<Card>,
        expected_score: i32,
    }

    #[test]
    fn test_score() {
        let scenarios = [
            // Jillian selected the Yellow 2 at the beginning of the round as her Pain Color. She
            // receives 11 negative points (2+5+4). For the other six cards, she receives one point
            // each for a total of 6 positive points. Jillian’s point total is -5.
            ScoreScenario {
                pain_card: Card {
                    id: 0,
                    suit: Suit::Yellow,
                    value: 2,
                },
                cards_won: vec![
                    Card {
                        id: 1,
                        suit: Suit::Yellow,
                        value: 5,
                    },
                    Card {
                        id: 2,
                        suit: Suit::Yellow,
                        value: 4,
                    },
                    Card {
                        id: 3,
                        suit: Suit::Red,
                        value: 5,
                    },
                    Card {
                        id: 4,
                        suit: Suit::Purple,
                        value: 11,
                    },
                    Card {
                        id: 5,
                        suit: Suit::Green,
                        value: 11,
                    },
                    Card {
                        id: 6,
                        suit: Suit::Green,
                        value: 8,
                    },
                    Card {
                        id: 7,
                        suit: Suit::Green,
                        value: 2,
                    },
                    Card {
                        id: 8,
                        suit: Suit::Blue,
                        value: 1,
                    },
                ],
                expected_score: -5,
            },
            // Andrew selected the Red 0 at the beginning of the round as his Pain Color. He
            // receives 7 negative points (0+2+4+1). For the other nine cards, he receives one
            // point each for a total of 9 positive points. Andrew’s point total is +2.
            ScoreScenario {
                pain_card: Card {
                    id: 0,
                    suit: Suit::Red,
                    value: 0,
                },
                cards_won: vec![
                    Card {
                        id: 1,
                        suit: Suit::Red,
                        value: 2,
                    },
                    Card {
                        id: 2,
                        suit: Suit::Red,
                        value: 4,
                    },
                    Card {
                        id: 3,
                        suit: Suit::Red,
                        value: 1,
                    },
                    Card {
                        id: 4,
                        suit: Suit::Yellow,
                        value: 0,
                    },
                    Card {
                        id: 5,
                        suit: Suit::Blue,
                        value: 3,
                    },
                    Card {
                        id: 6,
                        suit: Suit::Blue,
                        value: 7,
                    },
                    Card {
                        id: 7,
                        suit: Suit::Blue,
                        value: 10,
                    },
                    Card {
                        id: 8,
                        suit: Suit::Blue,
                        value: 6,
                    },
                    Card {
                        id: 9,
                        suit: Suit::Purple,
                        value: 9,
                    },
                    Card {
                        id: 10,
                        suit: Suit::Purple,
                        value: 8,
                    },
                    Card {
                        id: 11,
                        suit: Suit::Purple,
                        value: 0,
                    },
                    Card {
                        id: 12,
                        suit: Suit::Green,
                        value: 6,
                    },
                ],
                expected_score: 2,
            },
        ];

        for scenario in scenarios {
            let actual_score =
                StickEmGame::score_cards_won(scenario.pain_card, &scenario.cards_won);
            assert_eq!(actual_score, scenario.expected_score);
        }
    }

    #[test]
    fn test_select_final_pain_card() {
        let mut game = StickEmGame::new();
        game.current_player = 1;
        game.dealer = 2;
        game.hands[1] = vec![Card {
            id: 4,
            value: 11,
            suit: Suit::Purple,
        }];
        game.pain_cards = [
            Some(Card {
                id: 0,
                value: 0,
                suit: Suit::Red,
            }),
            None,
            Some(Card {
                id: 2,
                value: 0,
                suit: Suit::Yellow,
            }),
            Some(Card {
                id: 3,
                value: 0,
                suit: Suit::Purple,
            }),
        ];
        game.apply_move(4);
        assert_eq!(
            game.current_player, game.dealer,
            "After last player selects pain suit "
        );
        assert_eq!(game.pain_cards[1].unwrap().id, 4);
        assert_eq!(
            game.state,
            State::Play,
            "Game should transition to play state after all pain cards are selected"
        );
    }

    #[test]
    fn test_play_final_card_in_trick() {
        let mut game = StickEmGame::new();
        game.state = State::Play;
        game.current_player = 1;
        game.lead_player = 2;
        game.dealer = 2;
        game.hands[1] = vec![Card {
            id: 4,
            value: 11,
            suit: Suit::Purple,
        }];
        game.current_hand = [
            Some(Card {
                id: 0,
                value: 0,
                suit: Suit::Red,
            }),
            None,
            Some(Card {
                id: 2,
                value: 0,
                suit: Suit::Yellow,
            }),
            Some(Card {
                id: 3,
                value: 0,
                suit: Suit::Purple,
            }),
        ];
        game.apply_move(4);
        assert_eq!(
            game.current_player, 1,
            "Player 1 won by playing the highest off suit card"
        );
        assert!(
            game.current_hand.iter().all(|c| c.is_none()),
            "Hand is reset"
        );
    }

    #[test]
    fn test_play_final_card_in_trick_last_round() {
        let mut game = StickEmGame::new();
        game.scores = [-50, 0, 0, 0];
        game.round = PLAYER_COUNT as i32;
        game.state = State::Play;
        game.current_player = 1;
        game.lead_player = 2;
        game.dealer = 2;
        game.pain_cards = [
            Some(Card {
                id: 4,
                value: 1,
                suit: Suit::Purple,
            }),
            Some(Card {
                id: 4,
                value: 2,
                suit: Suit::Purple,
            }),
            Some(Card {
                id: 4,
                value: 3,
                suit: Suit::Purple,
            }),
            Some(Card {
                id: 4,
                value: 4,
                suit: Suit::Purple,
            }),
        ];
        game.hands = [
            vec![],
            vec![Card {
                id: 4,
                value: 11,
                suit: Suit::Purple,
            }],
            vec![],
            vec![],
        ];
        game.current_hand = [
            Some(Card {
                id: 0,
                value: 0,
                suit: Suit::Red,
            }),
            None,
            Some(Card {
                id: 2,
                value: 0,
                suit: Suit::Yellow,
            }),
            Some(Card {
                id: 3,
                value: 0,
                suit: Suit::Purple,
            }),
        ];
        game.apply_move(4);
        assert_eq!(
            game.current_player, 1,
            "Player 1 won by playing the highest off suit card"
        );
        assert_eq!(game.state, State::GameOver);
        assert_eq!(game.scores, [-51, -11, -3, -4], "Scores are correct");
        assert_eq!(game.winner, Some(2), "Winner is properly set")
    }
}
