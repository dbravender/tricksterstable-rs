use tricksterstable_rs::games::pala::{get_mcts_move, PalaGame};

fn main() {
    for _ in 0..100 {
        let mut game = PalaGame::new();
        while game.winner.is_none() {
            let iterations = 1000;
            let action = get_mcts_move(&game, iterations, false);
            game.apply_move(action);
        }
        println!("winner: {:?}", game.winner);
        println!("scores: {:?}", game.scores);
    }
}
