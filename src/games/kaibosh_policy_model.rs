use crate::features::Features;
use crate::kaibosh::{Card, Suit};
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
        serde_json::from_str(&json).unwrap_or_else(|_| Self::new(14, 64, 8))
    }

    /// Load from embedded JSON string (for mobile/embedded deployments)
    pub fn from_json_str(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_else(|_| Self::new(14, 64, 8))
    }

    /// Load the default embedded model (compiled into the binary)
    pub fn embedded() -> Self {
        const MODEL_JSON: &str = include_str!("policy_model_superhuman.json");
        Self::from_json_str(MODEL_JSON)
    }

    /// Evaluate hand and return probabilities for each bid
    /// Returns: [P(Pass), P(1), P(2), P(3), P(4), P(5), P(6), P(Kaibosh)]
    pub fn evaluate(&self, hand: &[Card], trump: Suit) -> Vec<f32> {
        let features = Features::from_hand(hand, trump);
        self.evaluate_features(&features)
    }

    /// Evaluate using pre-built features (allows adding bidding context)
    pub fn evaluate_features(&self, features: &Features) -> Vec<f32> {
        let inputs = features.to_vec();

        // Forward pass - hidden layer
        let mut hidden_outputs = vec![0.0; self.hidden_size];
        for j in 0..self.hidden_size {
            let mut sum = self.hidden_bias[j];
            for i in 0..self.input_size {
                sum += inputs[i] * self.hidden_weights[i][j];
            }
            hidden_outputs[j] = if sum > 0.0 { sum } else { 0.0 }; // ReLU
        }

        // Forward pass - output layer
        let mut logits = vec![0.0; self.output_size];
        for k in 0..self.output_size {
            let mut sum = self.output_bias[k];
            for j in 0..self.hidden_size {
                sum += hidden_outputs[j] * self.output_weights[j][k];
            }
            logits[k] = sum;
        }

        // Softmax activation
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&x| (x - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        exp_logits.iter().map(|&x| x / sum_exp).collect()
    }

    /// Train on a single example using cross-entropy loss
    pub fn train(&mut self, hand: &[Card], trump: Suit, target_bid: i32, learning_rate: f32) {
        let features = Features::from_hand(hand, trump);
        let inputs = features.to_vec();

        // Create one-hot target
        let target_index = bid_to_index(target_bid);
        let mut target = vec![0.0; self.output_size];
        target[target_index] = 1.0;

        // Forward pass
        let mut hidden_inputs = vec![0.0; self.hidden_size];
        let mut hidden_outputs = vec![0.0; self.hidden_size];

        for j in 0..self.hidden_size {
            let mut sum = self.hidden_bias[j];
            for i in 0..self.input_size {
                sum += inputs[i] * self.hidden_weights[i][j];
            }
            hidden_inputs[j] = sum;
            hidden_outputs[j] = if sum > 0.0 { sum } else { 0.0 }; // ReLU
        }

        let mut logits = vec![0.0; self.output_size];
        for k in 0..self.output_size {
            let mut sum = self.output_bias[k];
            for j in 0..self.hidden_size {
                sum += hidden_outputs[j] * self.output_weights[j][k];
            }
            logits[k] = sum;
        }

        // Softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&x| (x - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        let probabilities: Vec<f32> = exp_logits.iter().map(|&x| x / sum_exp).collect();

        // Backpropagation
        // Output layer gradients (cross-entropy + softmax derivative)
        let mut delta_output = vec![0.0; self.output_size];
        for k in 0..self.output_size {
            delta_output[k] = probabilities[k] - target[k];
        }

        // Hidden layer gradients
        let mut delta_hidden = vec![0.0; self.hidden_size];
        for j in 0..self.hidden_size {
            let mut error = 0.0;
            for k in 0..self.output_size {
                error += delta_output[k] * self.output_weights[j][k];
            }
            let relu_prime = if hidden_inputs[j] > 0.0 { 1.0 } else { 0.0 };
            delta_hidden[j] = error * relu_prime;
        }

        // Update weights
        for j in 0..self.hidden_size {
            for k in 0..self.output_size {
                self.output_weights[j][k] -= learning_rate * delta_output[k] * hidden_outputs[j];
            }
        }

        for k in 0..self.output_size {
            self.output_bias[k] -= learning_rate * delta_output[k];
        }

        for i in 0..self.input_size {
            for j in 0..self.hidden_size {
                self.hidden_weights[i][j] -= learning_rate * delta_hidden[j] * inputs[i];
            }
        }

        for j in 0..self.hidden_size {
            self.hidden_bias[j] -= learning_rate * delta_hidden[j];
        }
    }

    /// Save model to JSON file
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
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
