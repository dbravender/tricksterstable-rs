pub mod games;

fn main() {
    println!("{:?}", serde_json::to_string(&games::szs::deck()).unwrap());
    println!("{:?}", serde_json::to_string(&games::szs::Game::new()).unwrap());
}
