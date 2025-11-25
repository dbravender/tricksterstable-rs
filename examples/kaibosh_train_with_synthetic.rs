use tricksterstable_rs::games::kaibosh::game::{Card, Suit, DEFAULT_SCORE_THRESHOLD};
use tricksterstable_rs::games::kaibosh::policy_model::PolicyNetwork;

fn main() -> std::io::Result<()> {
    println!("=== TRAINING POLICY NETWORK WITH SYNTHETIC KAIBOSH DATA ===\n");

    // Create new 15-input policy network
    let mut network = PolicyNetwork::new(15, 128, 8);

    println!("Generating synthetic training data...\n");

    // Generate perfect Kaibosh hands
    let perfect_hands = generate_perfect_kaibosh_hands();
    println!(
        "Generated {} perfect Kaibosh scenarios",
        perfect_hands.len()
    );

    // Generate desperate scenarios
    let desperate_hands = generate_desperate_scenarios();
    println!("Generated {} desperate scenarios", desperate_hands.len());

    // Generate kamikaze scenarios (opponent has bid, need to steal)
    let kamikaze_hands = generate_kamikaze_scenarios();
    println!("Generated {} kamikaze scenarios", kamikaze_hands.len());

    // Generate normal/conservative hands
    let normal_hands = generate_normal_scenarios();
    println!("Generated {} normal scenarios\n", normal_hands.len());

    // Combine all training data
    let mut training_data = Vec::new();
    training_data.extend(perfect_hands);
    training_data.extend(desperate_hands);
    training_data.extend(kamikaze_hands);
    training_data.extend(normal_hands);

    println!("Total training examples: {}\n", training_data.len());

    // Training parameters
    let epochs = 100;
    let learning_rate = 0.015;

    println!(
        "Training for {} epochs with learning_rate={}\n",
        epochs, learning_rate
    );

    for epoch in 0..epochs {
        let mut total_loss = 0.0;
        let mut count = 0;

        for (hand, trump, my_score, opp_score, high_bidder, current_player, target_bid) in
            &training_data
        {
            network.train(
                hand,
                *trump,
                *my_score,
                *opp_score,
                *high_bidder,
                *current_player,
                *target_bid,
                learning_rate,
            );
            count += 1;
        }

        if (epoch + 1) % 10 == 0 {
            println!("Epoch {}/{} complete", epoch + 1, epochs);
        }
    }

    println!("\nTraining complete!");

    // Save the model
    let model_json = serde_json::to_string_pretty(&network)?;
    std::fs::write(
        "src/games/kaibosh/policy_model_with_context.json",
        model_json,
    )?;

    println!("\nModel saved to policy_model_with_context.json");

    // Test the model on a perfect hand
    println!("\n=== TESTING MODEL ===\n");
    test_model(&network);

    Ok(())
}

type TrainingExample = (Vec<Card>, Suit, i32, i32, Option<usize>, usize, i32);

fn generate_perfect_kaibosh_hands() -> Vec<TrainingExample> {
    let mut examples = Vec::new();

    // For each suit as trump
    for (trump_idx, trump) in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades]
        .iter()
        .enumerate()
    {
        let left_bower_suit = trump.same_color_suit();

        // Perfect hand: Both bowers + A,K,Q of trump + off-ace
        // Vary the off-ace suit
        for off_suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades].iter() {
            if off_suit == trump {
                continue;
            }

            let hand = vec![
                Card {
                    id: 0,
                    value: 11,
                    suit: *trump,
                }, // Right bower
                Card {
                    id: 1,
                    value: 11,
                    suit: left_bower_suit,
                }, // Left bower
                Card {
                    id: 2,
                    value: 14,
                    suit: *trump,
                }, // Ace of trump
                Card {
                    id: 3,
                    value: 13,
                    suit: *trump,
                }, // King of trump
                Card {
                    id: 4,
                    value: 12,
                    suit: *trump,
                }, // Queen of trump
                Card {
                    id: 5,
                    value: 14,
                    suit: *off_suit,
                }, // Off-ace
            ];

            // Add examples with different game contexts
            // Even score - should Kaibosh
            examples.push((hand.clone(), *trump, 0, 0, None, 0, 12));

            // Ahead in score - should Kaibosh
            examples.push((hand.clone(), *trump, 15, 10, None, 0, 12));

            // Behind in score - should definitely Kaibosh
            examples.push((hand.clone(), *trump, 10, 20, None, 0, 12));

            // Opponent near win - desperate Kaibosh
            examples.push((hand.clone(), *trump, 5, 23, None, 0, 12));
        }
    }

    examples
}

