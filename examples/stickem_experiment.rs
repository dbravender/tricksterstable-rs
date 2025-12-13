use rand::{seq::SliceRandom, thread_rng};
use std::collections::HashMap;
use tricksterstable_rs::games::stickem::{get_mcts_move, State, StickEmGame};
use ismcts::Game;

// Different reward function experiments
#[derive(Debug, Clone, Copy)]
enum RewardFunction {
    // Original linear interpolation
    LinearNormalized,
    // Winner takes all (1.0 for winner, -1.0 for loser, 0.0 for ties)
    WinnerTakesAll,
    // Score difference based (reward proportional to score difference)
    ScoreDifference,
    // Exponential reward (amplifies score differences)
    Exponential,
    // Rank-based reward (based on ranking position)
    RankBased,
}

// Modified game that can use different reward functions
#[derive(Debug, Clone)]
struct ExperimentalStickEmGame {
    game: StickEmGame,
    reward_function: RewardFunction,
}

impl ExperimentalStickEmGame {
    fn new(reward_function: RewardFunction) -> Self {
        Self {
            game: StickEmGame::new(),
            reward_function,
        }
    }
}

impl Game for ExperimentalStickEmGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        self.game.randomize_determination(observer);
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.game.current_player()
    }

    fn next_player(&self) -> Self::PlayerTag {
        self.game.next_player()
    }

    fn available_moves(&self) -> Self::MoveList {
        self.game.available_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.game.make_move(mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.game.state != State::GameOver {
            return None;
        }

        let scores = self.game.scores;
        let player_score = scores[player] as f64;
        let max_score = *scores.iter().max().unwrap() as f64;
        let min_score = *scores.iter().min().unwrap() as f64;

        match self.reward_function {
            RewardFunction::LinearNormalized => {
                // Original implementation
                if max_score == min_score {
                    return Some(0.0);
                }
                let normalized = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;
                Some(normalized)
            },
            RewardFunction::WinnerTakesAll => {
                // Simple win/lose
                if player_score == max_score {
                    if scores.iter().filter(|&&s| s == scores[player]).count() > 1 {
                        Some(0.0) // Tie
                    } else {
                        Some(1.0) // Win
                    }
                } else {
                    Some(-1.0) // Lose
                }
            },
            RewardFunction::ScoreDifference => {
                // Reward based on how much better/worse than average
                let avg_score: f64 = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
                let diff = player_score - avg_score;
                // Normalize by typical score range (observed to be ~50 points)
                Some(diff / 25.0).map(|x| x.max(-1.0).min(1.0))
            },
            RewardFunction::Exponential => {
                // Exponential amplification of linear normalized
                if max_score == min_score {
                    return Some(0.0);
                }
                let linear = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;
                // Apply exponential transformation: sign(x) * (|x|^2)
                Some(linear.signum() * linear.abs().powf(2.0))
            },
            RewardFunction::RankBased => {
                // Reward based on ranking position
                let mut score_ranks: Vec<(i32, usize)> = scores.iter().enumerate().map(|(i, &s)| (s, i)).collect();
                score_ranks.sort_by_key(|&(score, _)| std::cmp::Reverse(score));

                let player_rank = score_ranks.iter().position(|(_, p)| *p == player).unwrap();
                match player_rank {
                    0 => Some(1.0),   // 1st place
                    1 => Some(0.33),  // 2nd place
                    2 => Some(-0.33), // 3rd place
                    3 => Some(-1.0),  // 4th place
                    _ => Some(0.0),   // Shouldn't happen with 4 players
                }
            }
        }
    }
}

fn run_experiment(reward_function: RewardFunction, games: usize, iterations: i32) -> (f64, HashMap<usize, i32>) {
    let mut total_score = 0.0;
    let mut wins = HashMap::new();
    let mut rng = thread_rng();

    for _ in 0..games {
        let mut game = ExperimentalStickEmGame::new(reward_function);

        while game.game.state != State::GameOver {
            let action = if [1, 3].contains(&game.current_player()) {
                // AI players using the experimental reward function
                game.game.experiment = true;
                let mut new_game = game.clone();
                new_game.game.no_changes = true;
                let mut ismcts = ismcts::IsmctsHandler::new(new_game);
                let parallel_threads: usize = 8;
                ismcts.run_iterations(
                    parallel_threads,
                    (iterations as f64 / parallel_threads as f64) as usize,
                );
                ismcts.best_move().expect("should have a move to make")
            } else {
                // Control players using original reward function
                game.game.experiment = false;
                get_mcts_move(&game.game, iterations, false)
            };
            game.make_move(&action);
        }

        // Record results
        let max_score = *game.game.scores.iter().max().unwrap();
        if let Some(winner) = (0..4).find(|&i| game.game.scores[i] == max_score) {
            *wins.entry(winner).or_insert(0) += 1;
        }

        // Calculate performance of experimental players (1 and 3)
        let experimental_performance = (game.result(1).unwrap_or(0.0) + game.result(3).unwrap_or(0.0)) / 2.0;
        total_score += experimental_performance;
    }

    (total_score / games as f64, wins)
}

fn main() {
    let experiments = [
        RewardFunction::LinearNormalized,
        RewardFunction::WinnerTakesAll,
        RewardFunction::ScoreDifference,
        RewardFunction::Exponential,
        RewardFunction::RankBased,
    ];

    let games_per_experiment = 20;
    let iterations = 100;

    println!("Running Stick 'Em reward function experiments...");
    println!("Games per experiment: {}", games_per_experiment);
    println!("MCTS iterations: {}", iterations);
    println!("Experimental players: 1, 3 (vs control players 0, 2)");
    println!();

    let mut results = Vec::new();

    for &reward_function in &experiments {
        println!("Testing {:?}...", reward_function);
        let (avg_performance, wins) = run_experiment(reward_function, games_per_experiment, iterations);

        println!("  Average performance: {:.4}", avg_performance);
        println!("  Win distribution: {:?}", wins);

        let experimental_wins = wins.get(&1).unwrap_or(&0) + wins.get(&3).unwrap_or(&0);
        let experimental_win_rate = experimental_wins as f64 / games_per_experiment as f64;
        println!("  Experimental players win rate: {:.2}%", experimental_win_rate * 100.0);
        println!();

        results.push((reward_function, avg_performance, experimental_win_rate, wins));
    }

    // Find best performing reward function
    results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().reverse());

    println!("=== RESULTS SUMMARY ===");
    for (i, (reward_function, avg_performance, win_rate, wins)) in results.iter().enumerate() {
        println!("{}. {:?}", i + 1, reward_function);
        println!("   Performance: {:.4}", avg_performance);
        println!("   Win rate: {:.2}%", win_rate * 100.0);
        println!("   Wins: {:?}", wins);
        println!();
    }

    println!("Best performing reward function: {:?}", results[0].0);
}