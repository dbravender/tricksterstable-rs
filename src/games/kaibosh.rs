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
    pub id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KaiboshGame {
    pub hands: [Vec<Card>; 4],
    pub dealer: usize,
    pub current_player: usize,
    pub current_trick: [Option<Card>; 4],
    pub tricks_taken: [i32; 2], // tracked per team
    pub trump: Option<Suit>,
    pub lead_card: Option<Card>,
    pub state: GameState,
    pub bids: [Option<i32>; 4],
    pub voids: [HashSet<Suit>; 4], // voids revealed during play (used for hidden information state determization)
    pub scores: [i32; 2],
    pub score_threshold: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameState {
    #[default]
    Bidding,
    NameTrump,
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

        assert!(deck.is_empty(), "deck should be all dealt");

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

    fn bid(&mut self, bid: Option<i32>) {
        if bid.is_some() && bid.unwrap() <= self.bids.iter().filter_map(|&b| b).max().unwrap_or(0) {
            panic!("bid must increase");
        }

        self.bids[self.current_player] = bid;
        if bid == Some(KAIBOSH) {
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

    pub fn bidding_options(self: &KaiboshGame) -> Vec<i32> {
        let max_bid = self.bids.iter().filter_map(|&b| b).max().unwrap_or(0);
        let mut bids: Vec<i32> = (0..=6).collect();
        bids.push(KAIBOSH);
        bids.retain(|bid| *bid > max_bid);
        bids
    }

    fn play_options(&self) -> Vec<i32> {
        let actions: Vec<i32>;
        if self.lead_card.is_some() {
            actions = self.hands[self.current_player as usize]
                .iter()
                .filter(|c| Some(c.suit) == Some(self.lead_card.unwrap().suit))
                .map(|c| c.id)
                .collect();
            if !actions.is_empty() {
                return actions;
            }
        }
        self.hands[self.current_player as usize]
            .iter()
            .map(|c| c.id)
            .collect()
    }

    pub fn get_moves(&mut self) -> Vec<i32> {
        match self.state {
            GameState::Bidding => self.bidding_options(),
            GameState::NameTrump => (0..=3).collect::<Vec<i32>>(),
            GameState::Play => self.play_options(),
        }
    }

    pub fn made_it(&self, trick_count: i32, bid: i32) -> bool {
        if bid == KAIBOSH {
            trick_count == 6
        } else {
            trick_count >= bid
        }
    }

    pub fn points_for_bid(&self, trick_count: i32, bid: i32) -> i32 {
        let made_it = self.made_it(trick_count, bid);
        if bid == KAIBOSH {
            if made_it {
                12
            } else {
                -12
            }
        } else {
            if made_it {
                trick_count
            } else {
                -bid
            }
        }
    }

    pub fn calculate_scores(&mut self) {
        let bidder = self
            .bids
            .iter()
            .enumerate()
            .max_by_key(|&(_, &bid)| bid.unwrap_or(0))
            .map(|(i, _)| i)
            .unwrap();
        let bidding_team = bidder % 2;
        let defending_team = (bidder + 1) % 2;

        let bid = self.bids[bidder].unwrap();
        let tricks_taken_by_bidding_team = self.tricks_taken[bidding_team];
        self.scores[bidding_team] += self.points_for_bid(tricks_taken_by_bidding_team, bid);
        let tricks_taken_by_defender = self.tricks_taken[defending_team];
        // Defending team scores all tricks taken if the bidder did not make their bid
        if !self.made_it(tricks_taken_by_bidding_team, bid) {
            self.scores[defending_team] += tricks_taken_by_defender;
        }
    }

    fn create_deck() -> Vec<Card> {
        let mut id: i32 = 0;
        let mut deck = Vec::new();
        for suit in &[Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
            for value in 9..=14 {
                deck.push(Card {
                    value,
                    suit: *suit,
                    id,
                });
                id += 1;
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
    #[test]
    fn test_bid_function_increases_bid() {
        let mut game = KaiboshGame::new();
        game.bid(Some(5));
        assert_eq!(game.bids[0], Some(5));
    }

    #[test]
    #[should_panic(expected = "bid must increase")]
    fn test_bid_function_panics_on_lower_bid() {
        let mut game = KaiboshGame::new();
        game.bid(Some(5));
        game.current_player = 1; // Move to next player
        game.bid(Some(4)); // This should panic
    }

    #[test]
    fn test_bid_function_kaibosh_ends_bidding() {
        let mut game = KaiboshGame::new();
        game.bid(Some(KAIBOSH));
        assert_eq!(game.state, GameState::Play);
    }

    #[test]
    fn test_play_card_moves_card_from_hand_to_trick() {
        let mut game = KaiboshGame::new();
        let test_card = Card {
            id: 0,
            value: 9,
            suit: Suit::Hearts,
        };
        game.hands[0] = vec![test_card]; // Simplify the hand for the test
        game.play_card(test_card);
        assert!(game.hands[0].is_empty());
        assert_eq!(game.current_trick[0], Some(test_card));
    }

    #[test]
    fn test_game_over_returns_true_when_score_exceeds_threshold() {
        let mut game = KaiboshGame::new();
        game.scores[0] = 26; // Set score above threshold
        assert!(game.game_over());
    }

    #[test]
    fn test_new_hand_resets_game_state() {
        let mut game = KaiboshGame::new();
        game.new_hand();
        assert_eq!(game.state, GameState::Bidding);
        // No bids should be placed yet
        assert!(game.bids.iter().all(|&bid| bid.is_none()));
        // Trick should be empty
        assert!(game.current_trick.iter().all(|&card| card.is_none()));
        // Each player should have 6 cards
        assert!(game.hands.iter().all(|hand| hand.len() == 6));
        // No voids should be known at the start of a new hand
        assert!(game.voids.iter().all(|void| void.is_empty()));
    }

    #[test]
    fn test_bid_function_allows_higher_bid() {
        let mut game = KaiboshGame::new();
        game.bid(Some(5));
        game.current_player = 1; // Move to next player
        game.bid(Some(6)); // This should succeed
        assert_eq!(game.bids[1], Some(6));
    }

    #[test]
    fn test_bid_function_progresses_player() {
        let mut game = KaiboshGame::new();
        game.bid(Some(5));
        assert_eq!(game.current_player, 1); // Should move to the next player
    }

    #[test]
    fn test_bid_function_handles_pass() {
        let mut game = KaiboshGame::new();
        game.bid(None); // Player 0 passes
        assert_eq!(game.bids[0], None);
        assert_eq!(game.current_player, 1); // Should move to the next player
    }

    #[test]
    fn test_bid_function_ends_with_kaibosh_bid() {
        let mut game = KaiboshGame::new();
        game.bid(Some(KAIBOSH)); // Player 0 bids Kaibosh
        assert_eq!(game.bids[0], Some(KAIBOSH));
        assert_eq!(game.state, GameState::Play); // State should change to Play
    }

    #[test]
    fn test_check_trick_and_hand_end_ends_hand() {
        let mut game = KaiboshGame::new();
        for i in 0..4 {
            game.hands[i].clear(); // Simulate that all hands are played
        }
        game.check_trick_and_hand_end();
        assert!(game.hands.iter().all(|hand| hand.is_empty())); // All hands should be empty
        assert_eq!(game.state, GameState::Bidding); // New hand should start, state should reset to Bidding
    }

    #[test]
    fn test_game_over_false_when_under_threshold() {
        let mut game = KaiboshGame::new();
        game.scores[0] = 10; // Set score below threshold
        assert!(!game.game_over());
    }

    #[test]
    fn test_bidding_options_includes_kaibosh() {
        let game = KaiboshGame::new();
        assert!(game.bidding_options().contains(&KAIBOSH));
    }

    #[test]
    fn test_bidding_options_excludes_lower_bids() {
        let mut game = KaiboshGame::new();
        game.bid(Some(5));
        game.current_player = 1; // Move to next player to test options
        let options = game.bidding_options();
        for i in 0..=5 {
            assert!(
                !options.contains(&i),
                "Options should not include bid lower than 5, found {}",
                i
            );
        }
    }

    #[test]
    fn test_bidding_options_includes_higher_bids_after_bid() {
        let mut game = KaiboshGame::new();
        game.bid(Some(5));
        game.current_player = 1; // Move to next player to test options
        let options = game.bidding_options();
        println!("{:?}", options);
        assert!(options.contains(&6), "Options should include 6");
        assert!(options.contains(&KAIBOSH), "Options should include kaibosh");
    }

    #[test]
    fn test_play_options_follow_suit() {
        let mut game = KaiboshGame::new();
        game.lead_card = Some(Card {
            id: 0,
            value: 9,
            suit: Suit::Hearts,
        });
        game.hands[0] = vec![
            Card {
                id: 1,
                value: 10,
                suit: Suit::Hearts,
            }, // should be included
            Card {
                id: 2,
                value: 11,
                suit: Suit::Diamonds,
            }, // should not be included
        ];
        game.current_player = 0;
        let options = game.play_options();
        assert_eq!(options.len(), 1);
        assert!(options.contains(&1));
    }

    #[test]
    fn test_play_options_no_lead_suit() {
        let mut game = KaiboshGame::new();
        game.lead_card = Some(Card {
            id: 0,
            value: 9,
            suit: Suit::Hearts,
        });
        game.hands[0] = vec![
            Card {
                id: 1,
                value: 10,
                suit: Suit::Diamonds,
            }, // should be included
            Card {
                id: 2,
                value: 11,
                suit: Suit::Clubs,
            }, // should be included
        ];
        game.current_player = 0;
        let options = game.play_options();
        assert_eq!(options.len(), 2);
        assert!(options.contains(&1) && options.contains(&2));
    }

    #[test]
    fn test_play_options_all_cards() {
        let mut game = KaiboshGame::new();
        game.lead_card = None; // No lead card yet
        game.hands[0] = vec![
            Card {
                id: 1,
                value: 10,
                suit: Suit::Diamonds,
            },
            Card {
                id: 2,
                value: 11,
                suit: Suit::Clubs,
            },
        ];
        game.current_player = 0;
        let options = game.play_options();
        assert_eq!(options.len(), 2);
        assert!(options.contains(&1) && options.contains(&2));
    }
}
