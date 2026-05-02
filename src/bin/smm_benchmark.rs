// Benchmark different ISMCTS result evaluation strategies for SMM.
//
// Strategies:
// 1. FullGame  - simulate all 3 rounds (current behavior)
// 2. RoundOnly - simulate to end of current round only
//
// Each strategy plays as MCTS player 0 against two random opponents.
// We run N games per strategy and compare win rates.

use ismcts::IsmctsHandler;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use std::time::Instant;
use tricksterstable_rs::games::smm::{ResultMode, SMMGame};

const ITERATIONS: usize = 1000;
const PARALLEL_THREADS: usize = 8;
const GAMES_PER_MATCHUP: usize = 200;

fn mcts_move(game: &SMMGame, mode: ResultMode) -> i32 {
    let mut sim_game = game.clone();
    sim_game.no_changes = true;
    sim_game.result_mode = mode;
    let mut ismcts = IsmctsHandler::new(sim_game);
    ismcts.run_iterations(PARALLEL_THREADS, ITERATIONS / PARALLEL_THREADS);
    ismcts.best_move().expect("should have a move to make")
}

fn random_move(game: &SMMGame) -> i32 {
    let mut moves = game.get_moves();
    moves.shuffle(&mut thread_rng());
    *moves.first().expect("should have a move")
}

fn play_game_mcts_vs_random(mode: ResultMode) -> [i32; 3] {
    let mut game = SMMGame::new();
    game.no_changes = true;

    while game.winner.is_none() {
        let moves = game.get_moves();
        if moves.is_empty() {
            break;
        }

        let mov = if game.current_player == 0 {
            mcts_move(&game, mode)
        } else {
            random_move(&game)
        };
        game.apply_move(mov);
    }
    game.scores
}

/// Pit two MCTS strategies against each other + one random player.
/// Player 0 = strategy_a, Player 1 = strategy_b, Player 2 = random.
fn play_game_head_to_head(strategy_a: ResultMode, strategy_b: ResultMode) -> [i32; 3] {
    let mut game = SMMGame::new();
    game.no_changes = true;

    while game.winner.is_none() {
        let moves = game.get_moves();
        if moves.is_empty() {
            break;
        }

        let mov = match game.current_player {
            0 => mcts_move(&game, strategy_a),
            1 => mcts_move(&game, strategy_b),
            _ => random_move(&game),
        };
        game.apply_move(mov);
    }
    game.scores
}

fn main() {
    println!("=== SMM ISMCTS Strategy Benchmark ===\n");

    // Phase 1: Each strategy vs random opponents
    println!("--- Phase 1: MCTS (player 0) vs 2 Random ---");
    for mode in [ResultMode::FullGame, ResultMode::RoundOnly] {
        let mode_name = match mode {
            ResultMode::FullGame => "FullGame",
            ResultMode::RoundOnly => "RoundOnly",
        };

        let start = Instant::now();
        let mut wins = 0;
        let mut ties = 0;
        let mut total_score = 0;
        let mut total_games = 0;

        for i in 0..GAMES_PER_MATCHUP {
            if i % 10 == 0 {
                eprint!(".");
            }
            let scores = play_game_mcts_vs_random(mode);
            total_games += 1;
            total_score += scores[0];
            let max = *scores.iter().max().unwrap();
            if scores[0] == max {
                if scores.iter().filter(|&&s| s == max).count() > 1 {
                    ties += 1;
                } else {
                    wins += 1;
                }
            }
        }
        eprintln!();

        let elapsed = start.elapsed();
        println!(
            "{}: wins={}/{} ({:.1}%), ties={}, avg_score={:.1}, time={:.1}s",
            mode_name,
            wins,
            total_games,
            100.0 * wins as f64 / total_games as f64,
            ties,
            total_score as f64 / total_games as f64,
            elapsed.as_secs_f64()
        );
    }

    // Phase 2: Head-to-head
    println!("\n--- Phase 2: Head-to-Head (P0=FullGame, P1=RoundOnly, P2=Random) ---");
    {
        let start = Instant::now();
        let mut score_totals = [0i64; 3];
        let mut win_counts = [0usize; 3];

        for i in 0..GAMES_PER_MATCHUP {
            if i % 10 == 0 {
                eprint!(".");
            }
            let scores = play_game_head_to_head(ResultMode::FullGame, ResultMode::RoundOnly);
            let max = *scores.iter().max().unwrap();
            for p in 0..3 {
                score_totals[p] += scores[p] as i64;
                if scores[p] == max && scores.iter().filter(|&&s| s == max).count() == 1 {
                    win_counts[p] += 1;
                }
            }
        }
        eprintln!();

        let elapsed = start.elapsed();
        let labels = ["FullGame", "RoundOnly", "Random"];
        for p in 0..3 {
            println!(
                "  {}: wins={}/{} ({:.1}%), avg_score={:.1}",
                labels[p],
                win_counts[p],
                GAMES_PER_MATCHUP,
                100.0 * win_counts[p] as f64 / GAMES_PER_MATCHUP as f64,
                score_totals[p] as f64 / GAMES_PER_MATCHUP as f64
            );
        }
        println!("  time={:.1}s", elapsed.as_secs_f64());
    }

    // Phase 3: Reversed head-to-head (swap positions)
    println!("\n--- Phase 3: Head-to-Head (P0=RoundOnly, P1=FullGame, P2=Random) ---");
    {
        let start = Instant::now();
        let mut score_totals = [0i64; 3];
        let mut win_counts = [0usize; 3];

        for i in 0..GAMES_PER_MATCHUP {
            if i % 10 == 0 {
                eprint!(".");
            }
            let scores = play_game_head_to_head(ResultMode::RoundOnly, ResultMode::FullGame);
            let max = *scores.iter().max().unwrap();
            for p in 0..3 {
                score_totals[p] += scores[p] as i64;
                if scores[p] == max && scores.iter().filter(|&&s| s == max).count() == 1 {
                    win_counts[p] += 1;
                }
            }
        }
        eprintln!();

        let elapsed = start.elapsed();
        let labels = ["RoundOnly", "FullGame", "Random"];
        for p in 0..3 {
            println!(
                "  {}: wins={}/{} ({:.1}%), avg_score={:.1}",
                labels[p],
                win_counts[p],
                GAMES_PER_MATCHUP,
                100.0 * win_counts[p] as f64 / GAMES_PER_MATCHUP as f64,
                score_totals[p] as f64 / GAMES_PER_MATCHUP as f64
            );
        }
        println!("  time={:.1}s", elapsed.as_secs_f64());
    }

    println!("\nDone.");
}
