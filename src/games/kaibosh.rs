/*
Game: Kaibosh
A Euchre variant where players bid to name trump
See rules/kaibosh.md for game rules
*/

use ismcts::IsmctsHandler;
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::{
    cmp::min,
    collections::{HashMap, HashSet},
};

use crate::utils::shuffle_and_divide_matching_cards;

const KAIBOSH: i32 = 12;
const JACK: i32 = 11;
const MISDEAL: i32 = 100; // high so it can be "bid" anytime

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
    pub bidder: Option<usize>, // player that bid
    pub high_bid: Option<i32>, // bid made by the bidder
    pub dealer: usize,
    pub current_player: usize,
    pub current_trick: [Option<Card>; 4],
    pub tricks_taken: [i32; 2], // tracked per team
    pub trump: Option<Suit>,
    pub lead_card: Option<Card>,
    pub state: GameState,
    pub bids: [Option<i32>; 4],
    pub voids: [HashSet<Suit>; 4], // voids revealed during play (used for hidden information state determization)
    pub scores: [i32; 2],          // team scores
    pub scores_this_hand: [i32; 2], // team scores for current hand (used during search)
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
        // reset bidder
        self.bidder = None;
        // reset bid
        self.high_bid = None;
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
        self.scores_this_hand = [0, 0];
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

    pub fn name_trump(&mut self, trump: i32) {
        let suit = match trump {
            0 => Suit::Clubs,
            1 => Suit::Diamonds,
            2 => Suit::Hearts,
            3 => Suit::Spades,
            _ => panic!("Invalid trump suit"),
        };
        self.trump = Some(suit);
        if self.high_bid != Some(KAIBOSH) {
            // current player stays when kaibosh is bid
            self.current_player = (self.dealer + 1) % 4;
        }
        self.state = GameState::Play;
    }

    pub fn play_card(&mut self, id: i32) {
        let card = *self.hands[self.current_player]
            .iter()
            .find(|c| c.id == id)
            .expect("Card not found in player's hand");
        // Handle playing a card
        if self.lead_card.is_none() {
            self.lead_card = Some(card);
        }
        self.hands[self.current_player].retain(|c: &Card| *c != card);
        if self.lead_card.is_some() && card.suit != self.lead_card.unwrap().suit {
            // if the player didn't follow suit then they have revealed a void
            // which is used when determining which cards a player might have
            // during simulations
            self.voids[self.current_player].insert(self.lead_card.unwrap().suit);
        }
        self.current_trick[self.current_player] = Some(card);
        self.current_player = (self.current_player + 1) % 4;
        if self.high_bid == Some(KAIBOSH) && self.current_player == (self.bidder.unwrap() + 2) % 4 {
            // skip partner during loners
            self.current_player = (self.current_player + 1) % 4;
        }
        // TODO - animate trick to table
        self.check_trick_and_hand_end()
    }

    fn check_trick_and_hand_end(&mut self) {
        let card_count = if self.bids.contains(&Some(KAIBOSH)) {
            3
        } else {
            4
        };
        if self
            .current_trick
            .iter()
            .filter(|&card| card.is_some())
            .count()
            == card_count
        {
            // trick is over

            let trick_winner = get_winner(
                self.lead_card.unwrap().suit,
                self.trump.unwrap(),
                &self.current_trick,
            );
            let winning_card =
                self.current_trick[trick_winner].expect("there has to be a trick_winner card");
            self.current_trick = [None; 4];
            self.lead_card = None;
            self.tricks_taken[trick_winner % 2] += 1;
            // TODO: animate trick to winner
            // winner of the trick leads
            self.current_player = trick_winner;
            // check if hand is over
            if self.hands.iter().filter(|hand| hand.is_empty()).count() >= 3 {
                self.calculate_scores();
                // check for end of game
                if self.game_over() {
                    // Animate game end - declare winner
                    return;
                }
                // Prepare for a new hand if the game continues
                // TODO: animate shuffle
                self.new_hand();
            }
        }
    }

    fn game_over(&self) -> bool {
        if self
            .scores
            .iter()
            .any(|&score| score >= self.score_threshold)
        {
            return true;
        }
        return false;
    }

    fn check_for_misdeal(&self, player: usize) -> bool {
        let hand = &self.hands[player];
        let nines_count = hand.iter().filter(|&card| card.value == 9).count();
        let tens_count = hand.iter().filter(|&card| card.value == 10).count();
        nines_count == 4 || (nines_count == 3 && tens_count >= 2)
    }

    fn bid(&mut self, bid: Option<i32>) {
        if bid.is_some() && bid.unwrap() <= self.bids.iter().filter_map(|&b| b).max().unwrap_or(0) {
            panic!("bid must increase");
        }

        self.bids[self.current_player] = bid;
        if bid == Some(KAIBOSH) {
            self.high_bid = Some(KAIBOSH);
            self.bidder = Some(self.current_player);
            // player names trump and then leads immediately
            self.state = GameState::NameTrump;
            return;
        }

        if bid == Some(MISDEAL) {
            // redeal - dealer moves to the next player - no score
            // TODO: animation or dialog informing of the misdeal
            self.dealer = (self.dealer + 1) % 4;
            self.new_hand();
            return;
        }

        self.current_player = (self.current_player + 1) % 4;
        if self.bids[self.current_player].is_some() {
            // everyone bid
            let bidder = self
                .bids
                .iter()
                .enumerate()
                .max_by_key(|&(_, &bid)| bid.unwrap_or(0))
                .map(|(i, _)| i)
                .unwrap();
            self.bidder = Some(bidder);
            self.high_bid = self.bids[bidder];
            self.current_player = bidder;
            self.state = GameState::NameTrump;
        }
    }

    pub fn bidding_options(self: &KaiboshGame) -> Vec<i32> {
        let max_bid = self.bids.iter().filter_map(|&b| b).max().unwrap_or(0);
        let mut bids: Vec<i32> = (0..=6).collect();
        if self.check_for_misdeal(self.current_player) {
            bids.push(MISDEAL);
        }
        bids.push(KAIBOSH);
        bids.retain(|bid| *bid > max_bid);
        bids
    }

    fn play_options(&self) -> Vec<i32> {
        let actions: Vec<i32>;
        if let Some(lead_card) = self.lead_card {
            actions = self.hands[self.current_player as usize]
                .iter()
                .filter(|c| c.suit == lead_card.suit)
                .map(|c| c.id)
                .collect();
            if !actions.is_empty() {
                return actions;
            }
        }

        let actions: Vec<i32> = self.hands[self.current_player as usize]
            .iter()
            .map(|c| c.id)
            .collect();
        actions
    }

    pub fn get_moves(&self) -> Vec<i32> {
        match self.state {
            GameState::Bidding => self.bidding_options(),
            GameState::NameTrump => (0..=3).collect::<Vec<i32>>(),
            GameState::Play => self.play_options(),
        }
    }

    pub fn apply_move(&mut self, mov: Option<i32>) {
        // reset only after a move is made in the next round
        // so the tree search can see the result
        self.scores_this_hand = [0, 0];
        match self.state {
            GameState::Bidding => self.bid(mov),
            GameState::NameTrump => self.name_trump(mov.unwrap()),
            GameState::Play => self.play_card(mov.unwrap()),
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
        self.scores_this_hand[bidding_team] +=
            self.points_for_bid(tricks_taken_by_bidding_team, bid);
        let tricks_taken_by_defender = self.tricks_taken[defending_team];
        // Defending team scores all tricks taken if the bidder did not make their bid
        if !self.made_it(tricks_taken_by_bidding_team, bid) {
            self.scores[defending_team] += tricks_taken_by_defender;
            self.scores_this_hand[defending_team] += tricks_taken_by_defender;
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

pub fn get_winner(lead_suit: Suit, trump_suit: Suit, trick: &[Option<Card>; 4]) -> usize {
    let mut card_id_to_player: HashMap<i32, usize> = HashMap::new();
    for (index, card_option) in trick.iter().enumerate() {
        if let Some(card) = card_option {
            card_id_to_player.insert(card.id, index);
        }
    }
    let mut cards: Vec<Card> = trick.iter().filter_map(|&c| c).collect();
    cards.sort_by_key(|c| std::cmp::Reverse(value_for_card(lead_suit, trump_suit, c)));
    *card_id_to_player
        .get(&cards.first().expect("there should be a winning card").id)
        .expect("cards_to_player missing card")
}

pub fn same_color(suita: Suit, suitb: Suit) -> bool {
    suita == Suit::Diamonds && suitb == Suit::Hearts
        || suita == Suit::Hearts && suitb == Suit::Diamonds
        || suita == Suit::Clubs && suitb == Suit::Spades
        || suita == Suit::Spades && suitb == Suit::Clubs
}

pub fn value_for_card(lead_suit: Suit, trump_suit: Suit, c: &Card) -> i32 {
    // jack of trump is the strongest card (right bower)
    if c.suit == trump_suit && c.value == JACK {
        return 1000;
    }
    // jack of same color suit as trump is the second strongest card (left bower)
    if same_color(trump_suit, c.suit) && c.value == JACK {
        return 500;
    }
    if c.suit == trump_suit {
        return c.value + 200;
    }
    if c.suit == lead_suit {
        return c.value + 100;
    }
    return c.value;
}

impl ismcts::Game for KaiboshGame {
    type Move = i32;
    type PlayerTag = i32;
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
        self.current_player as i32
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player as i32 + 1) % 4
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.apply_move(Some(*mov));
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        let hand_over = self.scores_this_hand.iter().any(|&score| score != 0);
        if !hand_over {
            None
        } else {
            let mut score = self.scores_this_hand[player as usize % 2];
            if score <= 0 {
                // Capping the score at -6
                score = min(-6, score);
                let normalized_score = (score.abs() as f64) / 6.0;
                // Normalizing the score to 0 - .2
                Some(0.2 * (1.0 - normalized_score))
            } else {
                let score = score as f64 / 6.0;
                Some(0.2 + (0.8 * score))
            }
        }
    }
}

pub fn get_mcts_move(game: &KaiboshGame, iterations: i32) -> i32 {
    let mut new_game = game.clone();
    new_game.score_threshold = -10000;
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 1;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    ismcts.best_move().expect("should have a move to make")
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
        assert_eq!(game.state, GameState::NameTrump);
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
        game.play_card(0);
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
        assert_eq!(game.state, GameState::NameTrump); // State should change to Play
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

    #[test]
    fn test_calculate_scores_bidder_wins() {
        let mut game = KaiboshGame::new();
        game.bids[0] = Some(2); // Player 0 bids 2
        game.tricks_taken[0] = 2; // Player 0's team wins 2 tricks
        game.calculate_scores();
        assert_eq!(game.scores[0], 2); // Player 0's team should score 2 points
    }

    #[test]
    fn test_calculate_scores_bidder_loses() {
        let mut game = KaiboshGame::new();
        game.bids[0] = Some(3); // Player 0 bids 3
        game.tricks_taken[0] = 2; // Player 0's team wins 2 tricks, less than the bid
        game.calculate_scores();
        assert_eq!(game.scores[0], -3); // Player 0's team should lose 3 points
    }

    #[test]
    fn test_calculate_scores_kaibosh_win() {
        let mut game = KaiboshGame::new();
        game.bids[0] = Some(KAIBOSH); // Player 0 bids KAIBOSH
        game.tricks_taken[0] = 6; // Player 0's team wins all 6 tricks
        game.calculate_scores();
        assert_eq!(game.scores[0], 12); // Player 0's team should score 12 points
    }

    #[test]
    fn test_calculate_scores_kaibosh_lose() {
        let mut game = KaiboshGame::new();
        game.bids[0] = Some(KAIBOSH); // Player 0 bids KAIBOSH
        game.tricks_taken[0] = 5; // Player 0's team wins 5 tricks, not all
        game.tricks_taken[1] = 1; // Player 1's team wins 1 trick
        game.calculate_scores();
        assert_eq!(game.scores[0], -12); // Player 0's team should lose 12 points
        assert_eq!(game.scores[1], 1); // Player 1's team should score 1 point
    }

    #[test]
    fn test_calculate_scores_defender_scores() {
        let mut game = KaiboshGame::new();
        game.bids[2] = Some(3); // Player 2 bids 3
        game.tricks_taken[1] = 4; // Player 2's team (defenders) wins 4 tricks
        game.calculate_scores();
        assert_eq!(game.scores[1], 4); // Player 2's team should score 4 points
    }
}
#[test]
fn test_misdeal_with_four_nines() {
    let mut game = KaiboshGame::new();
    // Set up a hand with four nines for the current player
    game.hands[game.current_player] = vec![
        Card {
            value: 9,
            suit: Suit::Hearts,
            id: 0,
        },
        Card {
            value: 9,
            suit: Suit::Diamonds,
            id: 1,
        },
        Card {
            value: 9,
            suit: Suit::Clubs,
            id: 2,
        },
        Card {
            value: 9,
            suit: Suit::Spades,
            id: 3,
        },
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 4,
        },
        Card {
            value: 10,
            suit: Suit::Diamonds,
            id: 5,
        },
    ];
    assert!(game.check_for_misdeal(game.current_player));
}