fn generate_desperate_scenarios() -> Vec<TrainingExample> {
    let mut examples = Vec::new();

    // Behind in score with strong but not perfect hands - bid more aggressively
    for trump in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades].iter() {
        let left_bower_suit = trump.same_color_suit();

        // Strong hand: both bowers + A,K of trump + two tens
        let strong_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 11,
                suit: left_bower_suit,
            },
            Card {
                id: 2,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 13,
                suit: *trump,
            },
            Card {
                id: 4,
                value: 10,
                suit: *trump,
            },
            Card {
                id: 5,
                value: 10,
                suit: Suit::Spades,
            },
        ];

        // Very behind with perfect-ish hand - bid Kaibosh only then
        let perfect_ish = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 11,
                suit: left_bower_suit,
            },
            Card {
                id: 2,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 13,
                suit: *trump,
            },
            Card {
                id: 4,
                value: 12,
                suit: *trump,
            },
            Card {
                id: 5,
                value: 14,
                suit: Suit::Spades,
            },
        ];

        // Very behind with near-perfect - Kaibosh desperation
        examples.push((perfect_ish.clone(), *trump, 5, 23, None, 0, 12));
        examples.push((perfect_ish, *trump, 8, 24, None, 0, 12));

        // Behind with strong but not perfect - bid high (5-6) to name trump
        examples.push((strong_hand.clone(), *trump, 10, 18, None, 0, 6));
        examples.push((strong_hand.clone(), *trump, 12, 20, None, 0, 6));
        examples.push((strong_hand.clone(), *trump, 8, 16, None, 0, 5));
        examples.push((strong_hand, *trump, 15, 21, None, 0, 5));

        // Medium hand - normally bid 3, but bid 4-5 when behind
        let medium_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 2,
                value: 10,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 9,
                suit: *trump,
            },
            Card {
                id: 4,
                value: 10,
                suit: Suit::Clubs,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Spades,
            },
        ];

        // Behind - bid higher than you normally would
        examples.push((medium_hand.clone(), *trump, 10, 18, None, 0, 5));
        examples.push((medium_hand.clone(), *trump, 12, 19, None, 0, 4));

        // Not behind - bid normally
        examples.push((medium_hand, *trump, 12, 10, None, 0, 3));
    }

    examples
}

fn generate_kamikaze_scenarios() -> Vec<TrainingExample> {
    let mut examples = Vec::new();

    // Opponent has bid, need to steal trump with decent hand
    for trump in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades].iter() {
        let left_bower_suit = trump.same_color_suit();

        // Decent hand with both bowers
        let good_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 11,
                suit: left_bower_suit,
            },
            Card {
                id: 2,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 10,
                suit: *trump,
            },
            Card {
                id: 4,
                value: 9,
                suit: *trump,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Clubs,
            },
        ];

        // Perfect hand for desperate Kaibosh steal
        let perfect_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 11,
                suit: left_bower_suit,
            },
            Card {
                id: 2,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 13,
                suit: *trump,
            },
            Card {
                id: 4,
                value: 12,
                suit: *trump,
            },
            Card {
                id: 5,
                value: 14,
                suit: Suit::Spades,
            },
        ];

        // Opponent (player 1) has bid, team 0 behind - steal with aggressive bid
        examples.push((good_hand.clone(), *trump, 10, 15, Some(1), 0, 6));
        examples.push((good_hand.clone(), *trump, 12, 18, Some(1), 0, 5));

        // Opponent very close to winning with perfect hand - Kaibosh to steal
        examples.push((perfect_hand, *trump, 10, 23, Some(1), 0, 12));

        // Opponent has bid but we're even - still bid aggressively to name trump
        examples.push((good_hand, *trump, 12, 12, Some(1), 0, 5));
    }

    examples
}

