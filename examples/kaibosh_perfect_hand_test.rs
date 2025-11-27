use tricksterstable_rs::games::kaibosh::game::{Card, GameState, KaiboshGame, Suit, KAIBOSH};

fn main() -> std::io::Result<()> {
    println!("=== TESTING MODEL WITH PERFECT KAIBOSH HANDS ===\n");

    // Test 1: Perfect Hearts hand (both jacks, A, K, Q of hearts, A of spades)
    println!("Test 1: Perfect Hearts Kaibosh hand");
    println!("Hand: J♥ J♦ A♥ K♥ Q♥ A♠");
    let mut game1 = KaiboshGame::default();
    game1.hands[0] = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        }, // J♥ (right bower)
        Card {
            id: 1,
            value: 11,
            suit: Suit::Diamonds,
        }, // J♦ (left bower)
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
    game1.state = GameState::Bidding;
    game1.current_player = 0;

    let mut game_clone = game1.clone();
    game_clone.use_policy_priors = true;
    let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
    ismcts.run_iterations(1, 500);
    let bid = ismcts.best_move();
    let bid_str = match bid {
        Some(KAIBOSH) => "KAIBOSH".to_string(),
        Some(0) => "Pass".to_string(),
        Some(n) => n.to_string(),
        None => "None".to_string(),
    };
    println!("Model bid: {}", bid_str);
    println!();

    // Test 2: Perfect Clubs hand
    println!("Test 2: Perfect Clubs Kaibosh hand");
    println!("Hand: J♣ J♠ A♣ K♣ Q♣ A♥");
    let mut game2 = KaiboshGame::default();
    game2.hands[0] = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Clubs,
        }, // J♣ (right bower)
        Card {
            id: 1,
            value: 11,
            suit: Suit::Spades,
        }, // J♠ (left bower)
        Card {
            id: 2,
            value: 14,
            suit: Suit::Clubs,
        }, // A♣
        Card {
            id: 3,
            value: 13,
            suit: Suit::Clubs,
        }, // K♣
        Card {
            id: 4,
            value: 12,
            suit: Suit::Clubs,
        }, // Q♣
        Card {
            id: 5,
            value: 14,
            suit: Suit::Hearts,
        }, // A♥
    ];
    game2.state = GameState::Bidding;
    game2.current_player = 0;

    let mut game_clone = game2.clone();
    game_clone.use_policy_priors = true;
    let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
    ismcts.run_iterations(1, 500);
    let bid = ismcts.best_move();
    let bid_str = match bid {
        Some(KAIBOSH) => "KAIBOSH".to_string(),
        Some(0) => "Pass".to_string(),
        Some(n) => n.to_string(),
        None => "None".to_string(),
    };
    println!("Model bid: {}", bid_str);
    println!();

    // Test 3: Very strong but not quite perfect hand
    println!("Test 3: Strong (but not perfect) hand");
    println!("Hand: J♥ J♦ A♥ K♥ 10♥ 9♥");
    let mut game3 = KaiboshGame::default();
    game3.hands[0] = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        }, // J♥
        Card {
            id: 1,
            value: 11,
            suit: Suit::Diamonds,
        }, // J♦
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
            value: 9,
            suit: Suit::Hearts,
        }, // 9♥
    ];
    game3.state = GameState::Bidding;
    game3.current_player = 0;

    let mut game_clone = game3.clone();
    game_clone.use_policy_priors = true;
    let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
    ismcts.run_iterations(1, 500);
    let bid = ismcts.best_move();
    let bid_str = match bid {
        Some(KAIBOSH) => "KAIBOSH".to_string(),
        Some(0) => "Pass".to_string(),
        Some(n) => n.to_string(),
        None => "None".to_string(),
    };
    println!("Model bid: {}", bid_str);
    println!();

    // Test 4: All trump hand (6 trump cards)
    println!("Test 4: All 6 cards are trump (Spades)");
    println!("Hand: J♠ J♣ A♠ K♠ Q♠ 10♠");
    let mut game4 = KaiboshGame::default();
    game4.hands[0] = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Spades,
        }, // J♠
        Card {
            id: 1,
            value: 11,
            suit: Suit::Clubs,
        }, // J♣
        Card {
            id: 2,
            value: 14,
            suit: Suit::Spades,
        }, // A♠
        Card {
            id: 3,
            value: 13,
            suit: Suit::Spades,
        }, // K♠
        Card {
            id: 4,
            value: 12,
            suit: Suit::Spades,
        }, // Q♠
        Card {
            id: 5,
            value: 10,
            suit: Suit::Spades,
        }, // 10♠
    ];
    game4.state = GameState::Bidding;
    game4.current_player = 0;

    let mut game_clone = game4.clone();
    game_clone.use_policy_priors = true;
    let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
    ismcts.run_iterations(1, 500);
    let bid = ismcts.best_move();
    let bid_str = match bid {
        Some(KAIBOSH) => "KAIBOSH".to_string(),
        Some(0) => "Pass".to_string(),
        Some(n) => n.to_string(),
        None => "None".to_string(),
    };
    println!("Model bid: {}", bid_str);

    Ok(())
}
