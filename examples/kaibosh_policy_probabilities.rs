use tricksterstable_rs::games::kaibosh::game::{Card, KaiboshGame, Suit};
use tricksterstable_rs::games::kaibosh::policy_model::{bid_to_index, index_to_bid, PolicyNetwork};

fn main() {
    println!("=== Policy Network Probability Test ===\n");

    // Load the policy network
    let policy_model = PolicyNetwork::from_file(
        "/Users/dbravender/projects/tricksterstable-rs/src/games/kaibosh/policy_model_with_context.json",
    );

    println!(
        "Model loaded: {} inputs, {} hidden, {} outputs\n",
        policy_model.input_size, policy_model.hidden_size, policy_model.output_size
    );

    // Test with a random game
    let game = KaiboshGame::new();
    let hand = &game.hands[0];

    println!("Test hand: {}", format_hand(hand));
    println!();

    // Test for each suit
    for suit in &[Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
        let probs = policy_model.evaluate(hand, *suit, 0, 0, None, 0);

        println!("Trump: {:?}", suit);
        for i in 0..probs.len() {
            let bid = index_to_bid(i);
            let bid_str = if bid == 0 {
                "Pass".to_string()
            } else if bid == 12 {
                "KAIBOSH".to_string()
            } else {
                bid.to_string()
            };
            println!("  {}: {:.1}%", bid_str, probs[i] * 100.0);
        }
        println!();
    }

    // Test perfect Kaibosh hand
    let perfect_hand = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        }, // Right bower
        Card {
            id: 1,
            value: 11,
            suit: Suit::Diamonds,
        }, // Left bower
        Card {
            id: 2,
            value: 14,
            suit: Suit::Hearts,
        }, // A♥
        Card {
            id: 3,
            value: 13,
            suit: Suit::Hearts,
        }, // K♥
        Card {
            id: 4,
            value: 12,
            suit: Suit::Hearts,
        }, // Q♥
        Card {
            id: 5,
            value: 14,
            suit: Suit::Spades,
        }, // A♠
    ];

    println!("\n=== Perfect Kaibosh Hand - ALL SUITS ===");
    println!("Hand: {}", format_hand(&perfect_hand));
    println!();

    for suit in &[Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
        let probs = policy_model.evaluate(&perfect_hand, *suit, 0, 0, None, 0);
        println!("Trump: {:?}", suit);
        for i in 0..probs.len() {
            let bid = index_to_bid(i);
            let bid_str = if bid == 0 {
                "Pass".to_string()
            } else if bid == 12 {
                "KAIBOSH".to_string()
            } else {
                bid.to_string()
            };
            if probs[i] > 0.01 {
                println!("  {}: {:.1}%", bid_str, probs[i] * 100.0);
            }
        }
        println!();
    }

    // Show max probabilities across all suits (like move_probabilities does)
    println!("=== MAX PROBABILITIES ACROSS ALL SUITS ===");
    let mut max_probs = [0.0f32; 8];
    for suit in &[Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades] {
        let probs = policy_model.evaluate(&perfect_hand, *suit, 0, 0, None, 0);
        for i in 0..8 {
            if probs[i] > max_probs[i] {
                max_probs[i] = probs[i];
            }
        }
    }
    for i in 0..max_probs.len() {
        let bid = index_to_bid(i);
        let bid_str = if bid == 0 {
            "Pass".to_string()
        } else if bid == 12 {
            "KAIBOSH".to_string()
        } else {
            bid.to_string()
        };
        if max_probs[i] > 0.01 {
            println!("  {}: {:.1}%", bid_str, max_probs[i] * 100.0);
        }
    }
}

fn format_hand(hand: &[Card]) -> String {
    hand.iter()
        .map(|c| format!("{}{}", value_to_str(c.value), suit_to_str(&c.suit)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn value_to_str(value: i32) -> String {
    match value {
        14 => "A".to_string(),
        13 => "K".to_string(),
        12 => "Q".to_string(),
        11 => "J".to_string(),
        10 => "10".to_string(),
        9 => "9".to_string(),
        _ => value.to_string(),
    }
}

fn suit_to_str(suit: &Suit) -> &str {
    match suit {
        Suit::Hearts => "♥",
        Suit::Diamonds => "♦",
        Suit::Clubs => "♣",
        Suit::Spades => "♠",
    }
}
