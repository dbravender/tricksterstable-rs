/*
Game: Kaibosh
A Euchre variant where players bid to name trump
See rules/kaibosh.txt for game rules
*/

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const KAIBOSH: i32 = 12;

// Define the card, player, and game state structures based on Kaibosh rules

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub value: i32,
    pub suit: Suit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KaiboshGame {
    pub hands: [Vec<Card>; 4],
    pub dealer: usize,
    pub current_player: usize,
    pub current_trick: [Option<Card>; 4],
    pub trump: Option<Suit>,
    pub lead_card: Option<Card>,
    pub state: GameState,
    pub bids: [Option<i32>; 4],
    pub voids: [HashSet<Suit>; 4], // voids revealed during play (used for hidden information state determization)
    pub scores: [usize; 2],
    pub score_threshold: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameState {
    #[default]
    Bidding,
    Play,
}

impl KaiboshGame {
    pub fn new() -> Self {
        let mut game = Self {
            ..Default::default()
        };
        game.new_hand();
        // always let human bid first
        game.current_player = 0;
        game.dealer = 3;
        // TODO: make this configurable for humans playing the game
        game.score_threshold = 25;
        return game;
    }

    fn new_hand(&mut self) {
        // deal goes counter clockwise around the table
        self.dealer = (self.dealer + 1) % 4;
        // deal out cards
        self.hands = Self::deal();
        // player to the left of the dealer leads
        self.current_player = (self.dealer + 1) % 4;
        // reset trump
        self.trump = None;
        // reset lead card
        self.lead_card = None;
        // start state is bidding
        self.state = GameState::Bidding;
        // reset bids
        self.bids = [None; 4];
        // clean up trick
        self.current_trick = [None; 4];
        // no longer know which voids a player has revealed
        self.voids = [
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
        ];
        self.scores = [0, 0];
    }

    fn deal() -> [Vec<Card>; 4] {
        let mut deck = Self::create_deck();
        let mut rng = rand::thread_rng();
        let mut hands: [Vec<Card>; 4] = [vec![], vec![], vec![], vec![]];
        deck.shuffle(&mut rng);

        for _ in 0..6 {
            for hand in &mut hands {
                hand.push(deck.pop().expect("The deck should have enough cards"));
            }
        }

        return hands;
    }

    pub fn play_card(&mut self, card: Card) {
        // Handle playing a card
        if self.lead_card.is_none() {
            self.lead_card = Some(card);
        }
        self.hands[self.current_player].retain(|c: &Card| *c != card);
        self.current_trick[self.current_player] = Some(card);
        // TODO - animate trick to table
        self.check_trick_and_hand_end()
    }

    fn check_trick_and_hand_end(&mut self) {
        if self
            .current_trick
            .iter()
            .filter(|&card| card.is_some())
            .count()
            == 4
        {
            // trick is over
            // check if hand is over
            if self.hands.iter().all(|hand| hand.is_empty()) {
                self.calculate_scores();
                // check for end of game
                if self.game_over() {
                    return;
                }
                // Prepare for a new hand if the game continues
                // TODO: animate shuffle
                self.new_hand();
            }
        }
    }

    fn game_over(&mut self) -> bool {
        if self
            .scores
            .iter()
            .any(|&score| score > self.score_threshold)
        {
            // Animate game end - declare winner
            return true;
        }
        return false;
    }

    fn bid(&mut self, bid: i32) {
        self.bids[self.current_player] = Some(bid);
        if bid == KAIBOSH {
            // play begins immediately, current player leads
            self.state = GameState::Play;
            return;
        }
        self.current_player = (self.current_player + 1) % 4;
        if self.bids[self.current_player].is_some() {
            // everyone bid
            self.state = GameState::Play;
        }
    }

    pub fn calculate_scores(&mut self) {
        // Calculate and update scores after a round
        unimplemented!();
    }

    fn create_deck() -> Vec<Card> {
        let mut deck = Vec::new();
        for suit in &[Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
            for value in 9..=14 {
                deck.push(Card { value, suit: *suit });
            }
        }
        deck
    }
}

fn bid_to_string(bid: i32) -> String {
    match bid {
        KAIBOSH => "kaibosh".to_string(),
        _ => bid.to_string(),
    }
}
// Tests for game logic
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bid_to_string_kaibosh() {
        assert_eq!(bid_to_string(KAIBOSH), "kaibosh");
    }

    #[test]
    fn test_bid_to_string_numeric() {
        assert_eq!(bid_to_string(10), "10");
    }

    #[test]
    fn test_new_game() {
        let game = KaiboshGame::new();
        // Each player should have 6 cards
        assert!(game.hands.iter().all(|hand| hand.len() == 6));
        // The game should start with the first player
        assert_eq!(game.current_player, 0);
        // The initial game state should be bidding
        assert_eq!(game.state, GameState::Bidding);
        // No bids should be placed yet
        assert!(game.bids.iter().all(|&bid| bid.is_none()));
        // No voids should be known at the start
        assert!(game.voids.iter().all(|void| void.is_empty()));
    }

    // Additional tests
}
