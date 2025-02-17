use tricksterstable_rs::games::torchlit::{get_mcts_move, TorchlitGame};

fn main() {
    let mut game = TorchlitGame::new();
    while game.winner.is_none() {
        println!("moves: {:?}", game.get_moves());
        println!("state: {:?}", game.state);
        let action = get_mcts_move(&game, 10 + (game.current_player as i32 * 250), false);
        game.apply_move(action);
    }
    println!("winner: {:?}", game.winner);
    println!("scores: {:?}", game.scores);
}
