use tricksterstable_rs::games::kaibosh::game::KaiboshGame;

fn main() -> std::io::Result<()> {
    let num_games = 100;
    let iterations = 100;
    let debug = false; // Set to true to see Kaibosh bid details

    println!("=== Policy Network vs Pure ISMCTS ===\n");
    println!(
        "Testing {} games with {} iterations per move...\n",
        num_games, iterations
    );
    println!("Team 0: With policy priors (context-aware model)");
    println!("Team 1: Pure ISMCTS (no policy priors)");
    println!();

    let mut team_0_wins = 0;
    let mut team_1_wins = 0;
    let mut team_0_kaibosh_bids = 0;
    let mut team_0_kaibosh_won_auction = 0;
    let mut team_0_kaibosh_successes = 0;
    let mut team_1_kaibosh_bids = 0;
    let mut team_1_kaibosh_won_auction = 0;
    let mut team_1_kaibosh_successes = 0;

    for game_num in 0..num_games {
        if !debug && (game_num + 1) % 50 == 0 {
            println!("Completed {} games...", game_num + 1);
        }
        if debug {
            println!("\n--- Starting Game {} ---", game_num);
        }

        let mut game = KaiboshGame::new();
        let mut _hand_count = 0;

        // Play multiple hands until someone wins the game
        while game.scores[0] < game.score_threshold && game.scores[1] < game.score_threshold {
            let mut winning_bidder: Option<usize> = None;
            let mut winning_bid: Option<i32> = None;

            // Play until this hand is complete
            while game.last_hand_score.is_none() {
                let moves = game.get_moves();
                if moves.is_empty() {
                    break;
                }

                let current_team = game.current_player % 2;

                // Track Kaibosh bids (bid value 12)
                let best_move: Option<i32>;
                if current_team == 0 {
                    // Team 0: With policy priors
                    let mut game_clone = game.clone();
                    game_clone.use_policy_priors = true;
                    game_clone.experiment = 0;
                    let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
                    ismcts.run_iterations(1, iterations);
                    best_move = ismcts.best_move();
                } else {
                    // Team 1: Pure ISMCTS
                    let mut game_clone = game.clone();
                    game_clone.use_policy_priors = false;
                    let mut ismcts = ismcts::IsmctsHandler::new(game_clone);
                    ismcts.run_iterations(1, iterations);
                    best_move = ismcts.best_move();
                }

                // Track Kaibosh bids made
                if best_move == Some(12) {
                    if debug {
                        println!(
                            "Game {}, Hand {}: Player {} (Team {}) bid KAIBOSH!",
                            game_num, _hand_count, game.current_player, current_team
                        );
                    }
                    if current_team == 0 {
                        team_0_kaibosh_bids += 1;
                    } else {
                        team_1_kaibosh_bids += 1;
                    }
                }

                // Track winning bid
                if let Some(bid) = best_move {
                    if bid > 0 && bid != 100 {
                        winning_bidder = Some(game.current_player);
                        winning_bid = Some(bid);
                    }
                }

                game.apply_move(best_move);
            }

            _hand_count += 1;

            // Check if the winning bid was Kaibosh and track success
            if let (Some(bidder), Some(12)) = (winning_bidder, winning_bid) {
                let bidder_team = bidder % 2;

                // Track that this team won the auction with Kaibosh
                if bidder_team == 0 {
                    team_0_kaibosh_won_auction += 1;
                } else {
                    team_1_kaibosh_won_auction += 1;
                }

                // Check if they made all 6 tricks (scored 12 points)
                if game.scores_this_hand[bidder_team] == 12 {
                    if bidder_team == 0 {
                        team_0_kaibosh_successes += 1;
                    } else {
                        team_1_kaibosh_successes += 1;
                    }
                }
            }

            // Check if game is over
            if game.scores[0] >= game.score_threshold || game.scores[1] >= game.score_threshold {
                break;
            }

            // Start a new hand
            game.new_hand();
        }

        // Record winner
        if game.scores[0] >= game.score_threshold {
            team_0_wins += 1;
        } else if game.scores[1] >= game.score_threshold {
            team_1_wins += 1;
        }
    }

    println!("\n=== RESULTS ===");
    println!("Total games: {}\n", num_games);

    println!("WINS:");
    println!(
        "Team 0 (policy): {} wins ({:.1}%)",
        team_0_wins,
        (team_0_wins as f32 / num_games as f32) * 100.0
    );
    println!(
        "Team 1 (pure ISMCTS): {} wins ({:.1}%)",
        team_1_wins,
        (team_1_wins as f32 / num_games as f32) * 100.0
    );
    println!();

    println!("KAIBOSH STATISTICS:");
    println!("\nTeam 0 (policy network):");
    println!("  Kaibosh bids made: {}", team_0_kaibosh_bids);
    println!("  Won auction with Kaibosh: {}", team_0_kaibosh_won_auction);
    println!(
        "  Successfully made Kaibosh: {} ({:.1}% of auctions won)",
        team_0_kaibosh_successes,
        if team_0_kaibosh_won_auction > 0 {
            (team_0_kaibosh_successes as f32 / team_0_kaibosh_won_auction as f32) * 100.0
        } else {
            0.0
        }
    );

    println!("\nTeam 1 (pure ISMCTS):");
    println!("  Kaibosh bids made: {}", team_1_kaibosh_bids);
    println!("  Won auction with Kaibosh: {}", team_1_kaibosh_won_auction);
    println!(
        "  Successfully made Kaibosh: {} ({:.1}% of auctions won)",
        team_1_kaibosh_successes,
        if team_1_kaibosh_won_auction > 0 {
            (team_1_kaibosh_successes as f32 / team_1_kaibosh_won_auction as f32) * 100.0
        } else {
            0.0
        }
    );
    println!();

    let advantage = ((team_0_wins as f32 / num_games as f32) - 0.5) * 200.0;
    if advantage > 0.0 {
        println!("Policy network advantage: +{:.1}%", advantage);
    } else {
        println!("Pure ISMCTS advantage: +{:.1}%", -advantage);
    }

    Ok(())
}
