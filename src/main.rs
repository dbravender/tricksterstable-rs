use std::io::{self, Write};

use rand::{seq::SliceRandom, thread_rng};

pub mod games;

fn main() {
    let mut scores = vec![0, 0, 0];
    for _ in 0..1000 {
        let mut game = games::szs::Game::new();
        while game.winner == None {
            let mut actions = game.get_moves();
            actions.shuffle(&mut thread_rng());
            game = game.clone_and_apply_move(*actions.first().expect("should have a move to make"));
        }
        print!(".");
        io::stdout().flush().unwrap();
        scores = (0..3).map(|x| scores[x] + game.scores[x]).collect();
    }
    println!("\n{:?}", scores);
}
