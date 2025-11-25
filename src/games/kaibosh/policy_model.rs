use super::features::Features;
use super::game::{Card, Suit};
use rand::Rng;
use serde::{Deserialize, Serialize};

// Policy Network - outputs probabilities for each bid action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyNetwork {
    pub input_size: usize,
    pub hidden_size: usize,
    pub output_size: usize,            // 8 bid options: Pass, 1-6, Kaibosh
    pub hidden_weights: Vec<Vec<f32>>, // [input_size][hidden_size]
    pub hidden_bias: Vec<f32>,         // [hidden_size]
    pub output_weights: Vec<Vec<f32>>, // [hidden_size][output_size]
    pub output_bias: Vec<f32>,         // [output_size]
}

impl PolicyNetwork {
    pub fn new(input_size: usize, hidden_size: usize, output_size: usize) -> Self {
        let mut rng = rand::thread_rng();

        let hidden_weights = (0..input_size)
            .map(|_| (0..hidden_size).map(|_| rng.gen_range(-0.1..0.1)).collect())
            .collect();

        let hidden_bias = (0..hidden_size).map(|_| rng.gen_range(-0.1..0.1)).collect();

        let output_weights = (0..hidden_size)
            .map(|_| (0..output_size).map(|_| rng.gen_range(-0.1..0.1)).collect())
            .collect();

        let output_bias = (0..output_size).map(|_| rng.gen_range(-0.1..0.1)).collect();

        Self {
            input_size,
            hidden_size,
            output_size,
            hidden_weights,
            hidden_bias,
            output_weights,
            output_bias,
        }
    }

