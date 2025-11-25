use tricksterstable_rs::games::kaibosh::game::KaiboshGame;

fn main() -> std::io::Result<()> {
    let num_games = 1000;
    let iterations = 100;

    println!("=== EXPERIMENT 1: Symmetric Rewards vs Baseline ===\n");
    println!(
        "Testing {} games with {} iterations per move...\n",
        num_games, iterations
    );
    println!("Team 0: experiment=1 (Symmetric rewards [-1,1] with game win/loss emphasis)");
    println!("Team 1: experiment=0 (Baseline asymmetric rewards [0,1])");
    println!();

    let mut team_0_wins = 0;
    let mut team_1_wins = 0;
    let mut max_iterations = 0;

    for game_num in 0..num_games {
        if (game_num + 1) % 100 == 0 {
            println!("Completed {} games...", game_num + 1);
        }

        let mut game = KaiboshGame::new();
        let mut hand_count = 0;

        // Play multiple hands until someone wins the game
        while game.scores[0] < game.score_threshold && game.scores[1] < game.score_threshold {
            // Play until this hand is complete
            while game.last_hand_score.is_none() {
                let moves = game.get_moves();
                if moves.is_empty() {
                    break;
                }

                let current_team = game.current_player % 2;

                // Team 0 uses experiment=1, Team 1 uses experiment=0
                let mut game_clone = game.clone();
                game_clone.use_policy_priors = true;
                game_clone.experiment = if current_team == 0 { 1 } else { 0 };

                let mut ismcts = ismcts::IsmctsHandler::new_with_puct(game_clone, 1.0);
                ismcts.run_iterations(1, iterations);
                let best_move = ismcts.best_move();

                game.apply_move(best_move);
            }

            hand_count += 1;

            // Check if game is over
            if game.scores[0] >= game.score_threshold || game.scores[1] >= game.score_threshold {
                break;
            }

            // Start a new hand
            game.new_hand();
        }

        if hand_count > max_iterations {
            max_iterations = hand_count;
        }

        // Debug first few games
        if game_num < 3 {
            println!(
                "Game {}: hands={}, scores=[{}, {}]",
                game_num, hand_count, game.scores[0], game.scores[1]
            );
        }

        // Record winner
        if game.scores[0] >= game.score_threshold {
            team_0_wins += 1;
        } else if game.scores[1] >= game.score_threshold {
            team_1_wins += 1;
        }
    }

    println!("\nMax hands in a game: {}", max_iterations);

    println!("\n=== RESULTS ===");
    println!("Total games: {}", num_games);
    println!();
    println!(
        "Team 0 (experiment=1): {} wins ({:.1}%)",
        team_0_wins,
        (team_0_wins as f32 / num_games as f32) * 100.0
    );
    println!(
        "Team 1 (experiment=0): {} wins ({:.1}%)",
        team_1_wins,
        (team_1_wins as f32 / num_games as f32) * 100.0
    );
    println!();

    let advantage = ((team_0_wins as f32 / num_games as f32) - 0.5) * 200.0;
    if advantage > 0.0 {
        println!("Experiment 1 advantage: +{:.1}%", advantage);
    } else {
        println!("Baseline advantage: +{:.1}%", -advantage);
    }

    Ok(())
}