fn generate_normal_scenarios() -> Vec<TrainingExample> {
    let mut examples = Vec::new();

    // Generate conservative/normal bidding scenarios
    for trump in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades].iter() {
        let left_bower_suit = trump.same_color_suit();

        // Weak hand - should pass
        let weak_hand = vec![
            Card {
                id: 0,
                value: 9,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 10,
                suit: Suit::Clubs,
            },
            Card {
                id: 2,
                value: 12,
                suit: Suit::Diamonds,
            },
            Card {
                id: 3,
                value: 9,
                suit: Suit::Spades,
            },
            Card {
                id: 4,
                value: 10,
                suit: Suit::Hearts,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Spades,
            },
        ];
        examples.push((weak_hand, *trump, 0, 0, None, 0, 0)); // Pass

        // Medium hand - bid 3
        let medium_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 2,
                value: 10,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 9,
                suit: Suit::Clubs,
            },
            Card {
                id: 4,
                value: 10,
                suit: Suit::Diamonds,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Spades,
            },
        ];
        examples.push((medium_hand, *trump, 0, 0, None, 0, 3));

        // Good hand - bid 4-5
        let good_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: *trump,
            },
            Card {
                id: 1,
                value: 11,
                suit: left_bower_suit,
            },
            Card {
                id: 2,
                value: 14,
                suit: *trump,
            },
            Card {
                id: 3,
                value: 10,
                suit: *trump,
            },
            Card {
                id: 4,
                value: 9,
                suit: Suit::Diamonds,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Spades,
            },
        ];
        examples.push((good_hand.clone(), *trump, 0, 0, None, 0, 4));
        examples.push((good_hand, *trump, 5, 10, None, 0, 5));
    }

    // Add many more normal examples to balance the dataset
    for _ in 0..15 {
        examples.extend(generate_normal_scenarios_batch());
    }

    // Add explicit "bid higher when behind" examples
    for _ in 0..10 {
        examples.extend(generate_context_aware_examples());
    }

    examples
}

fn generate_context_aware_examples() -> Vec<TrainingExample> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut examples = Vec::new();

    for _ in 0..5 {
        let trump_idx = rng.gen_range(0..4);
        let trump = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades][trump_idx];
        let left_bower_suit = trump.same_color_suit();

        // Create a medium-strength hand
        let hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: trump,
            },
            Card {
                id: 1,
                value: 14,
                suit: trump,
            },
            Card {
                id: 2,
                value: 10,
                suit: trump,
            },
            Card {
                id: 3,
                value: 9,
                suit: trump,
            },
            Card {
                id: 4,
                value: 10,
                suit: Suit::Clubs,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Spades,
            },
        ];

        // Same hand, different contexts
        // Even score - bid 3
        examples.push((hand.clone(), trump, 12, 12, None, 0, 3));

        // Moderately behind - bid 4-5
        let behind_score_my = rng.gen_range(8..15);
        let behind_score_opp = behind_score_my + rng.gen_range(5..10);
        let higher_bid = if rng.gen_bool(0.5) { 4 } else { 5 };
        examples.push((
            hand.clone(),
            trump,
            behind_score_my,
            behind_score_opp,
            None,
            0,
            higher_bid,
        ));

        // Create a good-strength hand
        let good_hand = vec![
            Card {
                id: 0,
                value: 11,
                suit: trump,
            },
            Card {
                id: 1,
                value: 11,
                suit: left_bower_suit,
            },
            Card {
                id: 2,
                value: 14,
                suit: trump,
            },
            Card {
                id: 3,
                value: 10,
                suit: trump,
            },
            Card {
                id: 4,
                value: 9,
                suit: trump,
            },
            Card {
                id: 5,
                value: 12,
                suit: Suit::Diamonds,
            },
        ];

        // Even score - bid 4
        examples.push((good_hand.clone(), trump, 10, 10, None, 0, 4));

        // Behind - bid 5-6
        let behind_my = rng.gen_range(5..12);
        let behind_opp = behind_my + rng.gen_range(6..12);
        let aggressive_bid = if behind_opp >= 20 { 6 } else { 5 };
        examples.push((
            good_hand,
            trump,
            behind_my,
            behind_opp,
            None,
            0,
            aggressive_bid,
        ));
    }

    examples
}

