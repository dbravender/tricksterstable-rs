use rand::seq::SliceRandom;
use tricksterstable_rs::games::otter1p::OtterGame;

fn main() {
    let mut game = OtterGame::new();
    while !game.get_moves().is_empty() {
        let mut moves = game.get_moves();
        moves.shuffle(&mut rand::thread_rng());
        game.apply_move(*moves.first().unwrap());
        println!("state: {:?}", &game.state);
        println!("remaining lucky stones: {:?}", &game.lucky_stones);
    }
}
