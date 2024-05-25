use tricksterstable_rs::games::{yokai2p::get_mcts_move, yokai2p::Yokai2pGame};

fn main() {
    let mut game = Yokai2pGame::new();
    println!("{:?}", &game);
    while game.winner.is_none() {
        let action = get_mcts_move(&game, 250);

        game.apply_move(&action);
    }
    println!("{:?}", &game);
}
