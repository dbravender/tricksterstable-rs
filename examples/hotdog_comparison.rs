use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use tricksterstable_rs::games::hotdog::HotdogGame;

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Random,
    Baseline,
    Improved,
}

fn get_mcts_move(game: &HotdogGame, engine: Engine) -> i32 {
    let iterations = 1000;
    let mut new_game = game.clone();
    new_game.no_changes = true;
    match engine {
        Engine::Random => {
            let mut moves = game.get_moves();
            moves.shuffle(&mut thread_rng());
            moves[0]
        }
        Engine::Baseline => {
            let mut ismcts = ismctsbaseline::IsmctsHandler::new(new_game);
            let parallel_threads: usize = 8;
            ismcts.run_iterations(
                parallel_threads,
                (iterations as f64 / parallel_threads as f64) as usize,
            );
            ismcts.best_move().expect("should have a move to make")
        }
        Engine::Improved => {
            let mut ismcts = ismcts::IsmctsHandler::new(new_game);
            let parallel_threads: usize = 8;
            ismcts.run_iterations(
                parallel_threads,
                (iterations as f64 / parallel_threads as f64) as usize,
            );
            ismcts.best_move().expect("should have a move to make")
        }
    }
}

fn main() {
    let mut wins: HashMap<Engine, i32> = HashMap::new();
    let mut scores: HashMap<Engine, i32> = HashMap::new();
    let mut durations: HashMap<Engine, Duration> = HashMap::new();
    let engines = [Engine::Baseline, Engine::Improved];
    for engine in engines {
        wins.insert(engine, 0);
        scores.insert(engine, 0);
        durations.insert(engine, Duration::new(0, 0));
    }

    for i in 0..1000 {
        let mut game = HotdogGame::new();
        while game.scores == [0, 0] {
            let engine = if game.current_player == 0 {
                engines[i % 2]
            } else {
                engines[(i + 1) % 2]
            };
            let duration = durations.get_mut(&engine).unwrap();
            let start = Instant::now();
            let action = get_mcts_move(&game, engine);
            *duration += start.elapsed();
            game.apply_move(action);
        }
        let max_score: i32 = *game.scores.iter().max().unwrap();
        let winner = if game.scores[0] == max_score { 0 } else { 1 };
        let wins = wins.get_mut(&engines[(i + winner) % 2]).unwrap();
        *wins += 1;
        let scores = scores.get_mut(&engines[(i + winner) % 2]).unwrap();
        *scores += game.scores[0];
    }
    println!(
        "wins: {:?}\nscores: {:?}\ndurations: {:?}",
        wins, scores, durations
    );
}