#[test]
fn test_misdeal_with_three_nines_two_tens() {
    let mut game = KaiboshGame::new();
    // Set up a hand with three nines and two tens for the current player
    game.hands[game.current_player] = vec![
        Card {
            value: 9,
            suit: Suit::Hearts,
            id: 0,
        },
        Card {
            value: 9,
            suit: Suit::Diamonds,
            id: 1,
        },
        Card {
            value: 9,
            suit: Suit::Clubs,
            id: 2,
        },
        Card {
            value: 10,
            suit: Suit::Spades,
            id: 3,
        },
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 4,
        },
        Card {
            value: 11,
            suit: Suit::Diamonds,
            id: 5,
        },
    ];
    assert!(game.check_for_misdeal(game.current_player));
}

#[test]
fn test_get_moves_includes_misdeal() {
    let mut game = KaiboshGame::new();
    // Set up a hand with four nines for the current player
    game.hands[game.current_player] = vec![
        Card {
            value: 9,
            suit: Suit::Hearts,
            id: 0,
        },
        Card {
            value: 9,
            suit: Suit::Diamonds,
            id: 1,
        },
        Card {
            value: 9,
            suit: Suit::Clubs,
            id: 2,
        },
        Card {
            value: 9,
            suit: Suit::Spades,
            id: 3,
        },
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 4,
        },
        Card {
            value: 10,
            suit: Suit::Diamonds,
            id: 5,
        },
    ];
    let moves = game.get_moves();
    assert!(
        moves.contains(&MISDEAL),
        "get_moves should include MISDEAL for a misdeal-eligible hand"
    );
}

