use tricksterstable_rs::games::hotdog::{get_mcts_move, HotdogGame, State};

fn main() {
    for _ in 0..1000 {
        let mut game = HotdogGame::new();
        //println!("{:?}", &game);
        while game.scores == [0, 0] {
            let debug = match game.state {
                State::Bid => true,
                _ => false,
            };

            let action = if game.current_player == 0 {
                let iterations = match game.state {
                    // State::Bid
                    // | State::NameRelish
                    // | State::NameTrump
                    // | State::WorksSelectFirstTrickType => 50000,
                    _ => 1000,
                };
                get_mcts_move(&game, iterations, debug)
            } else {
                get_mcts_move(&game, 10, debug)
            };

            game.apply_move(action);
        }
        //println!("{:?}", &game);
        println!("{:?} ", &game.scores)
    }
}
