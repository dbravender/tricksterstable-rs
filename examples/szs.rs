use rand::{thread_rng, Rng};
use tricksterstable_rs::games::szs::{get_mcts_move, Game};

fn main() {
    let mut rnd = thread_rng();
    for _ in 0..1000 {
        let mut game = Game::new();
        game.dealer = if rnd.gen_range(0..100) > 50 { 0 } else { 1 };
        game.current_player = game.dealer;
        while game.scores == vec![0, 0, 0] {
            let iterations = 1000;
            let action = get_mcts_move(&game, iterations);
            game.apply_move(action);
        }
        println!("{:?} ", &game.scores)
    }
}
