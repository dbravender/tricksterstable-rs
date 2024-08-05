use tricksterstable_rs::games::{hotdog::get_mcts_move, hotdog::HotdogGame};

fn main() {
    for _ in 0..100 {
        let mut game = HotdogGame::new();
        //println!("{:?}", &game);
        while game.scores == [0, 0] {
            let action = if game.current_player == 0 {
                get_mcts_move(&game, 2000)
            } else {
                get_mcts_move(&game, 10)
            };

            game.apply_move(action);
        }
        //println!("{:?}", &game);
        println!("{:?} ", &game.scores)
    }
}
