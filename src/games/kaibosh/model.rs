use super::features::Features;
use super::game::{Card, Suit};
use rand::Rng;
use serde::{Deserialize, Serialize};

// Neural Network with 1 hidden layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub input_size: usize,
    pub hidden_size: usize,
    pub hidden_weights: Vec<Vec<f32>>, // [input_size][hidden_size]
    pub hidden_bias: Vec<f32>,         // [hidden_size]
    pub output_weights: Vec<f32>,      // [hidden_size]
    pub output_bias: f32,
}

impl Network {
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        let mut rng = rand::thread_rng();

        let hidden_weights = (0..input_size)
            .map(|_| (0..hidden_size).map(|_| rng.gen_range(-0.1..0.1)).collect())
            .collect();

        let hidden_bias = (0..hidden_size).map(|_| rng.gen_range(-0.1..0.1)).collect();

        let output_weights = (0..hidden_size).map(|_| rng.gen_range(-0.1..0.1)).collect();

        Self {
            input_size,
            hidden_size,
            hidden_weights,
            hidden_bias,
            output_weights,
            output_bias: rng.gen_range(-0.1..0.1),
        }
    }

    pub fn production() -> Self {
        let json = include_str!("model.json");
        serde_json::from_str(json).unwrap_or_else(|_| Self::new(13, 64))
    }

    pub fn evaluate(&self, hand: &[Card], trump: Suit, bid: i32) -> f32 {
        let features = Features::from_hand(hand, trump);
        let inputs = features.to_vec_with_bid(bid);

        // Forward pass
        let mut hidden_outputs = vec![0.0; self.hidden_size];

        for j in 0..self.hidden_size {
            let mut sum = self.hidden_bias[j];
            for i in 0..self.input_size {
                sum += inputs[i] * self.hidden_weights[i][j];
            }
            // ReLU activation
            hidden_outputs[j] = if sum > 0.0 { sum } else { 0.0 };
        }

        let mut final_output = self.output_bias;
        for j in 0..self.hidden_size {
            final_output += hidden_outputs[j] * self.output_weights[j];
        }

        // Sigmoid activation for output (0.0 - 1.0)
        1.0 / (1.0 + (-final_output).exp())
    }

    pub fn train(&mut self, hand: &[Card], trump: Suit, bid: i32, target: f32, learning_rate: f32) {
        let features = Features::from_hand(hand, trump);
        let inputs = features.to_vec_with_bid(bid);

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

        let mut final_sum = self.output_bias;
        for j in 0..self.hidden_size {
            final_sum += hidden_outputs[j] * self.output_weights[j];
        }

        let predicted = 1.0 / (1.0 + (-final_sum).exp()); // Sigmoid

        // Backpropagation
        let output_error = predicted - target;
        let delta_output = output_error * predicted * (1.0 - predicted);

        // Update output weights and bias
        self.output_bias -= learning_rate * delta_output;

        let mut delta_hidden = vec![0.0; self.hidden_size];
        for j in 0..self.hidden_size {
            let error_wrt_hidden = delta_output * self.output_weights[j];
            let relu_prime = if hidden_inputs[j] > 0.0 { 1.0 } else { 0.0 };
            delta_hidden[j] = error_wrt_hidden * relu_prime;
        }

        // Now update weights
        for j in 0..self.hidden_size {
            self.output_weights[j] -= learning_rate * delta_output * hidden_outputs[j];
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
}
