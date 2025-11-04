use tricksterstable_rs::games::trickorbid::{get_mcts_move, State, TrickOrBidGame};

fn main() {
    for i in 0..10 {
        let mut game = TrickOrBidGame::new();
        while game.state != State::GameOver {
            let iterations = 1000;
            let action = get_mcts_move(&game, iterations, false);
            game.apply_move(action);
        }
        println!("Game {}: scores: {:?}", i + 1, game.scores);
    }
}
