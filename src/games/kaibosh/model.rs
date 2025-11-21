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

        for (j, output) in hidden_outputs.iter_mut().enumerate() {
            let mut sum = self.hidden_bias[j];
            for (i, &input) in inputs.iter().enumerate() {
                sum += input * self.hidden_weights[i][j];
            }
            // ReLU activation
            *output = if sum > 0.0 { sum } else { 0.0 };
        }

        let mut final_output = self.output_bias;
        for (j, &hidden) in hidden_outputs.iter().enumerate() {
            final_output += hidden * self.output_weights[j];
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

        let mut final_sum = self.output_bias;
        for (j, &hidden) in hidden_outputs.iter().enumerate() {
            final_sum += hidden * self.output_weights[j];
        }

        let predicted = 1.0 / (1.0 + (-final_sum).exp()); // Sigmoid

        // Backpropagation
        let output_error = predicted - target;
        let delta_output = output_error * predicted * (1.0 - predicted);

        // Update output weights and bias
        self.output_bias -= learning_rate * delta_output;

        let mut delta_hidden = vec![0.0; self.hidden_size];
        for (j, (delta, &h_input)) in delta_hidden
            .iter_mut()
            .zip(hidden_inputs.iter())
            .enumerate()
        {
            let error_wrt_hidden = delta_output * self.output_weights[j];
            let relu_prime = if h_input > 0.0 { 1.0 } else { 0.0 };
            *delta = error_wrt_hidden * relu_prime;
        }

        // Now update weights
        for (&hidden, weight) in hidden_outputs.iter().zip(self.output_weights.iter_mut()) {
            *weight -= learning_rate * delta_output * hidden;
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
