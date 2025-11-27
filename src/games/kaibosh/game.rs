/*
Game: Kaibosh
A Euchre variant where players bid to name trump
See rules/kaibosh.md for game rules
*/

use ismcts::IsmctsHandler;
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::utils::shuffle_and_divide_matching_cards;

pub const KAIBOSH: i32 = 12;
pub const DEFAULT_SCORE_THRESHOLD: i32 = 25;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Black,
}

impl Suit {
    pub fn color(&self) -> Color {
        match self {
            Suit::Hearts | Suit::Diamonds => Color::Red,
            Suit::Clubs | Suit::Spades => Color::Black,
        }
    }

    pub fn same_color_suit(&self) -> Suit {
        match self {
            Suit::Hearts => Suit::Diamonds,
            Suit::Diamonds => Suit::Hearts,
            Suit::Clubs => Suit::Spades,
            Suit::Spades => Suit::Clubs,
        }
    }

    pub fn all() -> Vec<Suit> {
        vec![Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades]
    }
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
    pub use_heuristic: bool,
    pub use_policy_priors: bool,      // use policy network priors in MCTS
    pub experiment: i32,              // experimental reward function (0=baseline, 1+=experiments)
    pub last_hand_score: Option<i32>, // score from the last completed hand
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
        game.score_threshold = DEFAULT_SCORE_THRESHOLD;
        game.use_policy_priors = true; // Enable by default
        game
    }

    pub fn new_hand(&mut self) {
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
        // reset tricks taken
        self.tricks_taken = [0, 0];
        // no longer know which voids a player has revealed
        self.voids = [
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
        ];
        // Don't reset game scores - only reset hand scores
        self.scores_this_hand = [0, 0];
        self.last_hand_score = None;
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

        hands
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
            let _winning_card =
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
                    #[allow(clippy::needless_return)]
                    return;
                }
                // FIX: Don't call new_hand() here - it resets scores_this_hand to [0,0]
                // which breaks ISMCTS rollouts that check result() to see if hand is over.
                // For actual gameplay, the UI should call new_hand() manually.
                // For ISMCTS, we only simulate one hand and check last_hand_score.
                // Commented out to prevent infinite loops in ISMCTS:
                // self.new_hand();
            }
        }
    }

    pub fn game_over(&self) -> bool {
        if self
            .scores
            .iter()
            .any(|&score| score >= self.score_threshold)
        {
            return true;
        }
        false
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
        bids.retain(|bid| *bid == 0 || *bid > max_bid);

        // Convention logic:
        // 1 - signals 2 or more aces
        // 2 - signals one black jack and one red jack
        // Only allowed if partner hasn't bid yet.

        // Determine if partner has bid.
        // Bidding order: (dealer + 1) % 4, (dealer + 2) % 4, ...
        // 1st and 2nd players to bid have partners who haven't bid yet.
        // 3rd and 4th players have partners who HAVE bid.

        // Calculate position relative to first bidder (0 to 3)
        // first bidder is (dealer + 1) % 4
        let first_bidder = (self.dealer + 1) % 4;
        let relative_pos = (self.current_player + 4 - first_bidder) % 4;
        let partner_has_bid = relative_pos >= 2;

        let hand = &self.hands[self.current_player];
        let aces = hand.iter().filter(|c| c.value == 14).count();
        let has_black_jack = hand
            .iter()
            .any(|c| c.value == 11 && (c.suit == Suit::Clubs || c.suit == Suit::Spades));
        let has_red_jack = hand
            .iter()
            .any(|c| c.value == 11 && (c.suit == Suit::Hearts || c.suit == Suit::Diamonds));
        let has_convention_jacks = has_black_jack && has_red_jack;

        bids.retain(|&bid| {
            if bid == 1 {
                !partner_has_bid && aces >= 2
            } else if bid == 2 {
                !partner_has_bid && has_convention_jacks
            } else {
                true
            }
        });

        bids
    }

    fn play_options(&self) -> Vec<i32> {
        let actions: Vec<i32>;
        if let Some(lead_card) = self.lead_card {
            actions = self.hands[self.current_player]
                .iter()
                .filter(|c| c.suit == lead_card.suit)
                .map(|c| c.id)
                .collect();
            if !actions.is_empty() {
                return actions;
            }
        }

        let actions: Vec<i32> = self.hands[self.current_player]
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
            GameState::Bidding => {
                if mov == Some(0) {
                    self.bid(None);
                } else {
                    self.bid(mov);
                }
            }
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
        } else if made_it {
            trick_count
        } else {
            -bid
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

        // Store hand score from bidding team's perspective (range: -12 to 12)
        // Positive = bidder made it, Negative = bidder got set
        let raw_score = self.scores_this_hand[bidding_team] - self.scores_this_hand[defending_team];
        self.last_hand_score = Some(raw_score.clamp(-12, 12));
    }

    pub fn create_deck() -> Vec<Card> {
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

#[allow(dead_code)]
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
    c.value
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
        if hand_over {
            let team = player as usize % 2;
            let opponent_team = 1 - team;

            match self.experiment {
                0 => {
                    // Baseline: Asymmetric rewards [0.0, 1.0]
                    let score = self.scores_this_hand[team];
                    // Normalize score (-12 to 12) to 0.0 to 1.0
                    // -12 -> 0.0
                    // 0 -> 0.5
                    // 12 -> 1.0
                    let normalized_score = (score as f64 + 12.0) / 24.0;
                    Some(normalized_score.clamp(0.0, 1.0))
                }
                1 => {
                    // Experiment 1: Symmetric rewards [-1.0, 1.0] with game win/loss emphasis
                    // Check if the game is actually won or lost
                    if self.scores[team] >= self.score_threshold {
                        // My team won the game - maximum reward
                        return Some(1.0);
                    } else if self.scores[opponent_team] >= self.score_threshold {
                        // Opponent won the game - minimum reward
                        return Some(-1.0);
                    }

                    // Game is ongoing - use normalized hand score
                    let score = self.scores_this_hand[team];
                    // Normalize score (-12 to 12) to -1.0 to 1.0
                    // -12 -> -1.0
                    // 0 -> 0.0
                    // 12 -> 1.0
                    let normalized_score = (score as f64) / 12.0;
                    Some(normalized_score.clamp(-1.0, 1.0))
                }
                _ => {
                    // Default to baseline for unknown experiments
                    let score = self.scores_this_hand[team];
                    let normalized_score = (score as f64 + 12.0) / 24.0;
                    Some(normalized_score.clamp(0.0, 1.0))
                }
            }
        } else if self.use_heuristic {
            // Use heuristic ONLY at the start of the hand (to evaluate bidding decisions).
            // If we are mid-hand, we want to use Pure ISMCTS rollouts.
            let tricks_played = self.tricks_taken[0] + self.tricks_taken[1];
            let cards_in_trick = self.current_trick.iter().filter(|c| c.is_some()).count();

            if tricks_played == 0 && cards_in_trick == 0 {
                if let Some(trump) = self.trump {
                    let hand = &self.hands[player as usize];
                    let network = super::model::Network::production();
                    let bid = self.high_bid.unwrap_or(0);
                    let score = network.evaluate(hand, trump, bid);

                    // Heuristic now predicts normalized score (0.0 - 1.0) directly

                    let mut final_score = score as f64;

                    // Risk adjustment: if losing, be more optimistic about our hand if we named trump
                    let my_score = self.scores[player as usize % 2];
                    let opp_score = self.scores[(player as usize + 1) % 2];

                    if my_score < opp_score {
                        let diff = (opp_score - my_score) as f64;
                        if let Some(bidder) = self.bidder {
                            if bidder % 2 == player as usize % 2 {
                                // Add a bonus proportional to the deficit
                                final_score += diff * 0.01;
                            }
                        }
                    }

                    Some(final_score.clamp(0.0, 1.0))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn move_probabilities(&self) -> Option<Vec<(Self::Move, f64)>> {
        // Only provide policy priors if enabled and during bidding phase
        if !self.use_policy_priors || !matches!(self.state, GameState::Bidding) {
            return None;
        }

        // Load the policy network (using cached instance)
        let policy_model = get_policy_model();

        // Get the current player's hand
        let hand = &self.hands[self.current_player];

        // Get game context
        let my_team = self.current_player % 2;
        let opponent_team = 1 - my_team;
        let my_score = self.scores[my_team];
        let opponent_score = self.scores[opponent_team];
        let high_bidder = self.bids.iter().enumerate().rev().find_map(|(i, &bid)| {
            if bid.is_some() && bid != Some(0) {
                Some(i)
            } else {
                None
            }
        });

        // During bidding, trump is not yet known, so we evaluate for all possible trumps
        // and average the probabilities
        let all_suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
        let mut max_probs = [0.0f32; 8]; // 8 possible bids

        for suit in &all_suits {
            let probs = policy_model.evaluate(
                hand,
                *suit,
                my_score,
                opponent_score,
                high_bidder,
                self.current_player,
            );
            for i in 0..8 {
                if probs[i] > max_probs[i] {
                    max_probs[i] = probs[i];
                }
            }
        }

        // Get valid moves
        let valid_moves = self.get_moves();

        // Calculate total probability mass for valid moves
        let mut total_prob = 0.0;
        for &bid in &valid_moves {
            let idx = super::policy_model::bid_to_index(bid);
            total_prob += max_probs[idx] as f64;
        }

        // Create (move, probability) pairs for valid moves, renormalized
        let move_probs: Vec<(i32, f64)> = valid_moves
            .iter()
            .map(|&bid| {
                let idx = super::policy_model::bid_to_index(bid);
                let prob = max_probs[idx] as f64;
                if total_prob > 0.0 {
                    (bid, prob / total_prob)
                } else {
                    (bid, 1.0 / valid_moves.len() as f64)
                }
            })
            .collect();

        Some(move_probs)
    }
}

use super::policy_model::PolicyNetwork;
use std::sync::OnceLock;

static POLICY_MODEL: OnceLock<PolicyNetwork> = OnceLock::new();

fn get_policy_model() -> &'static PolicyNetwork {
    POLICY_MODEL.get_or_init(|| {
        const MODEL_JSON: &str = include_str!("policy_model_with_context.json");
        serde_json::from_str(MODEL_JSON).expect("Failed to parse embedded policy model")
    })
}

impl KaiboshGame {
    /// Get the best move using ISMCTS with appropriate settings
    /// Automatically applies convention heuristic during bidding if enabled
    /// Uses policy priors and value network based on game flags
    pub fn get_best_move(&self, iterations: i32) -> Option<i32> {
        let mut game_copy = self.clone();
        game_copy.score_threshold = -10000;

        let mut ismcts = if self.use_policy_priors {
            IsmctsHandler::new_with_puct(game_copy, 1.0)
        } else {
            IsmctsHandler::new(game_copy)
        };

        ismcts.run_iterations(1, iterations as usize);
        let raw_move = ismcts.best_move();

        // Apply convention heuristic if appropriate
        apply_convention_heuristic(self, raw_move)
    }
}

pub fn get_mcts_move(game: &KaiboshGame, iterations: i32) -> i32 {
    game.get_best_move(iterations)
        .expect("should have a move to make")
}

/// Apply convention bidding heuristic if appropriate
/// If ISMCTS wants to bid < 4 and partner hasn't bid yet, override with convention bid
pub fn apply_convention_heuristic(game: &KaiboshGame, best_move: Option<i32>) -> Option<i32> {
    // Only apply during bidding with heuristic enabled
    if !matches!(game.state, GameState::Bidding) || !game.use_heuristic {
        return best_move;
    }

    let best_move = best_move?;

    // Check if partner has bid yet
    let first_bidder = (game.dealer + 1) % 4;
    let relative_pos = (game.current_player + 4 - first_bidder) % 4;
    let partner_has_bid = relative_pos >= 2;

    if partner_has_bid {
        return Some(best_move);
    }

    // Only override if ISMCTS wants to bid less than 4 (not a strong bid)
    if best_move >= 4 {
        return Some(best_move);
    }

    // Check for convention cards
    let hand = &game.hands[game.current_player];
    let aces = hand.iter().filter(|c| c.value == 14).count();
    let has_black_jack = hand
        .iter()
        .any(|c| c.value == 11 && (c.suit == Suit::Clubs || c.suit == Suit::Spades));
    let has_red_jack = hand
        .iter()
        .any(|c| c.value == 11 && (c.suit == Suit::Hearts || c.suit == Suit::Diamonds));
    let has_convention_jacks = has_black_jack && has_red_jack;

    // Get available bids to ensure convention bid is valid
    let available_bids = game.bidding_options();

    // Prefer bid 2 (both jacks) over bid 1 (aces)
    // Bid 2 is more important - signals you have partner's left/right bower
    if has_convention_jacks && available_bids.contains(&2) {
        Some(2)
    } else if aces >= 2 && available_bids.contains(&1) {
        Some(1)
    } else {
        Some(best_move)
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
        // Pass (0) should always be available
        assert!(
            options.contains(&0),
            "Pass option should always be available"
        );
        // But bids 1-5 should not be available
        for i in 1..=5 {
            assert!(
                !options.contains(&i),
                "Options should not include bid lower than or equal to 5, found {}",
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

#[test]
fn test_complete_hand_end_to_end() {
    let mut game = KaiboshGame::new();

    // Manually set up a simple hand to control the outcome
    // Player 0 bids 3, wins 3 tricks, scores 3
    game.hands[0] = vec![
        Card {
            value: 14,
            suit: Suit::Hearts,
            id: 0,
        }, // A♥
        Card {
            value: 13,
            suit: Suit::Hearts,
            id: 1,
        }, // K♥
        Card {
            value: 12,
            suit: Suit::Hearts,
            id: 2,
        }, // Q♥
        Card {
            value: 11,
            suit: Suit::Spades,
            id: 3,
        }, // J♠
        Card {
            value: 10,
            suit: Suit::Spades,
            id: 4,
        }, // 10♠
        Card {
            value: 9,
            suit: Suit::Spades,
            id: 5,
        }, // 9♠
    ];
    game.hands[1] = vec![
        Card {
            value: 11,
            suit: Suit::Hearts,
            id: 6,
        }, // J♥ (right bower if H is trump)
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 7,
        }, // 10♥
        Card {
            value: 9,
            suit: Suit::Hearts,
            id: 8,
        }, // 9♥
        Card {
            value: 14,
            suit: Suit::Clubs,
            id: 9,
        }, // A♣
        Card {
            value: 13,
            suit: Suit::Clubs,
            id: 10,
        }, // K♣
        Card {
            value: 12,
            suit: Suit::Clubs,
            id: 11,
        }, // Q♣
    ];
    game.hands[2] = vec![
        Card {
            value: 14,
            suit: Suit::Diamonds,
            id: 12,
        }, // A♦
        Card {
            value: 13,
            suit: Suit::Diamonds,
            id: 13,
        }, // K♦
        Card {
            value: 12,
            suit: Suit::Diamonds,
            id: 14,
        }, // Q♦
        Card {
            value: 11,
            suit: Suit::Diamonds,
            id: 15,
        }, // J♦ (left bower if H is trump)
        Card {
            value: 10,
            suit: Suit::Diamonds,
            id: 16,
        }, // 10♦
        Card {
            value: 9,
            suit: Suit::Diamonds,
            id: 17,
        }, // 9♦
    ];
    game.hands[3] = vec![
        Card {
            value: 11,
            suit: Suit::Clubs,
            id: 18,
        }, // J♣
        Card {
            value: 10,
            suit: Suit::Clubs,
            id: 19,
        }, // 10♣
        Card {
            value: 9,
            suit: Suit::Clubs,
            id: 20,
        }, // 9♣
        Card {
            value: 14,
            suit: Suit::Spades,
            id: 21,
        }, // A♠
        Card {
            value: 13,
            suit: Suit::Spades,
            id: 22,
        }, // K♠
        Card {
            value: 12,
            suit: Suit::Spades,
            id: 23,
        }, // Q♠
    ];

    // Bidding phase: Player 0 bids 3, others pass
    game.bid(Some(3)); // Player 0 bids 3
    game.bid(None); // Player 1 passes
    game.bid(None); // Player 2 passes
    game.bid(None); // Player 3 passes

    assert_eq!(game.state, GameState::NameTrump);
    assert_eq!(game.bidder, Some(0));
    assert_eq!(game.high_bid, Some(3));

    // Name trump: Player 0 chooses Hearts
    game.name_trump(2); // 2 = Hearts
    assert_eq!(game.trump, Some(Suit::Hearts));
    assert_eq!(game.state, GameState::Play);

    // Play 6 tricks - player 0 leads
    // Track initial state
    let initial_scores = game.scores;
    let initial_scores_this_hand = game.scores_this_hand;

    // Play through all 24 cards (6 tricks × 4 players)
    for trick_num in 0..6 {
        for _ in 0..4 {
            let moves = game.get_moves();
            assert!(
                !moves.is_empty(),
                "Should have moves available on trick {}",
                trick_num
            );
            game.apply_move(Some(moves[0]));
        }
    }

    // Verify hand completed
    assert!(
        game.hands.iter().all(|h| h.is_empty()),
        "All hands should be empty"
    );
    assert!(
        game.last_hand_score.is_some(),
        "last_hand_score should be set"
    );

    // Verify scores were calculated
    let final_scores = game.scores;
    let final_scores_this_hand = game.scores_this_hand;

    println!("Initial scores: {:?}", initial_scores);
    println!("Final scores: {:?}", final_scores);
    println!("Initial scores_this_hand: {:?}", initial_scores_this_hand);
    println!("Final scores_this_hand: {:?}", final_scores_this_hand);
    println!("last_hand_score: {:?}", game.last_hand_score);
    println!("tricks_taken: {:?}", game.tricks_taken);

    // Verify score change happened
    assert_ne!(
        final_scores_this_hand,
        [0, 0],
        "scores_this_hand should not be [0,0] after hand completes"
    );

    // Verify last_hand_score matches scores_this_hand
    let expected_hand_score = final_scores_this_hand[0] - final_scores_this_hand[1];
    assert_eq!(
        game.last_hand_score,
        Some(expected_hand_score),
        "last_hand_score should equal scores_this_hand[0] - scores_this_hand[1]"
    );
}

#[test]
fn test_kaibosh_hand_end_to_end() {
    let mut game = KaiboshGame::new();

    // Set up a hand where player 0 can bid and make KAIBOSH
    // Give player 0 all trump (Hearts) including both bowers
    game.hands[0] = vec![
        Card {
            value: 11,
            suit: Suit::Hearts,
            id: 0,
        }, // J♥ (right bower)
        Card {
            value: 11,
            suit: Suit::Diamonds,
            id: 1,
        }, // J♦ (left bower)
        Card {
            value: 14,
            suit: Suit::Hearts,
            id: 2,
        }, // A♥
        Card {
            value: 13,
            suit: Suit::Hearts,
            id: 3,
        }, // K♥
        Card {
            value: 12,
            suit: Suit::Hearts,
            id: 4,
        }, // Q♥
        Card {
            value: 10,
            suit: Suit::Hearts,
            id: 5,
        }, // 10♥
    ];

    // Give other players weak hands
    for player in 1..4 {
        let base_id = (6 + player * 6) as i32;
        game.hands[player] = vec![
            Card {
                value: 9,
                suit: Suit::Clubs,
                id: base_id,
            },
            Card {
                value: 10,
                suit: Suit::Clubs,
                id: base_id + 1,
            },
            Card {
                value: 12,
                suit: Suit::Clubs,
                id: base_id + 2,
            },
            Card {
                value: 9,
                suit: Suit::Spades,
                id: base_id + 3,
            },
            Card {
                value: 10,
                suit: Suit::Spades,
                id: base_id + 4,
            },
            Card {
                value: 12,
                suit: Suit::Spades,
                id: base_id + 5,
            },
        ];
    }

    // Bidding: Player 0 bids KAIBOSH (12), others pass
    game.bid(Some(KAIBOSH));
    assert_eq!(
        game.state,
        GameState::NameTrump,
        "KAIBOSH bid should go straight to NameTrump"
    );

    // Name trump
    game.name_trump(2); // 2 = Hearts
    assert_eq!(game.trump, Some(Suit::Hearts));

    // Play through all 6 tricks
    for _ in 0..24 {
        let moves = game.get_moves();
        if !moves.is_empty() {
            game.apply_move(Some(moves[0]));
        }
    }

    // Verify KAIBOSH scoring
    assert!(game.last_hand_score.is_some());

    println!("KAIBOSH test - last_hand_score: {:?}", game.last_hand_score);
    println!(
        "KAIBOSH test - scores_this_hand: {:?}",
        game.scores_this_hand
    );
    println!("KAIBOSH test - tricks_taken: {:?}", game.tricks_taken);

    // If player 0 won all 6 tricks, should score +12
    // If player 0 failed, should score -12
    let score = game.last_hand_score.unwrap();
    assert!(
        score == 12 || score == -12,
        "KAIBOSH score should be exactly +12 or -12, got {}",
        score
    );
}
