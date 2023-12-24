use enum_iterator::{all, Sequence};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::cmp::{min, Ordering};
use std::collections::{HashMap, HashSet};

const DRAW: i32 = 0;
const PASS: i32 = 1;
const DISCARD_OFFSET: i32 = 2; // 2-50 discards
const PLAY_OFFSET: i32 = 51; // 51-99 plays

#[derive(Debug, Clone, Copy, Default, PartialEq, Sequence, Serialize, Deserialize)]
enum State {
    #[default]
    Play,
    Discard,
    OptionalDraw,
}

#[derive(
    Debug,
    PartialOrd,
    Ord,
    Clone,
    Copy,
    Sequence,
    Default,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
)]
pub enum Suit {
    #[default]
    Red,
    Blue,
    Yellow,
    Green,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Card {
    id: i32,
    value: i32,
    suit: Suit,
}

fn move_offset(state: State, card: &Card) -> i32 {
    match state {
        State::OptionalDraw => 0,
        State::Discard => card.id + DISCARD_OFFSET,
        State::Play => card.id + PLAY_OFFSET,
    }
}

fn card_offset(state: State, offset: i32) -> i32 {
    match state {
        State::OptionalDraw => panic!("impossible move"),
        State::Discard => offset - DISCARD_OFFSET,
        State::Play => offset - PLAY_OFFSET,
    }
}

pub fn deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    let mut id = 0;
    for suit in all::<Suit>() {
        for value in 0..12 {
            deck.push(Card {
                id: id,
                value: value + 1,
                suit: suit,
            });
            id += 1;
        }
    }
    deck.shuffle(&mut thread_rng());
    return deck;
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
enum ChangeType {
    #[default]
    Deal,
    Discard,
    Play,
    TricksToWinner,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    TrickToShortsPile,
    Reorder,
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    TricksTaken,
    DrawDeck,
    Score,
    ShortsPile,
    ReorderHand,
    StageDrawDeck,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Change {
    change_type: ChangeType,
    player: i32,
    object_id: i32,
    source_offset: i32,
    dest: Location,
    dest_offset: i32,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    hand_offset: i32,
    length: i32,
    cards_remaining: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Game {
    undo_players: HashSet<i32>,
    action_size: i32,
    hands: Vec<Vec<Card>>,
    draw_decks: Vec<Vec<Card>>,
    shorts_piles: Vec<Vec<Card>>,
    changes: Vec<Vec<Change>>,
    tricks_taken: Vec<i32>,
    current_trick: Vec<Option<Card>>,
    lead_suit: Option<Suit>,
    round: i32,
    scores: Vec<i32>,
    voids: Vec<HashSet<Suit>>,
    current_player: i32,
    winner: Option<i32>,
    dealer: i32,
    state: State,
    undo_plyaers: HashSet<i32>,
    draw_players_remaining: Vec<i32>,
    lead_player: i32,
}

impl Game {
    pub fn new() -> Game {
        let game = Game::default();
        let mut game = game.deal();
        game.changes.push(show_playable(&game));
        game
    }

    fn deal(self: Game) -> Self {
        let mut new_game = self.clone();
        new_game.state = State::Discard;
        new_game.current_trick = vec![None, None, None, None];
        new_game.draw_players_remaining = (0..3).collect();
        new_game.tricks_taken = vec![0, 0, 0];
        new_game.hands = vec![vec![], vec![], vec![]];
        new_game.draw_decks = vec![vec![], vec![], vec![]];
        new_game.shorts_piles = vec![vec![], vec![], vec![]];
        new_game.dealer = (new_game.dealer + 1) % 3;
        new_game.current_player = new_game.dealer;
        new_game.voids = vec![HashSet::new(), HashSet::new(), HashSet::new()];
        let mut cards = deck();
        let deal_index: usize = new_game.changes.len();
        let reorder_index = deal_index + 1;
        new_game.changes.push(vec![]); // deal_index
        new_game.changes.push(vec![]); // reorder_index
        new_game.hands = vec![vec![], vec![], vec![]];
        for y in 0..16 {
            for player in 0..3 {
                let card = cards.pop().expect("cards should be available here");
                new_game.changes[deal_index].push(Change {
                    change_type: ChangeType::Deal,
                    object_id: card.id,
                    dest: Location::Hand,
                    dest_offset: player,
                    player,
                    hand_offset: y,
                    length: 16,
                    ..Default::default()
                });
                new_game.hands[player as usize].push(card);
            }
        }
        new_game.hands[0].sort_by(card_sorter);
        new_game.changes[reorder_index].append(&mut reorder_hand(0, &new_game.hands[0]));
        new_game
    }

    pub fn clone_and_apply_move(self: Game, action: i32) -> Self {
        let mut new_game: Game = self.clone();
        new_game.changes = vec![vec![]]; // card from player to table or discard to draw deck
        let mut current_hand = new_game.hands[new_game.current_player as usize].to_owned();
        if new_game.state == State::OptionalDraw {
            if action == DRAW {
                // Once a player draws a card we don't know what their voids are
                new_game.voids[new_game.current_player as usize] = HashSet::new();
                let new_card: Card = new_game.draw_decks[new_game.current_player as usize]
                    .pop()
                    .expect("there has to be a card to draw");
                new_game.hands[new_game.current_player as usize].push(new_card);
                new_game.hands[new_game.current_player as usize].sort_by(card_sorter);
                new_game.changes[0]
                    .append(reorder_hand(new_game.current_player, &current_hand.to_vec()).as_mut());
                for card in &new_game.draw_decks[new_game.current_player as usize] {
                    new_game.changes[0].push(Change {
                        change_type: ChangeType::Discard,
                        object_id: card.id,
                        source_offset: new_game.current_player,
                        dest: Location::DrawDeck,
                        player: new_game.current_player,
                        cards_remaining: new_game.draw_decks[new_game.current_player as usize].len()
                            as i32,
                        ..Default::default()
                    });
                }
            }
            let player_index = new_game
                .draw_players_remaining
                .iter()
                .position(|x| x == &new_game.current_player)
                .expect("Player who just discarded must be in draw_players_remaining");
            new_game.draw_players_remaining.remove(player_index);
            if new_game.draw_players_remaining.is_empty() {
                if let Some(finished_game) = check_hand_end(&new_game) {
                    return finished_game;
                }
                new_game.current_player = new_game.lead_player;
                new_game.state = State::Play;
            } else {
                new_game.current_player = *new_game
                    .draw_players_remaining
                    .first()
                    .expect("draw_players_remaining cannot be empty here");
            }
            show_playable(&new_game);
            return new_game;
        }
        if new_game.state == State::Discard {
            let mut all_cards = current_hand.clone();
            all_cards.append(&mut new_game.draw_decks[new_game.current_player as usize].clone());
            let card_id = action - DISCARD_OFFSET;
            let card_index: usize =
                all_cards
                    .iter()
                    .position(|c| c.id == card_id)
                    .expect(&format!(
                        "discarding card id {} which is not but should be in draw deck or hand",
                        card_id
                    ));
            let card = all_cards[card_index];
            if new_game.draw_decks[new_game.current_player as usize].contains(&card) {
                // Allows undo
                new_game.draw_decks[new_game.current_player as usize].remove(card_index);
                current_hand.push(card);
            } else {
                current_hand.remove(card_index);

                new_game.draw_decks[new_game.current_player as usize].push(card);
            }
            let mut offset: i32 = 0;
            if new_game.current_player == 0 {
                for card in &new_game.draw_decks[new_game.current_player as usize] {
                    new_game.changes[0].push(Change {
                        change_type: ChangeType::Discard,
                        object_id: card.id,
                        source_offset: new_game.current_player,
                        dest: Location::StageDrawDeck,
                        dest_offset: offset,
                        player: new_game.current_player,
                        cards_remaining: new_game.draw_decks[new_game.current_player as usize].len()
                            as i32,
                        ..Default::default()
                    });
                    offset += 1;
                }
            } else {
                new_game.changes[0].push(Change {
                    change_type: ChangeType::Discard,
                    object_id: card.id,
                    source_offset: new_game.current_player,
                    dest: Location::DrawDeck,
                    dest_offset: offset,
                    player: new_game.current_player,
                    cards_remaining: new_game.draw_decks[new_game.current_player as usize].len()
                        as i32,
                    ..Default::default()
                });
            }
            new_game.hands[new_game.current_player as usize].sort_by(card_sorter);
            new_game.changes[0].append(
                reorder_hand(
                    new_game.current_player,
                    &new_game.hands[new_game.current_player as usize],
                )
                .as_mut(),
            );
            if new_game.draw_decks[new_game.current_player as usize].len() == 5 {
                if new_game.current_player == 0 {
                    let mut cards_remaining_changes: Vec<Change> = vec![];
                    for card in &new_game.draw_decks[0] {
                        cards_remaining_changes.push(Change {
                            object_id: card.id,
                            change_type: ChangeType::Discard,
                            dest: Location::DrawDeck,
                            cards_remaining: 5,
                            ..Default::default()
                        });
                    }
                    new_game.changes.push(cards_remaining_changes);
                }
                new_game.current_player = (new_game.current_player + 1) % 3;
            }
            if new_game.draw_decks[new_game.current_player as usize].len() == 5 {
                for player in 0..3 {
                    new_game.draw_decks[player].shuffle(&mut thread_rng());
                }
                new_game.state = State::OptionalDraw;
            }
            show_playable(&new_game);
            return new_game;
        }
        let card_id = action - PLAY_OFFSET;
        let card_index: usize = current_hand
            .iter()
            .position(|c| c.id == card_id)
            .expect(&format!(
                "playing card id {} which is not but should be in hand",
                card_id
            ));
        let card = current_hand[card_index];
        current_hand.remove(card_index);
        new_game.changes[0].push(Change {
            change_type: ChangeType::Play,
            object_id: card_id,
            source_offset: new_game.current_player,
            dest: Location::Play,
            dest_offset: new_game.current_player,
            player: new_game.current_player,
            ..Default::default()
        });
        new_game.changes[0].append(
            reorder_hand(
                new_game.current_player,
                &new_game.hands[new_game.current_player as usize],
            )
            .as_mut(),
        );
        hide_playable(&new_game);
        new_game.current_trick[new_game.current_player as usize] = Some(card);
        if let Some(suit) = new_game.lead_suit {
            // Player has revealed a void
            new_game.voids[new_game.current_player as usize].insert(suit);
        }
        if None == new_game.lead_suit {
            new_game.lead_suit = Some(card.suit);
        }
        new_game.current_player = (new_game.current_player + 1) % 3;
        // end trick
        if new_game.current_trick.iter().flatten().count() == 3 {
            let trick_winner = get_winner(new_game.lead_suit, &new_game.current_trick);
            let winning_card = new_game.current_trick[trick_winner as usize]
                .expect("there has to be a trick_winner card");
            new_game.tricks_taken[trick_winner as usize] =
                new_game.tricks_taken[trick_winner as usize] + 1;
            // winner of the trick leads
            new_game.current_player = trick_winner;
            new_game.lead_player = trick_winner;
            new_game.changes.push(vec![
                Change {
                    change_type: ChangeType::ShowWinningCard,
                    object_id: winning_card.id,
                    dest: Location::Play,
                    ..Default::default()
                },
                Change {
                    change_type: ChangeType::OptionalPause,
                    object_id: 0,
                    dest: Location::Play,
                    ..Default::default()
                },
                Change {
                    object_id: winning_card.id,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    dest_offset: new_game.current_player,
                    ..Default::default()
                },
            ]);
            new_game.changes.push(vec![]); // trick back to player
            let offset: usize = new_game.changes.len() - 1;
            for player in 0..3 {
                let card =
                    new_game.current_trick[player].expect("each player should have played a card");
                if Some(card.suit) == new_game.lead_suit {
                    new_game.changes[offset].push(Change {
                        change_type: ChangeType::TricksToWinner,
                        object_id: card.id,
                        source_offset: player as i32,
                        dest: Location::TricksTaken,
                        player: trick_winner,
                        tricks_taken: new_game.tricks_taken[trick_winner as usize] as i32,
                        ..Default::default()
                    });
                } else {
                    new_game.shorts_piles[player].push(card);
                    new_game.changes[offset].push(Change {
                        change_type: ChangeType::TrickToShortsPile,
                        object_id: card.id,
                        source_offset: player as i32,
                        dest: Location::ShortsPile,
                        player: player as i32,
                        dest_offset: trick_winner,
                        tricks_taken: new_game.shorts_piles[player].len() as i32,
                        ..Default::default()
                    });
                }
            }
            new_game.draw_players_remaining = vec![];
            for player_offset in 0..3 {
                let player = (player_offset + new_game.lead_player) % 3;
                if !new_game.draw_decks[player as usize].is_empty() {
                    new_game.draw_players_remaining.push(player);
                }
            }
            if !new_game.draw_players_remaining.is_empty() {
                new_game.current_player = *new_game
                    .draw_players_remaining
                    .first()
                    .expect("draw_players_remaining unexpectedly empty");
                new_game.state = State::OptionalDraw;
            } else {
                if let Some(finished_game) = check_hand_end(&new_game) {
                    return finished_game;
                }

                new_game.current_player = new_game.lead_player;
                new_game.state = State::Play;
            }
            new_game.current_trick = vec![None, None, None, None];
            new_game.lead_suit = None;
        }
        new_game.changes.push(show_playable(&new_game));
        return new_game;
    }

    fn get_moves(self: &Game) -> Vec<i32> {
        if self.state == State::OptionalDraw {
            if !self.draw_decks[self.current_player as usize].is_empty() {
                return vec![DRAW, PASS];
            }
            return vec![PASS];
        }
        let allow_undoes = self.undo_players.contains(&self.current_player);
        let mut actions: Vec<i32>;
        if self.state == State::Discard {
            actions = self.hands[self.current_player as usize]
                .iter()
                .map(|c| move_offset(self.state, c))
                .collect();
            if allow_undoes {
                actions.append(
                    &mut self.draw_decks[self.current_player as usize]
                        .iter()
                        .map(|c| move_offset(self.state, c))
                        .collect(),
                );
            }
            return actions;
        }
        if self.lead_suit != None {
            actions = self.hands[self.current_player as usize]
                .iter()
                .filter(|c| Some(c.suit) == self.lead_suit)
                .map(|c| move_offset(self.state, c))
                .collect();
            if !actions.is_empty() {
                return actions;
            }
        }
        return self.hands[self.current_player as usize]
            .iter()
            .map(|c| move_offset(self.state, c))
            .collect();
    }
}

fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

pub fn get_winner(lead_suit: Option<Suit>, trick: &Vec<Option<Card>>) -> i32 {
    let mut card_id_to_player: HashMap<i32, i32> = HashMap::new();
    for player in 0..3 {
        let card = trick[player].expect("each player must have played to the trick");
        card_id_to_player.insert(card.id, player as i32);
    }
    let mut cards: Vec<Card> = trick
        .iter() // Convert the Vec into an Iterator
        .filter_map(|&x| x) // filter_map will only pass through the Some values
        .collect();
    cards.sort_by(|a, b| value_for_card(lead_suit, b).cmp(&value_for_card(lead_suit, a)));
    return *card_id_to_player
        .get(&cards.first().expect("there should be a winning card").id)
        .expect("cards_to_player missing card");
}

pub fn value_for_card(lead_suit: Option<Suit>, card: &Card) -> i32 {
    let mut lead_bonus: i32 = 0;
    if Some(card.suit) == lead_suit {
        lead_bonus += 100;
    }
    return card.value + lead_bonus;
}

fn check_hand_end(new_game: &Game) -> Option<Game> {
    if !new_game.hands.iter().any(|x| x.is_empty()) {
        return None;
    }

    let mut new_game = new_game.clone();

    let original_scores: Vec<i32> = new_game.scores.clone();

    hide_playable(&new_game);
    new_game.scores = score_game(
        new_game.scores,
        &new_game.tricks_taken,
        new_game
            .shorts_piles
            .iter()
            .map(|sp| sp.len() as i32)
            .collect(),
    );
    let mut max_score = 0;
    for player in 0..3 {
        if new_game.scores[player] > max_score {
            max_score = new_game.scores[player];
        }
    }
    for player in 0..3 {
        new_game.changes.push(vec![Change {
            change_type: ChangeType::Score,
            object_id: player,
            player,
            dest: Location::Score,
            start_score: original_scores[player as usize],
            end_score: new_game.scores[player as usize],
            ..Default::default()
        }]);
    }
    let mut high_score: i32 = 0;
    let mut winners: Vec<i32> = vec![];
    for player in 0..3 {
        let score = new_game.scores[player];
        if score > high_score {
            high_score = score;
        }
    }
    for player in 0..3 {
        let score = new_game.scores[player];
        if score == high_score {
            winners.push(player as i32);
        }
    }
    if new_game.round >= 3 && winners.len() == 1 {
        new_game.winner = Some(winners[0]);
        new_game.changes.push(vec![Change {
            change_type: ChangeType::GameOver,
            dest: Location::Deck,
            ..Default::default()
        }]);
        return Some(new_game);
    } else {
        new_game.round += 1;
        new_game.changes.push(vec![Change {
            change_type: ChangeType::Shuffle,
            object_id: 0,
            source_offset: 0,
            dest: Location::Deck,
            dest_offset: 0,
            ..Default::default()
        }]);
        new_game = new_game.deal();
    }
    return Some(new_game);
}

pub fn score_game(
    original_scores: Vec<i32>,
    tricks_taken: &Vec<i32>,
    shorts_pile_lengths: Vec<i32>,
) -> Vec<i32> {
    let mut scores = original_scores.clone();
    for player in 0..3 {
        scores[player] = scores[player] + tricks_taken[player];
        let mut score_per_match = 3;
        if shorts_pile_lengths[player] == tricks_taken[player] {
            score_per_match = 5;
        }
        let match_count = min(shorts_pile_lengths[player], tricks_taken[player]);
        scores[player] = scores[player] + (match_count * score_per_match);
    }
    return scores;
}

pub fn reorder_hand(player: i32, hand: &Vec<Card>) -> Vec<Change> {
    let mut changes: Vec<Change> = vec![];
    for (offset_in_hand, card) in hand.iter().enumerate() {
        changes.push(Change {
            object_id: card.id,
            player,
            dest: Location::ReorderHand,
            change_type: ChangeType::Reorder,
            hand_offset: offset_in_hand as i32,
            length: hand.len() as i32,
            ..Default::default()
        });
    }
    return changes;
}

fn show_playable(new_game: &Game) -> Vec<Change> {
    let mut changes: Vec<Change> = vec![];

    if new_game.current_player == 0 {
        if new_game.state == State::OptionalDraw {
            changes.push(Change {
                object_id: -1,
                change_type: ChangeType::ShowPlayable,
                dest: Location::Hand,
                dest_offset: new_game.current_player,
                ..Default::default()
            });
            changes.push(Change {
                object_id: -2,
                change_type: ChangeType::ShowPlayable,
                dest: Location::Hand,
                dest_offset: new_game.current_player,
                ..Default::default()
            });
        } else {
            changes.push(Change {
                object_id: -1,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                dest_offset: new_game.current_player,
                ..Default::default()
            });
            changes.push(Change {
                object_id: -2,
                change_type: ChangeType::HidePlayable,
                dest: Location::Hand,
                dest_offset: new_game.current_player,
                ..Default::default()
            });
            for action in new_game.get_moves() {
                changes.push(Change {
                    object_id: card_offset(new_game.state, action),
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::Hand,
                    dest_offset: new_game.current_player,
                    ..Default::default()
                });
            }
        }
        return changes;
    } else {
        return hide_playable(&new_game);
    }
}

fn hide_playable(new_game: &Game) -> Vec<Change> {
    let mut changes: Vec<Change> = vec![];
    for card in &new_game.hands[0] {
        changes.push(Change {
            object_id: card.id,
            change_type: ChangeType::HidePlayable,
            dest: Location::Hand,
            dest_offset: new_game.current_player,
            ..Default::default()
        });
    }
    changes.push(Change {
        object_id: -1,
        change_type: ChangeType::HidePlayable,
        dest: Location::Hand,
        dest_offset: new_game.current_player,
        ..Default::default()
    });
    changes.push(Change {
        object_id: -2,
        change_type: ChangeType::HidePlayable,
        dest: Location::Hand,
        dest_offset: new_game.current_player,
        ..Default::default()
    });
    return changes;
}
