/*
Game: 3 Tricky Pigs
Designers: Andrew Stiles and Steven Ungaro
BoardGameGeek: https://boardgamegeek.com/boardgame/441614/3-tricky-pigs
*/

/*

Planned flow:
- Player can stage huff cards, puff cards and then a card from their hand
- As soon as a card is played from the hand the play is committed
- We'll play the actual card on top of the huff and puff cards
*/

const PLAYER_COUNT: usize = 4;
const HAND_SIZE: usize = 12;
const ROUNDS: usize = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Suit {
    Straw,
    Sticks,
    Bricks,
    Wolf,
    Huff,
    Puff,
}

#[derive(Copy, Clone)]
enum Bid {
    /// Try to win 0 tricks
    Sleep,
    /// Win 2 tricks
    Play,
    /// Win 3 or more tricks
    Work,
    /// Win the most tricks of any player
    Eat,
}

#[derive(Copy, Clone)]
enum State {
    /// Select one of the bid cards
    Bid,
    /// Standard must-follow trick taking
    Play,
}

#[derive(Copy, Clone, Debug)]
struct Card {
    value: i32,
    suit: Suit,
    id: i32,
}

impl Card {
    fn is_puff(&self) -> bool {
        self.suit == Suit::Puff
    }
    fn is_huff(&self) -> bool {
        self.suit == Suit::Huff
    }
    fn is_regular(&self) -> bool {
        !self.is_huff() && !self.is_puff()
    }
}

struct ThreeTrickyPigsGame {
    /// Current game state
    state: State,
    /// Current player
    current_player: usize,
    /// Lead player for the current trick
    lead_player: usize,
    /// Whether or not the Wolf suit has been broken yet
    wolf_suit_broken: bool,
    /// Regular cards in a trick (indexed by player)
    current_trick_regular: [Option<Card>; PLAYER_COUNT],
    /// Huff cards in the current trick (indexed by player)
    current_trick_huff: [Option<Card>; PLAYER_COUNT],
    /// Puff cards in the current trick (indexed by player)
    current_trick_puff: [Option<Card>; PLAYER_COUNT],
    /// Each player's hand
    hands: [Vec<Card>; PLAYER_COUNT],
    /// Each player's current bid
    bids: [Option<Bid>; PLAYER_COUNT],
}

impl ThreeTrickyPigsGame {
    /// Returns possible moves
    fn get_moves(&self) -> Vec<i32> {
        match self.state {
            // 4 bid options
            State::Bid => (0..=3).collect(),
            // Regular trick play
            State::Play => {
                let lead_suit = self.current_trick_regular[self.lead_player].map(|c| c.suit);
                let puff_played = self.current_trick_puff[self.current_player].is_some();
                let huff_played = self.current_trick_huff[self.current_player].is_some();
                let hand = &self.hands[self.current_player];
                let is_leading = lead_suit.is_none();

                // Find playable regular cards

                // Find all cards in the current lead suit
                let follow_suit_cards: Vec<&Card> = hand
                    .iter()
                    .filter(|c| lead_suit.map_or(false, |s| c.suit == s))
                    .collect();

                let playable_regular_cards: Vec<&Card> = if follow_suit_cards.is_empty() {
                    // No lead card or no cards in lead suit - any regular card in hand
                    // can be played (but wolves only if broken or not leading)
                    hand.iter()
                        .filter(|c| {
                            c.is_regular()
                                && (c.suit != Suit::Wolf || self.wolf_suit_broken || !is_leading)
                        })
                        .collect()
                } else {
                    // Must follow suit in 3 Tricky Pigs
                    follow_suit_cards
                };

                // Find playable huff and puff cards
                let playable_huff_and_puff_cards = hand
                    .iter()
                    .filter(|c| (!puff_played && c.is_puff()) || (!huff_played && c.is_huff()));

                // Return all playable cards
                playable_regular_cards
                    .into_iter()
                    .chain(playable_huff_and_puff_cards)
                    .map(|c| c.id)
                    .collect()
            }
        }
    }
}

