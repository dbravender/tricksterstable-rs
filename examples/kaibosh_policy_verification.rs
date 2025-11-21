use tricksterstable_rs::games::kaibosh::{GameState, KaiboshGame};

fn main() -> std::io::Result<()> {
    let num_games = 1000;
    let iterations = 100;

    println!("=== POLICY NETWORK + ISMCTS vs PURE ISMCTS ===");
    println!(
        "Comparing Policy Network + ISMCTS vs Pure ISMCTS with {} iterations\n",
        iterations
    );

    let mut team0_wins = 0; // Policy network + ISMCTS team
    let mut team1_wins = 0; // Pure ISMCTS team
    let mut team0_total_score = 0;
    let mut team1_total_score = 0;

    for game_num in 0..num_games {
        if (game_num + 1) % 50 == 0 {
            println!("Completed {} games...", game_num + 1);
        }

        let mut game = KaiboshGame::new();

        // Play until hand is complete
        while game.last_hand_score.is_none() {
            let moves = game.get_moves();
            if moves.is_empty() {
                break;
            }

            let best_move =
                if matches!(game.state, GameState::Bidding) && game.current_player % 2 == 0 {
                    // Team 0 (players 0,2): Use ISMCTS with policy network
                    let mut game_clone = game.clone();
                    game_clone.use_policy_priors = true;
                    let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
                    ismcts.run_iterations(1, iterations);
                    ismcts.best_move()
                } else if matches!(game.state, GameState::Bidding) {
                    // Team 1 (players 1,3): Use Pure ISMCTS (no policy network)
                    let mut game_clone = game.clone();
                    game_clone.use_policy_priors = false;
                    let mut ismcts = ismcts::IsmctsHandler::new(game_clone);
                    ismcts.run_iterations(1, iterations);
                    ismcts.best_move()
                } else {
                    // Both teams use same strategy for card play (pure ISMCTS)
                    let mut ismcts = ismcts::IsmctsHandler::new(game.clone());
                    ismcts.run_iterations(1, 100);
                    ismcts.best_move()
                };

            game.apply_move(best_move);
        }

        // Record results
        if let Some(score) = game.last_hand_score {
            let bidder = game
                .bids
                .iter()
                .enumerate()
                .max_by_key(|(_, &bid)| bid.unwrap_or(0))
                .map(|(i, _)| i)
                .unwrap();
            let bidding_team = bidder % 2;

            if score > 0 {
                if bidding_team == 0 {
                    team0_wins += 1;
                    team0_total_score += score;
                } else {
                    team1_wins += 1;
                    team1_total_score += score;
                }
            } else if score < 0 {
                if bidding_team == 0 {
                    team1_wins += 1;
                    team1_total_score += score.abs();
                } else {
                    team0_wins += 1;
                    team0_total_score += score.abs();
                }
            }
        }
    }

    println!("\n=== FINAL RESULTS ===");
    println!(
        "\nTeam 0 (Policy Network + ISMCTS - {} iterations):",
        iterations
    );
    println!("  Wins: {}", team0_wins);
    println!(
        "  Win rate: {:.1}%",
        100.0 * team0_wins as f32 / num_games as f32
    );
    println!("  Total score: {}", team0_total_score);
    println!(
        "  Avg score per game: {:.2}",
        team0_total_score as f32 / num_games as f32
    );

    println!("\nTeam 1 (Pure ISMCTS - {} iterations):", iterations);
    println!("  Wins: {}", team1_wins);
    println!(
        "  Win rate: {:.1}%",
        100.0 * team1_wins as f32 / num_games as f32
    );
    println!("  Total score: {}", team1_total_score);
    println!(
        "  Avg score per game: {:.2}",
        team1_total_score as f32 / num_games as f32
    );

    println!("\n=== PERFORMANCE SUMMARY ===");
    if team0_wins > team1_wins {
        let advantage = 100.0 * (team0_wins - team1_wins) as f32 / num_games as f32;
        println!(
            "Policy Network + ISMCTS WINS with {:.1}% advantage!",
            advantage
        );
    } else if team1_wins > team0_wins {
        let advantage = 100.0 * (team1_wins - team0_wins) as f32 / num_games as f32;
        println!("Pure ISMCTS wins with {:.1}% advantage", advantage);
    } else {
        println!("TIE!");
    }

    Ok(())
}
