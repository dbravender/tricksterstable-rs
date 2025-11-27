use tricksterstable_rs::games::kaibosh::game::{Card, KaiboshGame, Suit};

fn main() {
    println!("=== ISMCTS Kaibosh Diagnostic ===\n");

    // Test 1: Perfect Kaibosh hand
    let mut game = KaiboshGame::new();
    let perfect_hand = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        }, // Right bower (J♥)
        Card {
            id: 1,
            value: 11,
            suit: Suit::Diamonds,
        }, // Left bower (J♦)
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
    game.hands[0] = perfect_hand.clone();

    println!("Test 1: Perfect Kaibosh hand (J♥ J♦ A♥ K♥ Q♥ A♠)");
    println!("100 iterations:");
    test_hand_with_ismcts(&mut game, "Perfect hand", 100);
    println!("500 iterations:");
    test_hand_with_ismcts(&mut game, "Perfect hand", 500);
    println!("1000 iterations:");
    test_hand_with_ismcts(&mut game, "Perfect hand", 1000);

    // Test 2: Strong but not perfect hand
    let mut game = KaiboshGame::new();
    let strong_hand = vec![
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
            value: 10,
            suit: Suit::Hearts,
        }, // 10♥
        Card {
            id: 5,
            value: 10,
            suit: Suit::Spades,
        }, // 10♠
    ];
    game.hands[0] = strong_hand.clone();

    println!("\nTest 2: Strong hand (J♥ J♦ A♥ K♥ 10♥ 10♠)");
    test_hand_with_ismcts(&mut game, "Strong hand", 100);

    // Test 3: Medium hand
    let mut game = KaiboshGame::new();
    let medium_hand = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        }, // J♥
        Card {
            id: 1,
            value: 14,
            suit: Suit::Hearts,
        }, // A♥
        Card {
            id: 2,
            value: 10,
            suit: Suit::Hearts,
        }, // 10♥
        Card {
            id: 3,
            value: 9,
            suit: Suit::Hearts,
        }, // 9♥
        Card {
            id: 4,
            value: 10,
            suit: Suit::Clubs,
        }, // 10♣
        Card {
            id: 5,
            value: 12,
            suit: Suit::Spades,
        }, // Q♠
    ];
    game.hands[0] = medium_hand.clone();

    println!("\nTest 3: Medium hand (J♥ A♥ 10♥ 9♥ 10♣ Q♠)");
    test_hand_with_ismcts(&mut game, "Medium hand", 100);
}

fn test_hand_with_ismcts(game: &mut KaiboshGame, label: &str, iterations: usize) {
    // Test with policy priors
    let mut game_with_policy = game.clone();
    game_with_policy.use_policy_priors = true;

    let mut ismcts_with_policy = ismcts::IsmctsHandler::new_with_puct(game_with_policy, 1.0);
    ismcts_with_policy.run_iterations(1, iterations);
    let bid_with_policy = ismcts_with_policy.best_move();

    // Test without policy priors
    let mut game_no_policy = game.clone();
    game_no_policy.use_policy_priors = false;

    let mut ismcts_no_policy = ismcts::IsmctsHandler::new(game_no_policy);
    ismcts_no_policy.run_iterations(1, iterations);
    let bid_no_policy = ismcts_no_policy.best_move();

    println!(
        "  With policy priors: bid = {:?}",
        bid_to_string(bid_with_policy)
    );
    println!(
        "  Without policy priors: bid = {:?}",
        bid_to_string(bid_no_policy)
    );
}

fn bid_to_string(bid: Option<i32>) -> String {
    match bid {
        Some(0) => "Pass".to_string(),
        Some(12) => "KAIBOSH".to_string(),
        Some(n) => n.to_string(),
        None => "None".to_string(),
    }
}
