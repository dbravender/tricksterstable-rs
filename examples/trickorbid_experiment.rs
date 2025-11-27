use ismcts::Game;
use rand::{seq::SliceRandom, thread_rng};
use std::collections::HashMap;
use tricksterstable_rs::games::trickorbid::{State, TrickOrBidGame};

// Different reward function experiments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Strategy {
    Random,
    LinearNormalized,
    WinnerTakesAll,
    Exponential, // Current default
}

// Modified game that can use different reward functions
#[derive(Debug, Clone)]
struct ExperimentalTrickOrBidGame {
    game: TrickOrBidGame,
    strategy: Strategy,
}

impl ExperimentalTrickOrBidGame {
    fn new(strategy: Strategy) -> Self {
        Self {
            game: TrickOrBidGame::new(),
            strategy,
        }
    }
}

impl Game for ExperimentalTrickOrBidGame {
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

        match self.strategy {
            Strategy::Random => {
                // Random doesn't use reward function, but we still need to return something
                // Use linear for consistency
                if max_score == min_score {
                    return Some(0.0);
                }
                let normalized = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;
                Some(normalized)
            }
            Strategy::LinearNormalized => {
                if max_score == min_score {
                    return Some(0.0);
                }
                let normalized = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;
                Some(normalized)
            }
            Strategy::WinnerTakesAll => {
                if player_score == max_score {
                    if scores.iter().filter(|&&s| s == scores[player]).count() > 1 {
                        Some(0.0) // Tie
                    } else {
                        Some(1.0) // Win
                    }
                } else {
                    Some(-1.0) // Lose
                }
            }
            Strategy::Exponential => {
                // Current default implementation
                if max_score == min_score {
                    return Some(0.0);
                }
                let linear = 2.0 * (player_score - min_score) / (max_score - min_score) - 1.0;
                Some(linear.signum() * linear.abs().powf(2.0))
            }
        }
    }
}

// Round-robin tournament where each strategy plays against all others
fn run_round_robin(
    games_per_matchup: usize,
    iterations: i32,
) -> HashMap<Strategy, (i32, i32, i32)> {
    let strategies = vec![
        Strategy::Random,
        Strategy::LinearNormalized,
        Strategy::WinnerTakesAll,
        Strategy::Exponential,
    ];

    let mut results: HashMap<Strategy, (i32, i32, i32)> = HashMap::new();
    for &strategy in &strategies {
        results.insert(strategy, (0, 0, 0)); // (wins, total_score, games_played)
    }

    let mut rng = thread_rng();

    // Each matchup: all 4 strategies play together
    println!("Running round-robin tournament...");
    println!("Strategies: {:?}", strategies);
    println!();

    for game_num in 0..games_per_matchup {
        if (game_num + 1) % 10 == 0 {
            println!("Completed {} games...", game_num + 1);
        }

        // Shuffle player positions for fairness
        let mut player_strategies = strategies.clone();
        player_strategies.shuffle(&mut rng);

        let mut game = TrickOrBidGame::new();

        while game.state != State::GameOver {
            let current_player = game.current_player;
            let player_strategy = player_strategies[current_player];

            let action = if player_strategy == Strategy::Random {
                // Random player
                let moves = game.get_moves();
                *moves.choose(&mut rng).unwrap()
            } else {
                // MCTS player with specific reward function
                let mut experimental_game = ExperimentalTrickOrBidGame {
                    game: game.clone(),
                    strategy: player_strategy,
                };
                experimental_game.game.no_changes = true;
                let mut ismcts = ismcts::IsmctsHandler::new(experimental_game);
                let parallel_threads: usize = 8;
                ismcts.run_iterations(
                    parallel_threads,
                    (iterations as f64 / parallel_threads as f64) as usize,
                );
                ismcts.best_move().expect("should have a move to make")
            };

            game.apply_move(action);
        }

        // Record results
        let max_score = *game.scores.iter().max().unwrap();
        for (player, &strategy) in player_strategies.iter().enumerate() {
            let (wins, total_score, games_played) = results.get(&strategy).unwrap();
            let new_wins = if game.scores[player] == max_score {
                wins + 1
            } else {
                *wins
            };
            results.insert(
                strategy,
                (
                    new_wins,
                    total_score + game.scores[player],
                    games_played + 1,
                ),
            );
        }
    }

    results
}

fn main() {
    let games_per_matchup = 100;
    let iterations = 1000;

    println!("=== Trick or Bid Strategy Experiment ===");
    println!("Games: {}", games_per_matchup);
    println!("MCTS iterations: {}", iterations);
    println!();

    let results = run_round_robin(games_per_matchup, iterations);

    println!();
    println!("=== RESULTS ===");
    println!();

    let mut sorted_results: Vec<_> = results.iter().collect();
    sorted_results.sort_by_key(|(_, (wins, _, _))| std::cmp::Reverse(*wins));

    for (strategy, (wins, total_score, games_played)) in &sorted_results {
        let win_rate = *wins as f64 / *games_played as f64 * 100.0;
        let avg_score = *total_score as f64 / *games_played as f64;
        println!("{:?}:", strategy);
        println!("  Wins: {} / {} ({:.1}%)", wins, games_played, win_rate);
        println!("  Average score: {:.2}", avg_score);
        println!();
    }

    println!("Best performing strategy: {:?}", sorted_results[0].0);
}
