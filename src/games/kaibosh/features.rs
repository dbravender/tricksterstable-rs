use super::game::{Card, Color, Suit, DEFAULT_SCORE_THRESHOLD};

/// Hand features used for neural network evaluation
#[derive(Default, Debug, Clone)]
pub struct Features {
    pub trump: f32,
    pub right_bower: f32,
    pub left_bower: f32,
    pub ace: f32,
    pub king: f32,
    pub queen: f32,
    pub jack: f32,
    pub ten: f32,
    pub nine: f32,
    pub two_aces: f32,
    pub two_jacks: f32,
    pub score_differential: f32, // (my team score - opponent score) / DEFAULT_SCORE_THRESHOLD
    pub opponent_has_bid: f32,   // 1.0 if opponent has current high bid, 0.0 otherwise
    pub score_behind: f32, // max(0, opponent_score - my_score) / DEFAULT_SCORE_THRESHOLD - desperation when losing
    pub opponent_near_win: f32, // opponent_score / DEFAULT_SCORE_THRESHOLD - urgency when opponent is close to winning
}

impl Features {
    /// Extract hand features given a trump suit (without game context)
    pub fn from_hand(hand: &[Card], trump: Suit) -> Self {
        Self::from_hand_with_context(hand, trump, 0, 0, None, 0)
    }

    /// Extract hand features with game context for strategic bidding
    pub fn from_hand_with_context(
        hand: &[Card],
        trump: Suit,
        my_score: i32,
        opponent_score: i32,
        high_bidder: Option<usize>,
        current_player: usize,
    ) -> Self {
        let mut features = Features::default();

        let mut trump_count = 0.0;
        let mut right_bower = 0.0;
        let mut left_bower = 0.0;
        let mut ace = 0.0;
        let mut king = 0.0;
        let mut queen = 0.0;
        let mut jack = 0.0;
        let mut ten = 0.0;
        let mut nine = 0.0;
        let mut aces = 0;

        let left_bower_suit = trump.same_color_suit();

        for card in hand {
            if card.suit == trump {
                trump_count += 1.0;
            }

            if card.value == 11 && card.suit == trump {
                right_bower = 1.0;
            } else if card.value == 11 && card.suit == left_bower_suit {
                left_bower = 1.0;
            } else if card.value == 14 {
                ace += 1.0;
                aces += 1;
            } else if card.value == 13 {
                king += 1.0;
            } else if card.value == 12 {
                queen += 1.0;
            } else if card.value == 11 {
                jack += 1.0;
            } else if card.value == 10 {
                ten += 1.0;
            } else if card.value == 9 {
                nine += 1.0;
            }
        }

        let has_black_jack = hand
            .iter()
            .any(|c| c.value == 11 && c.suit.color() == Color::Black);
        let has_red_jack = hand
            .iter()
            .any(|c| c.value == 11 && c.suit.color() == Color::Red);
        let two_jacks = if has_black_jack && has_red_jack {
            1.0
        } else {
            0.0
        };

        let two_aces = if aces >= 2 { 1.0 } else { 0.0 };

        features.trump = trump_count;
        features.right_bower = right_bower;
        features.left_bower = left_bower;
        features.ace = ace;
        features.king = king;
        features.queen = queen;
        features.jack = jack;
        features.ten = ten;
        features.nine = nine;
        features.two_aces = two_aces;
        features.two_jacks = two_jacks;

        // Calculate game context features
        let my_team = current_player % 2;
        let opponent_team = 1 - my_team;

        // Score differential: positive when winning, negative when losing
        features.score_differential =
            (my_score - opponent_score) as f32 / DEFAULT_SCORE_THRESHOLD as f32;

        // Opponent has the current high bid (need to outbid or steal trump naming)
        features.opponent_has_bid = if let Some(bidder) = high_bidder {
            if bidder % 2 == opponent_team {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Desperation factor: how far behind we are (only positive when losing)
        features.score_behind =
            ((opponent_score - my_score).max(0) as f32) / DEFAULT_SCORE_THRESHOLD as f32;

        // Urgency factor: how close opponent is to winning
        features.opponent_near_win = (opponent_score as f32) / DEFAULT_SCORE_THRESHOLD as f32;

        features
    }

    /// Convert features to vector (for policy network - 15 features)
    pub fn to_vec(&self) -> Vec<f32> {
        vec![
            self.trump,
            self.right_bower,
            self.left_bower,
            self.ace,
            self.king,
            self.queen,
            self.jack,
            self.ten,
            self.nine,
            self.two_aces,
            self.two_jacks,
            self.score_differential,
            self.opponent_has_bid,
            self.score_behind,
            self.opponent_near_win,
        ]
    }

    /// Convert features to vector with additional bid and bias inputs (for value network - 17 features)
    pub fn to_vec_with_bid(&self, bid: i32) -> Vec<f32> {
        vec![
            self.trump,
            self.right_bower,
            self.left_bower,
            self.ace,
            self.king,
            self.queen,
            self.jack,
            self.ten,
            self.nine,
            self.two_aces,
            self.two_jacks,
            self.score_differential,
            self.opponent_has_bid,
            self.score_behind,
            self.opponent_near_win,
            bid as f32 / 12.0, // Normalized bid
            1.0,               // Bias input
        ]
    }
}
