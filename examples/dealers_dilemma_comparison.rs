use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;

use tricksterstable_rs::games::dealers_dilemma::Game;

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Random,
    Baseline,
    DivideByZeroFix,
}

fn get_mcts_move(game: &Game, engine: Engine) -> i32 {
    let iterations = 500;
    let mut new_game = game.clone();
    new_game.round = 6;
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
        Engine::DivideByZeroFix => {
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
    let engines = [Engine::Random, Engine::Baseline, Engine::DivideByZeroFix];
    for engine in engines {
        wins.insert(engine, 0);
        scores.insert(engine, 0);
    }

    for i in 0..1000 {
        let mut game = Game::new();
        game = game.deal();
        game.round = 6;
        let engine = engines[i % 3];
        while game.winner.is_none() {
            if game.current_player == 0 {
                let action = get_mcts_move(&game, engine);
                game = game.clone_and_apply_move(action);
            } else {
                let mut moves = game.get_moves();
                moves.shuffle(&mut thread_rng());
                game = game.clone_and_apply_move(moves[0]);
            }
        }
        let max_score: i32 = *game.scores.iter().max().unwrap();
        if game.scores[0] == max_score {
            let wins = wins.get_mut(&engine).unwrap();
            *wins += 1;
        }
        let scores = scores.get_mut(&engine).unwrap();
        *scores += game.scores[0];
    }
    println!("wins: {:?} scores: {:?}", wins, scores);
}
