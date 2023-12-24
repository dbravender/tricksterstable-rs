use rand::{seq::SliceRandom, thread_rng};

pub mod games;

fn main() {
    //println!("{:?}", serde_json::to_string(&games::szs::deck()).unwrap());
    //println!(
    //    "{:?}",
    //   serde_json::to_string(&games::szs::Game::new()).unwrap()
    // );
    for _ in 0..100 {
        let mut game = games::szs::Game::new();
        while game.winner == None {
            let mut actions = game.get_moves();
            actions.shuffle(&mut thread_rng());
            if actions.is_empty() {
                //println!("{}", serde_json::to_string(&game).unwrap());

                print!("X");
                break;
            }
            game = game.clone_and_apply_move(*actions.first().expect("should have a move to make"));
            print!(".");
        }
        //println!("{}", serde_json::to_string(&game).unwrap());
    }
}
