#![feature(test)]
extern crate test;

use rand::{rngs::StdRng, SeedableRng};
use test::{black_box, Bencher};
use tricksterstable_rs::{
    games::szs::{deck, Card, Game, Suit},
    utils::shuffle_and_divide_matching_cards,
};

fn szs_playthrough(no_changes: bool) {
    let mut game = Game::new();
    if no_changes {
        game.with_no_changes();
    }
    game.round = 4;
    while game.winner.is_none() {
        let action = *game.get_moves().first().unwrap();
        game = game.apply_move(action);
    }
}

#[bench]
fn bench_random_playthrough(b: &mut Bencher) {
    b.iter(|| {
        black_box(szs_playthrough(false));
    })
}

#[bench]
fn bench_random_playthrough_no_changes(b: &mut Bencher) {
    b.iter(|| {
        black_box(szs_playthrough(true));
    })
}

#[bench]
fn test_random_shuffles(b: &mut Bencher) {
    b.iter(|| {
        // let deck = new_deck();
        // let mut hands = vec![deck. , deck.take(5).collect()];
        let binding = deck();
        let mut deck_iter = binding.iter();
        let hand1: Vec<Card> = deck_iter.by_ref().take(5).cloned().collect();
        let hand2: Vec<Card> = deck_iter.take(5).cloned().collect();
        let mut hands = vec![hand1, hand2];
        let mut rng = StdRng::seed_from_u64(42);
        black_box(shuffle_and_divide_matching_cards(
            |c: &Card| c.suit != Suit::Red && c.suit != Suit::Blue,
            &mut hands,
            &mut rng,
        ));
    })
}
