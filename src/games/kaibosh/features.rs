use super::game::{Card, Color, Suit};

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
    pub score_differential: f32, // (my team score - opponent score) / 25
    pub opponent_has_bid: f32,   // 1.0 if opponent has current high bid, 0.0 otherwise
    pub score_behind: f32,       // max(0, opponent_score - my_score) / 25 - desperation when losing
    pub opponent_near_win: f32,  // opponent_score / 25 - urgency when opponent is close to winning
}

impl Features {
    /// Extract hand features given a trump suit
    pub fn from_hand(hand: &[Card], trump: Suit) -> Self {
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

        features
    }

    /// Convert features to vector (for policy network - 11 features)
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
        ]
    }

    /// Convert features to vector with additional bid and bias inputs (for value network - 13 features)
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
            bid as f32 / 12.0, // Normalized bid
            1.0,               // Bias input
        ]
    }
}
