use tricksterstable_rs::games::kaibosh::game::{GameState, KaiboshGame, KAIBOSH};

fn main() -> std::io::Result<()> {
    let num_games = 1000;
    let iterations = 100;

    println!("=== ANALYZING KAIBOSH BID FREQUENCY ===\n");
    println!("Running {} games with policy network...\n", num_games);

    let mut total_bids = 0;
    let mut bid_counts = std::collections::HashMap::new();
    let mut kaibosh_by_score = std::collections::HashMap::new();

    // Track score ranges when Kaibosh is bid
    for range in &[-25, -20, -15, -10, -5, 0, 5, 10, 15, 20] {
        kaibosh_by_score.insert(*range, 0);
    }
    let mut total_bidding_opportunities_by_score = std::collections::HashMap::new();
    for range in &[-25, -20, -15, -10, -5, 0, 5, 10, 15, 20] {
        total_bidding_opportunities_by_score.insert(*range, 0);
    }

    for game_num in 0..num_games {
        if (game_num + 1) % 100 == 0 {
            println!("Completed {} games...", game_num + 1);
        }

        let mut game = KaiboshGame::new();

        // Play until hand is complete
        while game.last_hand_score.is_none() {
            let moves = game.get_moves();
            if moves.is_empty() {
                break;
            }

            if matches!(game.state, GameState::Bidding) {
                // Track score differential
                let score_diff = game.scores[game.current_player % 2]
                    - game.scores[1 - (game.current_player % 2)];

                let score_range = ((score_diff / 5) * 5).max(-25).min(20);
                *total_bidding_opportunities_by_score
                    .entry(score_range)
                    .or_insert(0) += 1;

                let mut game_clone = game.clone();
                game_clone.use_policy_priors = true;
                let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
                ismcts.run_iterations(1, iterations);
                let best_move = ismcts.best_move();

                if let Some(bid) = best_move {
                    total_bids += 1;
                    *bid_counts.entry(bid).or_insert(0) += 1;

                    if bid == KAIBOSH {
                        *kaibosh_by_score.entry(score_range).or_insert(0) += 1;
                    }
                }

                game.apply_move(best_move);
            } else {
                let mut ismcts = ismcts::IsmctsHandler::new(game.clone());
                ismcts.run_iterations(1, 100);
                game.apply_move(ismcts.best_move());
            }
        }
    }

    println!("\n=== BID FREQUENCY ANALYSIS ===");
    println!("\nTotal bids made: {}", total_bids);
    println!("\nBid distribution:");

    let mut sorted_bids: Vec<_> = bid_counts.iter().collect();
    sorted_bids.sort_by_key(|(bid, _)| *bid);

    for (bid, count) in sorted_bids {
        let bid_name = if *bid == 0 {
            "Pass".to_string()
        } else if *bid == KAIBOSH {
            "Kaibosh".to_string()
        } else {
            bid.to_string()
        };
        let percentage = (*count as f32 / total_bids as f32) * 100.0;
        println!("  {}: {} ({:.2}%)", bid_name, count, percentage);
    }

    println!("\n=== KAIBOSH BY SCORE DIFFERENTIAL ===");
    println!("Score Range | Kaiboshes | Opportunities | Percentage");
    println!("--------------------------------------------------------");

    let mut score_ranges: Vec<_> = kaibosh_by_score.keys().collect();
    score_ranges.sort();

    for range in score_ranges {
        let kaibosh_count = kaibosh_by_score[range];
        let opportunities = total_bidding_opportunities_by_score[range];
        let percentage = if opportunities > 0 {
            (kaibosh_count as f32 / opportunities as f32) * 100.0
        } else {
            0.0
        };
        println!(
            "{:>4} to {:>3} | {:>9} | {:>13} | {:>6.2}%",
            range,
            range + 4,
            kaibosh_count,
            opportunities,
            percentage
        );
    }

    Ok(())
}
