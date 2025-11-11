use rand::{seq::SliceRandom, thread_rng};
use tricksterstable_rs::games::stickem::{get_mcts_move, State, StickEmGame};

fn main() {
    let rng = thread_rng();
    for _ in 0..100 {
        let mut game = StickEmGame::new();
        while game.state != State::GameOver {
            let iterations = 1000;
            let action = if [1, 3].contains(&game.current_player) {
                //let actions = game.get_moves();
                //*actions.choose(&mut rng).unwrap()

                game.experiment = true;
                get_mcts_move(&game, iterations, false)
            } else {
                game.experiment = false;
                get_mcts_move(&game, iterations, false)
            };
            game.apply_move(action);
        }
        println!("scores: {:?}", game.scores);
    }
}