    pub fn from_file(path: &str) -> Self {
        let json = std::fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("Could not read model file: {}", path));
        serde_json::from_str(&json).unwrap_or_else(|_| Self::new(11, 64, 8))
    }

    /// Evaluate hand and return probabilities for each bid
    /// Returns: [P(Pass), P(1), P(2), P(3), P(4), P(5), P(6), P(Kaibosh)]
    pub fn evaluate(
        &self,
        hand: &[Card],
        trump: Suit,
        my_score: i32,
        opponent_score: i32,
        high_bidder: Option<usize>,
        current_player: usize,
    ) -> Vec<f32> {
        let features = Features::from_hand_with_context(
            hand,
            trump,
            my_score,
            opponent_score,
            high_bidder,
            current_player,
        );
        let mut inputs = features.to_vec();

        // Handle model/feature size mismatch
        // If model expects fewer inputs, truncate; if more, pad with zeros
        match inputs.len().cmp(&self.input_size) {
            std::cmp::Ordering::Less => inputs.resize(self.input_size, 0.0),
            std::cmp::Ordering::Greater => inputs.truncate(self.input_size),
            std::cmp::Ordering::Equal => {}
        }

        // Forward pass - hidden layer
        let mut hidden_outputs = vec![0.0; self.hidden_size];
        for (j, output) in hidden_outputs.iter_mut().enumerate() {
            let mut sum = self.hidden_bias[j];
            for (i, &input) in inputs.iter().enumerate() {
                sum += input * self.hidden_weights[i][j];
            }
            *output = if sum > 0.0 { sum } else { 0.0 }; // ReLU
        }

        // Forward pass - output layer
        let mut logits = vec![0.0; self.output_size];
        for (k, logit) in logits.iter_mut().enumerate() {
            let mut sum = self.output_bias[k];
            for (j, &hidden) in hidden_outputs.iter().enumerate() {
                sum += hidden * self.output_weights[j][k];
            }
            *logit = sum;
        }

        // Softmax activation
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&x| (x - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        exp_logits.iter().map(|&x| x / sum_exp).collect()
    }

    /// Train on a single example using cross-entropy loss
    #[allow(clippy::too_many_arguments)]
    pub fn train(
        &mut self,
        hand: &[Card],
        trump: Suit,
        my_score: i32,
        opponent_score: i32,
        high_bidder: Option<usize>,
        current_player: usize,
        target_bid: i32,
        learning_rate: f32,
    ) {
        let features = Features::from_hand_with_context(
            hand,
            trump,
            my_score,
            opponent_score,
            high_bidder,
            current_player,
        );
        let mut inputs = features.to_vec();

        // Handle model/feature size mismatch
        match inputs.len().cmp(&self.input_size) {
            std::cmp::Ordering::Less => inputs.resize(self.input_size, 0.0),
            std::cmp::Ordering::Greater => inputs.truncate(self.input_size),
            std::cmp::Ordering::Equal => {}
        }

        // Create one-hot target
        let target_index = bid_to_index(target_bid);
        let mut target = vec![0.0; self.output_size];
        target[target_index] = 1.0;

        // Forward pass
        let mut hidden_inputs = vec![0.0; self.hidden_size];
        let mut hidden_outputs = vec![0.0; self.hidden_size];

        for (j, (h_input, h_output)) in hidden_inputs
            .iter_mut()
            .zip(hidden_outputs.iter_mut())
            .enumerate()
        {
            let mut sum = self.hidden_bias[j];
            for (i, &input) in inputs.iter().enumerate() {
                sum += input * self.hidden_weights[i][j];
            }
            *h_input = sum;
            *h_output = if sum > 0.0 { sum } else { 0.0 }; // ReLU
        }

        let mut logits = vec![0.0; self.output_size];
        for (k, logit) in logits.iter_mut().enumerate() {
            let mut sum = self.output_bias[k];
            for (j, &hidden) in hidden_outputs.iter().enumerate() {
                sum += hidden * self.output_weights[j][k];
            }
            *logit = sum;
        }

        // Softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&x| (x - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        let probabilities: Vec<f32> = exp_logits.iter().map(|&x| x / sum_exp).collect();

        // Backpropagation
        // Output layer gradients (cross-entropy + softmax derivative)
        let delta_output: Vec<f32> = probabilities
            .iter()
            .zip(target.iter())
            .map(|(&prob, &tgt)| prob - tgt)
            .collect();

        // Hidden layer gradients
        let mut delta_hidden = vec![0.0; self.hidden_size];
        for (j, (delta, &h_input)) in delta_hidden
            .iter_mut()
            .zip(hidden_inputs.iter())
            .enumerate()
        {
            let mut error = 0.0;
            for (k, &d_out) in delta_output.iter().enumerate() {
                error += d_out * self.output_weights[j][k];
            }
            let relu_prime = if h_input > 0.0 { 1.0 } else { 0.0 };
            *delta = error * relu_prime;
        }

        // Update weights
        for (j, &hidden) in hidden_outputs.iter().enumerate() {
            for (k, &d_out) in delta_output.iter().enumerate() {
                self.output_weights[j][k] -= learning_rate * d_out * hidden;
            }
        }

        for (bias, &d_out) in self.output_bias.iter_mut().zip(delta_output.iter()) {
            *bias -= learning_rate * d_out;
        }

        for (i, &input) in inputs.iter().enumerate() {
            for (j, &delta) in delta_hidden.iter().enumerate() {
                self.hidden_weights[i][j] -= learning_rate * delta * input;
            }
        }

        for (bias, &delta) in self.hidden_bias.iter_mut().zip(delta_hidden.iter()) {
            *bias -= learning_rate * delta;
        }
    }
}

/// Convert bid value to array index
/// Pass -> 0, 1 -> 1, 2 -> 2, ..., 6 -> 6, 12 (Kaibosh) -> 7
pub fn bid_to_index(bid: i32) -> usize {
    match bid {
        0 => 0, // Pass
        1..=6 => bid as usize,
        12 => 7,  // Kaibosh
        100 => 0, // Misdeal treated as Pass
        _ => panic!("Invalid bid: {}", bid),
    }
}

/// Convert array index to bid value
pub fn index_to_bid(index: usize) -> i32 {
    match index {
        0 => 0, // Pass
        1..=6 => index as i32,
        7 => 12, // Kaibosh
        _ => panic!("Invalid index: {}", index),
    }
}
