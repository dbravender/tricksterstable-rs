use rand::{seq::SliceRandom, thread_rng};
use tricksterstable_rs::games::pala::{get_mcts_move, PalaGame};

fn main() {
    let mut rng = thread_rng();
    for _ in 0..100 {
        let mut game = PalaGame::new();
        while game.winner.is_none() {
            let iterations = 1000;
            let action = if [1, 2, 3].contains(&game.current_player) {
                let actions = game.get_moves();
                *actions.choose(&mut rng).unwrap()
            } else {
                get_mcts_move(&game, iterations, false)
            };
            game.apply_move(action);
        }
        println!("winner: {:?}", game.winner);
        println!("scores: {:?}", game.scores);
    }
}
