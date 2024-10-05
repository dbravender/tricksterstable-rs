use rand::{seq::SliceRandom, Rng};

/// Shuffle and exchanges items matching criteria between two lists
/// Used when determining possible cards a player could have in their
/// hand given the current state of a game.
pub fn shuffle_and_divide_matching_cards<T: Copy>(
    matcher: impl Fn(&T) -> bool,
    hands: &mut Vec<Vec<T>>,
    rng: &mut impl Rng,
) {
    let mut hand_locations = vec![
        Vec::with_capacity(hands[0].len()),
        Vec::with_capacity(hands[1].len()),
    ];
    // Pre-allocate array so we don't spend time growing the array
    // (might waste a little space but should get more performance)
    let mut matched_cards: Vec<T> = Vec::with_capacity(hands[0].len() + hands[1].len());

    // Find all cards that match the criteria
    for hand_index in 0..2 {
        for (card_index, card) in hands[hand_index].iter().enumerate() {
            if matcher(card) {
                hand_locations[hand_index].push(card_index);
                matched_cards.push(card.clone());
            }
        }
    }

    // Shuffle the matching cards
    matched_cards.shuffle(rng);

    // Redistribute the matching cards
    for hand_index in 0..2 {
        for card_index in hand_locations[hand_index].iter() {
            hands[hand_index][*card_index] = matched_cards
                .pop()
                .expect("there should be a card left to pop")
        }
    }

    // All the matching cards were redistributed
    assert!(matched_cards.len() == 0);
}

pub mod tests {
    use enum_iterator::{all, Sequence};
    use rand::{seq::SliceRandom, thread_rng};

    #[derive(Debug, Clone, Copy, PartialEq, Sequence, PartialOrd, Ord, Eq)]
    enum Suit {
        Hearts,
        Clubs,
        Spades,
        Diamonds,
    }

    #[derive(Debug, Clone, PartialEq, Copy)]
    pub struct Card {
        value: i32,
        suit: Suit,
    }

    pub fn new_deck() -> Vec<Card> {
        let mut cards: Vec<Card> = vec![];
        for suit in all::<Suit>() {
            for value in 1..14 {
                cards.push(Card { value, suit });
            }
        }
        let mut rng = thread_rng();
        cards.shuffle(&mut rng);
        cards
    }

    #[test]
    fn test_shuffle_and_divide_matching_cards() {
        let mut hands = vec![
            vec![
                Card {
                    value: 1,
                    suit: Suit::Diamonds,
                },
                Card {
                    value: 1,
                    suit: Suit::Diamonds,
                },
                Card {
                    value: 1,
                    suit: Suit::Hearts,
                },
                Card {
                    value: 2,
                    suit: Suit::Hearts,
                },
            ],
            vec![
                Card {
                    value: 2,
                    suit: Suit::Clubs,
                },
                Card {
                    value: 2,
                    suit: Suit::Clubs,
                },
                Card {
                    value: 1,
                    suit: Suit::Spades,
                },
                Card {
                    value: 2,
                    suit: Suit::Spades,
                },
            ],
        ];

        // Through earlier play we know that player 1 has no spades and player 2 has no
        // hearts so we only exchange all non-spade non-heart cards
        let mut rng = StdRng::seed_from_u64(42);
        shuffle_and_divide_matching_cards(
            |c| c.suit != Suit::Spades && c.suit != Suit::Hearts,
            &mut hands,
            &mut rng,
        );
        assert_eq!(
            hands[0],
            vec![
                Card {
                    value: 2,
                    suit: Suit::Clubs
                },
                Card {
                    value: 1,
                    suit: Suit::Diamonds
                },
                Card {
                    value: 1,
                    suit: Suit::Hearts
                },
                Card {
                    value: 2,
                    suit: Suit::Hearts
                }
            ]
        );
        assert_eq!(
            hands[1],
            vec![
                Card {
                    value: 1,
                    suit: Suit::Diamonds
                },
                Card {
                    value: 2,
                    suit: Suit::Clubs
                },
                Card {
                    value: 1,
                    suit: Suit::Spades
                },
                Card {
                    value: 2,
                    suit: Suit::Spades
                }
            ]
        );
    }
}