fn deck() -> Vec<Card> {
    let distributions: Vec<(Suit, Vec<i32>)> = vec![
        (Suit::Straw, (1..=10).collect()),
        (Suit::Sticks, (1..=10).collect()),
        (Suit::Bricks, (21..=30).collect()),
        (
            Suit::Wolf,
            vec![0, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 31],
        ),
        (Suit::Huff, (1..=4).collect()),
        (Suit::Puff, (2..=5).collect()),
    ];
    let mut id = 0;
    let mut cards: Vec<Card> = vec![];
    for (suit, values) in &distributions {
        for value in values {
            cards.push(Card {
                value: *value,
                suit: *suit,
                id: id,
            });
            id += 1;
        }
    }
    cards
}

/// Returns the player index that won the trick
fn trick_winner(
    lead_player: usize,
    trick_regular: [Option<Card>; PLAYER_COUNT],
    trick_huff: [Option<Card>; PLAYER_COUNT],
    trick_puff: [Option<Card>; PLAYER_COUNT],
) -> usize {
    let empty_card = Card {
        id: -1,
        value: 0,
        suit: Suit::Bricks,
    };
    let mut winning_player = lead_player;
    let mut winning_value = trick_regular[lead_player].unwrap().value;
    let contains_wolf = trick_regular.iter().any(|c| c.unwrap().suit == Suit::Wolf);
    for offset in 0..PLAYER_COUNT {
        let current_player = (offset + lead_player) % PLAYER_COUNT;
        let card_value = trick_regular[current_player].unwrap().value
            + trick_huff[current_player].unwrap_or(empty_card).value
            + trick_puff[current_player].unwrap_or(empty_card).value;
        let winning = if contains_wolf {
            // When at least one wolf card is in the trick the highest card wins
            // (later played cards win ties)
            card_value >= winning_value
        } else {
            // When all cards are pig cards the lowest value card wins
            // (later played cards win ties)
            card_value <= winning_value
        };
        if winning {
            winning_value = card_value;
            winning_player = current_player;
        }
    }
    winning_player
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_composition() {
        let d = deck();
        // Total card count
        assert_eq!(d.len(), 50);
    }

    // Helper to create a card
    fn card(value: i32, suit: Suit) -> Option<Card> {
        Some(Card {
            id: -1,
            value,
            suit,
        })
    }

    // Helper for no huff/puff
    fn no_modifiers() -> [Option<Card>; PLAYER_COUNT] {
        [None; PLAYER_COUNT]
    }

    // Rulebook Example 6: No wolf, lowest card wins
    // Player 0 leads 5, Player 1 plays 10+5 puff, Player 2 plays 2
    // Player 2 wins with lowest value (2)
    #[test]
    fn test_no_wolf_lowest_wins() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(10, Suit::Straw),
            card(2, Suit::Straw),
            card(7, Suit::Straw),
        ];
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 2);
    }

    // Rulebook Example (wolf present): Player 0 leads 5, Player 1 plays 10+5 puff,
    // Player 2 can't follow and plays 14 (wolf), highest card wins
    // 10+5=15 beats 14, so Player 1 wins
    #[test]
    fn test_wolf_present_highest_wins() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(10, Suit::Straw),
            card(14, Suit::Wolf),
            card(8, Suit::Bricks),
        ];
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 1); // 10+5=15 is highest
    }

    // Rulebook Example 7: No wolf, off-suit doesn't matter for winner calculation
    // Player 0 leads 21, Player 1 plays 8 (off-suit)
    // In a 2-relevant-player scenario, 8 < 21, so Player 1 would win (lowest)
    #[test]
    fn test_no_wolf_lower_value_wins_regardless_of_suit() {
        let trick_regular = [
            card(21, Suit::Bricks),
            card(8, Suit::Straw),
            card(25, Suit::Bricks),
            card(28, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 1); // 8 is lowest
    }

    // Rulebook Example 8: Tie goes to last played card
    // Player 0 leads 5, Player 1 plays 28, Player 2 plays 8, Player 3 plays 8
    // No wolf, lowest wins. 5 is lowest, Player 0 wins.
    // (Separate test below covers the tie-breaker scenario)
    #[test]
    fn test_no_wolf_example_8() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(28, Suit::Straw),
            card(8, Suit::Straw),
            card(8, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0); // 5 is lowest, Player 0 wins
    }

    // Tie-breaker test: Two players have the same lowest value
    // No wolf, lowest wins. Two 5s tie, last played wins.
    #[test]
    fn test_no_wolf_tie_goes_to_last_played() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(28, Suit::Straw),
            card(8, Suit::Straw),
            card(5, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3); // Last played 5 wins the tie
    }

    // Wolf present, tie goes to last played (highest)
    #[test]
    fn test_wolf_present_tie_goes_to_last_played() {
        let trick_regular = [
            card(15, Suit::Wolf),
            card(10, Suit::Straw),
            card(15, Suit::Sticks),
            card(8, Suit::Bricks),
        ];

        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 2); // Second 15 wins (last played among ties)
    }

    // Test huff modifier adds to value
    #[test]
    fn test_huff_adds_to_value() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(3, Suit::Straw),
            card(6, Suit::Straw),
            card(7, Suit::Straw),
        ];
        // Player 1 has 3, but +4 huff makes it 7
        let trick_huff = [None, card(4, Suit::Huff), None, None];

        let winner = trick_winner(0, trick_regular, trick_huff, no_modifiers());
        // Without huff: 3 wins (lowest). With huff: 3+4=7, so 5 is now lowest
        assert_eq!(winner, 0);
    }

    // Test huff and puff together
    #[test]
    fn test_huff_and_puff_combined() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(2, Suit::Straw),
            card(6, Suit::Straw),
            card(7, Suit::Straw),
        ];
        // Player 1 has 2, but +3 huff +4 puff makes it 9
        let trick_huff = [None, card(3, Suit::Huff), None, None];
        let trick_puff = [None, card(4, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, trick_huff, trick_puff);
        // 2+3+4=9, so 5 is now lowest
        assert_eq!(winner, 0);
    }

    // Test with non-zero lead player
    #[test]
    fn test_lead_player_not_zero() {
        let trick_regular = [
            card(10, Suit::Straw), // Player 0
            card(8, Suit::Straw),  // Player 1
            card(5, Suit::Straw),  // Player 2 (leads)
            card(9, Suit::Straw),  // Player 3
        ];

        // Player 2 leads, play order is 2, 3, 0, 1
        let winner = trick_winner(2, trick_regular, no_modifiers(), no_modifiers());
        // No wolf, lowest wins: 5 (P2), 9 (P3), 10 (P0), 8 (P1)
        // Lowest is 5, Player 2 wins
        assert_eq!(winner, 2);
    }

    // Test lead player wrap-around with tie
    #[test]
    fn test_lead_player_wraparound_tie() {
        let trick_regular = [
            card(5, Suit::Straw),  // Player 0
            card(8, Suit::Straw),  // Player 1
            card(5, Suit::Sticks), // Player 2 (leads)
            card(9, Suit::Straw),  // Player 3
        ];

        // Player 2 leads, play order is 2, 3, 0, 1
        // Tie between P2 (5) and P0 (5), P0 plays later, P0 wins
        let winner = trick_winner(2, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0);
    }

    // Wolf card with value 0 (special wolf card)
    #[test]
    fn test_wolf_zero_value() {
        let trick_regular = [
            card(0, Suit::Wolf),
            card(5, Suit::Straw),
            card(3, Suit::Sticks),
            card(2, Suit::Bricks),
        ];

        // Wolf present, highest wins. 5 is highest.
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 1);
    }

    // Wolf card with value 31 (highest wolf)
    #[test]
    fn test_wolf_31_value() {
        let trick_regular = [
            card(31, Suit::Wolf),
            card(30, Suit::Bricks),
            card(28, Suit::Bricks),
            card(25, Suit::Bricks),
        ];

        // Wolf present, highest wins. 31 is highest.
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 0);
    }

    // Multiple wolves in trick
    #[test]
    fn test_multiple_wolves() {
        let trick_regular = [
            card(11, Suit::Wolf),
            card(15, Suit::Wolf),
            card(15, Suit::Wolf),
            card(12, Suit::Wolf),
        ];

        // All wolves, highest wins, tie goes to last played
        // P1 and P2 both have 15, P2 played later
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 2);
    }

    // Huff/puff with wolf - modifiers still apply
    #[test]
    fn test_wolf_with_modifiers() {
        let trick_regular = [
            card(14, Suit::Wolf),
            card(10, Suit::Straw),
            card(8, Suit::Sticks),
            card(7, Suit::Bricks),
        ];
        // Player 1 has 10 + 5 puff = 15, beats wolf 14
        let trick_puff = [None, card(5, Suit::Puff), None, None];

        let winner = trick_winner(0, trick_regular, no_modifiers(), trick_puff);
        assert_eq!(winner, 1);
    }

    // All same values, last player wins (no wolf)
    #[test]
    fn test_all_same_value_no_wolf() {
        let trick_regular = [
            card(5, Suit::Straw),
            card(5, Suit::Sticks),
            card(5, Suit::Bricks),
            card(5, Suit::Straw),
        ];

        // All 5s, no wolf, last played wins (Player 3)
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3);
    }

    // All same values, last player wins (with wolf)
    #[test]
    fn test_all_same_value_with_wolf() {
        let trick_regular = [
            card(15, Suit::Wolf),
            card(15, Suit::Sticks),
            card(15, Suit::Bricks),
            card(15, Suit::Straw),
        ];

        // All 15s, wolf present, last played wins (Player 3)
        let winner = trick_winner(0, trick_regular, no_modifiers(), no_modifiers());
        assert_eq!(winner, 3);
    }

    // ==================== get_moves tests ====================

    // Helper to create a card with id
    fn card_with_id(id: i32, value: i32, suit: Suit) -> Card {
        Card { id, value, suit }
    }

    // Helper to create a game with specific state
    fn game_with_hand(
        current_player: usize,
        lead_player: usize,
        hand: Vec<Card>,
        trick_regular: [Option<Card>; PLAYER_COUNT],
        trick_huff: [Option<Card>; PLAYER_COUNT],
        trick_puff: [Option<Card>; PLAYER_COUNT],
        wolf_suit_broken: bool,
    ) -> ThreeTrickyPigsGame {
        let mut hands: [Vec<Card>; PLAYER_COUNT] = Default::default();
        hands[current_player] = hand;
        ThreeTrickyPigsGame {
            state: State::Play,
            current_player,
            lead_player,
            wolf_suit_broken,
            current_trick_regular: trick_regular,
            current_trick_huff: trick_huff,
            current_trick_puff: trick_puff,
            hands,
            bids: [None; PLAYER_COUNT],
        }
    }

    // Bid state returns all 4 bid options
    #[test]
    fn test_get_moves_bid_state() {
        let game = ThreeTrickyPigsGame {
            state: State::Bid,
            current_player: 0,
            lead_player: 0,
            wolf_suit_broken: false,
            current_trick_regular: no_modifiers(),
            current_trick_huff: no_modifiers(),
            current_trick_puff: no_modifiers(),
            hands: Default::default(),
            bids: [None; PLAYER_COUNT],
        };
        let moves = game.get_moves();
        assert_eq!(moves, vec![0, 1, 2, 3]);
    }

    // Leading player (no lead card) can play any card
    #[test]
    fn test_get_moves_leading_player_any_card() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 10, Suit::Sticks),
            card_with_id(2, 25, Suit::Bricks),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // All cards playable when leading
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
    }

    // Must follow suit when able
    #[test]
    fn test_get_moves_must_follow_suit() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 7, Suit::Straw),
            card_with_id(2, 25, Suit::Bricks),
        ];

        // Player 0 led with Straw
        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // Only Straw cards playable
        assert!(moves.contains(&0)); // Straw
        assert!(moves.contains(&1)); // Straw
        assert!(!moves.contains(&2)); // Bricks - can't play
    }

    // Can play any card if can't follow suit
    #[test]
    fn test_get_moves_cant_follow_suit() {
        let hand = vec![
            card_with_id(0, 25, Suit::Bricks),
            card_with_id(1, 26, Suit::Bricks),
            card_with_id(2, 15, Suit::Wolf),
        ];

        // Player 0 led with Straw
        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        // No Straw in hand, can play anything
        assert!(moves.contains(&0));
        assert!(moves.contains(&1));
        assert!(moves.contains(&2));
    }

    // Huff cards are playable when not already played
    #[test]
    fn test_get_moves_huff_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card
        assert!(moves.contains(&1)); // huff card
    }

    // Huff cards not playable when already played this trick
    #[test]
    fn test_get_moves_huff_already_played() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
        ];

        let mut trick_huff = no_modifiers();
        trick_huff[0] = Some(Card {
            id: 99,
            value: 1,
            suit: Suit::Huff,
        });

        let game = game_with_hand(0, 0, hand, no_modifiers(), trick_huff, no_modifiers(), true);
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card still playable
        assert!(!moves.contains(&1)); // huff card NOT playable
    }

    // Puff cards are playable when not already played
    #[test]
    fn test_get_moves_puff_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card
        assert!(moves.contains(&1)); // puff card
    }

    // Puff cards not playable when already played this trick
    #[test]
    fn test_get_moves_puff_already_played() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 3, Suit::Puff),
        ];

        let mut trick_puff = no_modifiers();
        trick_puff[0] = Some(Card {
            id: 99,
            value: 2,
            suit: Suit::Puff,
        });

        let game = game_with_hand(0, 0, hand, no_modifiers(), no_modifiers(), trick_puff, true);
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular card still playable
        assert!(!moves.contains(&1)); // puff card NOT playable
    }

    // Both huff and puff playable
    #[test]
    fn test_get_moves_huff_and_puff_both_playable() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 2, Suit::Huff),
            card_with_id(2, 3, Suit::Puff),
        ];

        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // regular
        assert!(moves.contains(&1)); // huff
        assert!(moves.contains(&2)); // puff
    }

    // Following player with huff/puff in hand, must follow suit for regular
    #[test]
    fn test_get_moves_follow_suit_with_huff_puff() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 25, Suit::Bricks),
            card_with_id(2, 2, Suit::Huff),
            card_with_id(3, 3, Suit::Puff),
        ];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Straw,
        });

        let game = game_with_hand(
            1,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - must follow
        assert!(!moves.contains(&1)); // Bricks - can't play
        assert!(moves.contains(&2)); // Huff - always playable if not used
        assert!(moves.contains(&3)); // Puff - always playable if not used
    }

    // Player 2 following, player 0 led
    #[test]
    fn test_get_moves_different_lead_and_current_player() {
        let hand = vec![
            card_with_id(0, 5, Suit::Sticks),
            card_with_id(1, 25, Suit::Bricks),
        ];

        let mut trick_regular = no_modifiers();
        trick_regular[0] = Some(Card {
            id: 99,
            value: 3,
            suit: Suit::Sticks,
        });
        trick_regular[1] = Some(Card {
            id: 98,
            value: 7,
            suit: Suit::Sticks,
        });

        let game = game_with_hand(
            2,
            0,
            hand,
            trick_regular,
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Sticks - follows suit
        assert!(!moves.contains(&1)); // Bricks - can't play
    }

    // Cannot lead wolf when wolf not broken
    #[test]
    fn test_get_moves_cannot_lead_wolf_when_not_broken() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 15, Suit::Wolf),
        ];

        // Leading (no cards played), wolf not broken
        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            false,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - can lead
        assert!(!moves.contains(&1)); // Wolf - cannot lead when not broken
    }

    // Can lead wolf when wolf is broken
    #[test]
    fn test_get_moves_can_lead_wolf_when_broken() {
        let hand = vec![
            card_with_id(0, 5, Suit::Straw),
            card_with_id(1, 15, Suit::Wolf),
        ];

        // Leading (no cards played), wolf IS broken
        let game = game_with_hand(
            0,
            0,
            hand,
            no_modifiers(),
            no_modifiers(),
            no_modifiers(),
            true,
        );
        let moves = game.get_moves();

        assert!(moves.contains(&0)); // Straw - can lead
        assert!(moves.contains(&1)); // Wolf - can lead when broken
    }
}