#[test]
fn test_bid_with_misdeal_advances_game_state() {
    let mut game = KaiboshGame::new();
    // Set up a hand with four nines for the current player
    game.hands[game.current_player] = vec![
        Card {
            value: 9,
            suit: Suit::Hearts,
            id: 0,
        },
        Card {
            value: 9,
            suit: Suit::Diamonds,
            id: 1,
        },
        Card {
            value: 9,
            suit: Suit::Clubs,
            id: 2,
        },
        Card {
            value: 9,
            suit: Suit::Spades,
            id: 3,
        },
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 4,
        },
        Card {
            value: 10,
            suit: Suit::Diamonds,
            id: 5,
        },
    ];
    game.bid(Some(MISDEAL));
    assert_eq!(
        game.state,
        GameState::Bidding,
        "Game state should be reset to Bidding after a misdeal"
    );
    assert_eq!(
        game.dealer, 1,
        "Dealer should advance to the next player after a misdeal"
    );
}

#[test]
fn test_no_misdeal_with_insufficient_nines_or_tens() {
    let mut game = KaiboshGame::new();
    // Set up a hand without the necessary nines or tens for misdeal
    game.hands[game.current_player] = vec![
        Card {
            value: 9,
            suit: Suit::Hearts,
            id: 0,
        },
        Card {
            value: 9,
            suit: Suit::Diamonds,
            id: 1,
        },
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 2,
        },
        Card {
            value: 11,
            suit: Suit::Hearts,
            id: 3,
        },
        Card {
            value: 12,
            suit: Suit::Hearts,
            id: 4,
        },
        Card {
            value: 13,
            suit: Suit::Hearts,
            id: 5,
        },
    ];
    assert!(!game.check_for_misdeal(game.current_player));
}