fn generate_normal_scenarios_batch() -> Vec<TrainingExample> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut examples = Vec::new();

    for _ in 0..10 {
        let trump_idx = rng.gen_range(0..4);
        let trump = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades][trump_idx];

        // Random hand strength
        let strength = rng.gen_range(0..100);
        let bid = if strength < 40 {
            0 // Pass
        } else if strength < 60 {
            3
        } else if strength < 80 {
            4
        } else {
            5
        };

        // Create a hand matching the bid strength
        let hand = create_hand_for_bid(trump, bid);

        let my_score = rng.gen_range(0..20);
        let opp_score = rng.gen_range(0..20);

        examples.push((hand, trump, my_score, opp_score, None, 0, bid));
    }

    examples
}

fn create_hand_for_bid(trump: Suit, bid: i32) -> Vec<Card> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let left_bower_suit = trump.same_color_suit();

    let mut hand = Vec::new();
    let mut id = 0;

    match bid {
        0 => {
            // Weak hand - few trump, no bowers
            for _ in 0..6 {
                let suit = if rng.gen_bool(0.3) {
                    trump
                } else {
                    [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades][rng.gen_range(0..4)]
                };
                let value = rng.gen_range(9..=13);
                hand.push(Card { id, value, suit });
                id += 1;
            }
        }
        3 => {
            // Medium - one bower or good trump
            if rng.gen_bool(0.5) {
                hand.push(Card {
                    id,
                    value: 11,
                    suit: trump,
                });
                id += 1;
            }
            hand.push(Card {
                id,
                value: 14,
                suit: trump,
            });
            id += 1;
            hand.push(Card {
                id,
                value: 10,
                suit: trump,
            });
            id += 1;

            for _ in hand.len()..6 {
                let value = rng.gen_range(9..=12);
                let suit =
                    [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades][rng.gen_range(0..4)];
                hand.push(Card { id, value, suit });
                id += 1;
            }
        }
        4..=5 => {
            // Good - maybe both bowers
            if rng.gen_bool(0.7) {
                hand.push(Card {
                    id,
                    value: 11,
                    suit: trump,
                });
                id += 1;
            }
            if rng.gen_bool(0.5) {
                hand.push(Card {
                    id,
                    value: 11,
                    suit: left_bower_suit,
                });
                id += 1;
            }
            hand.push(Card {
                id,
                value: 14,
                suit: trump,
            });
            id += 1;

            for _ in hand.len()..6 {
                let value = if rng.gen_bool(0.6) {
                    rng.gen_range(10..=13)
                } else {
                    9
                };
                let suit = if rng.gen_bool(0.6) {
                    trump
                } else {
                    [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades][rng.gen_range(0..4)]
                };
                hand.push(Card { id, value, suit });
                id += 1;
            }
        }
        _ => {
            // Default to random
            for _ in 0..6 {
                let value = rng.gen_range(9..=14);
                let suit =
                    [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades][rng.gen_range(0..4)];
                hand.push(Card { id, value, suit });
                id += 1;
            }
        }
    }

    hand
}

fn test_model(network: &PolicyNetwork) {
    // Test 1: Perfect Kaibosh hand - even score
    let perfect_hand = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        },
        Card {
            id: 1,
            value: 11,
            suit: Suit::Diamonds,
        },
        Card {
            id: 2,
            value: 14,
            suit: Suit::Hearts,
        },
        Card {
            id: 3,
            value: 13,
            suit: Suit::Hearts,
        },
        Card {
            id: 4,
            value: 12,
            suit: Suit::Hearts,
        },
        Card {
            id: 5,
            value: 14,
            suit: Suit::Spades,
        },
    ];

    println!("Test 1: Perfect Kaibosh hand (J♥ J♦ A♥ K♥ Q♥ A♠) - even score");
    let probs = network.evaluate(&perfect_hand, Suit::Hearts, 0, 0, None, 0);
    print_bid_probabilities(&probs);

    // Test 2: Perfect hand, very behind - desperation Kaibosh
    println!("\nTest 2: Perfect hand, desperate (5-23)");
    let probs = network.evaluate(&perfect_hand, Suit::Hearts, 5, 23, None, 0);
    print_bid_probabilities(&probs);

    // Test 3: Strong but not perfect hand - even score
    let strong_hand = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        },
        Card {
            id: 1,
            value: 11,
            suit: Suit::Diamonds,
        },
        Card {
            id: 2,
            value: 14,
            suit: Suit::Hearts,
        },
        Card {
            id: 3,
            value: 13,
            suit: Suit::Hearts,
        },
        Card {
            id: 4,
            value: 10,
            suit: Suit::Hearts,
        },
        Card {
            id: 5,
            value: 10,
            suit: Suit::Spades,
        },
    ];

    println!("\nTest 3: Strong hand (J♥ J♦ A♥ K♥ 10♥ 10♠) - even score");
    let probs = network.evaluate(&strong_hand, Suit::Hearts, 0, 0, None, 0);
    print_bid_probabilities(&probs);

    // Test 4: Same strong hand, moderately behind - should bid higher
    println!("\nTest 4: Strong hand, moderately behind (12-19)");
    let probs = network.evaluate(&strong_hand, Suit::Hearts, 12, 19, None, 0);
    print_bid_probabilities(&probs);

    // Test 5: Medium hand - even score
    let medium_hand = vec![
        Card {
            id: 0,
            value: 11,
            suit: Suit::Hearts,
        },
        Card {
            id: 1,
            value: 14,
            suit: Suit::Hearts,
        },
        Card {
            id: 2,
            value: 10,
            suit: Suit::Hearts,
        },
        Card {
            id: 3,
            value: 9,
            suit: Suit::Hearts,
        },
        Card {
            id: 4,
            value: 10,
            suit: Suit::Clubs,
        },
        Card {
            id: 5,
            value: 12,
            suit: Suit::Spades,
        },
    ];

    println!("\nTest 5: Medium hand - even score");
    let probs = network.evaluate(&medium_hand, Suit::Hearts, 0, 0, None, 0);
    print_bid_probabilities(&probs);

    // Test 6: Same medium hand - behind, should bid higher
    println!("\nTest 6: Medium hand - behind (10-18)");
    let probs = network.evaluate(&medium_hand, Suit::Hearts, 10, 18, None, 0);
    print_bid_probabilities(&probs);

    // Test 7: Weak hand
    let weak_hand = vec![
        Card {
            id: 0,
            value: 9,
            suit: Suit::Hearts,
        },
        Card {
            id: 1,
            value: 10,
            suit: Suit::Clubs,
        },
        Card {
            id: 2,
            value: 12,
            suit: Suit::Diamonds,
        },
        Card {
            id: 3,
            value: 9,
            suit: Suit::Spades,
        },
        Card {
            id: 4,
            value: 10,
            suit: Suit::Hearts,
        },
        Card {
            id: 5,
            value: 12,
            suit: Suit::Spades,
        },
    ];

    println!("\nTest 7: Weak hand (should pass even when behind)");
    let probs = network.evaluate(&weak_hand, Suit::Hearts, 10, 18, None, 0);
    print_bid_probabilities(&probs);
}

fn print_bid_probabilities(probs: &[f32]) {
    let bids = ["Pass", "1", "2", "3", "4", "5", "6", "Kaibosh"];
    println!("Bid probabilities:");
    for (i, &prob) in probs.iter().enumerate() {
        println!("  {}: {:.1}%", bids[i], prob * 100.0);
    }

    let max_idx = probs
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    println!("  → Best bid: {}", bids[max_idx]);
}
