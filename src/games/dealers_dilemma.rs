/*
Game: Dealer's Dilemma
Designer: Shreesh Bhat
BoardGameGeek: https://boardgamegeek.com/boardgame/378945/dealers-dilemma
*/

use colored::Colorize;
use enum_iterator::{all, Sequence};
use ismcts::IsmctsHandler;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::cmp::{min, Ordering};
use std::collections::{HashMap, HashSet};
use std::mem;

use crate::utils::shuffle_and_divide_matching_cards;

/// Play offsets (each possible action has a unique ID)
// 0-35 - 36 cards 2 3 4 5 6 7 8 9 10 in 4 suits (for playing)
pub const DEALER_SELECT_CARD: i32 = 36; // 36 - left card, 37 - right card (trump selection)
pub const DEALER_SELECT_CARD_NO_TRUMP: i32 = 38; // 38 - left card (no trump), 39 - right card (no trump)
pub const BID_CARD_OFFSET: i32 = 40; // 40-76 cards 2 3 4 5 6 7 8 9 10 in 4 suits (for bidding)
pub const BID_TYPE_OFFSET: i32 = 77; // 77-80 Easy, Top, Difference, Zero
pub const BID_TYPE_EASY: i32 = 77;
pub const BID_TYPE_TOP: i32 = 78;
pub const BID_TYPE_DIFFERENCE: i32 = 79;
pub const BID_TYPE_ZERO: i32 = 80;

fn color_suit(suit: Option<Suit>, string: String) -> String {
    if !cfg!(windows) {
        return match suit {
            Some(Suit::Red) => string.red().to_string(),
            Some(Suit::Blue) => string.blue().to_string(),
            Some(Suit::Yellow) => string.yellow().to_string(),
            Some(Suit::Green) => string.green().to_string(),
            _ => string,
        };
    } else {
        string
    }
}

pub fn print_suit(suit: Option<Suit>) -> String {
    let string = if let Some(suit) = suit {
        match suit {
            Suit::Red => "♥".to_string(),
            Suit::Blue => "♣".to_string(),
            Suit::Yellow => "♦".to_string(),
            Suit::Green => "♠".to_string(),
        }
    } else {
        "?".to_string()
    };
    color_suit(suit, string)
}

pub fn print_card(card: Card, prefix_id: bool) -> String {
    let string = format!(
        "{}{}",
        color_suit(Some(card.suit), card.value.to_string()),
        print_suit(Some(card.suit))
    );
    if !prefix_id {
        return string;
    }
    return format!("{}: {}", card.id, string);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Sequence, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Play, // trick taking, must follow
    BidType,      // the type of bid the player is selecting
    BidCard, // each player bids by putting 2 cards from their hand onto the table in front of them
    DealerSelect, // the Dealer picks one of the cards into their hand
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BidType {
    #[default]
    Easy,
    Top,
    Difference,
    Zero,
}

impl BidType {
    fn score_for_tricks(&self, bid_cards: [Option<Card>; 2], tricks: i32) -> i32 {
        match self {
            BidType::Easy => {
                let faceup_card = bid_cards[0];
                let facedown_card = bid_cards[1];
                let lowest_bid = min(facedown_card.unwrap().value, faceup_card.unwrap().value);
                match tricks {
                    // tricks won is equal to the revealed card: score 4 points
                    _ if tricks == faceup_card.unwrap().value => 4,
                    // tricks won is equal to the hidden card: score 2 points
                    _ if tricks == facedown_card.unwrap().value => 2,
                    // -1 point per trick missed from your lowest bid value
                    _ => (lowest_bid - tricks).abs() * -1,
                }
            }
            BidType::Top => {
                let faceup_card = bid_cards[0];
                match tricks {
                    // tricks won is equal to your bid: score 8 points
                    _ if tricks == faceup_card.unwrap().value => 8,
                    // -2 points per trick missed from your bid value.
                    _ => (tricks - faceup_card.unwrap().value).abs() * -2,
                }
            }
            BidType::Difference => {
                let faceup_card = bid_cards[0];
                let sideways_card = bid_cards[1];
                let bid = (faceup_card.unwrap().value - sideways_card.unwrap().value).abs();
                match tricks {
                    // tricks won is equal to your bid: score 8 points.
                    _ if tricks == bid => 8,
                    // -2 points per trick missed from your bid value
                    _ => (tricks - bid).abs() * -2,
                }
            }
            BidType::Zero => match tricks {
                _ if tricks == 0 => 6,
                _ => tricks * -2,
            },
        }
    }
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
#[serde(rename_all = "camelCase")]
pub enum Suit {
    #[default]
    Red,
    Blue,
    Yellow,
    Green,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i32,
    pub value: i32,
    pub suit: Suit,
}

pub fn move_offset(state: State, card: &Card) -> i32 {
    match state {
        State::Play => card.id,
        State::BidCard => card.id + BID_CARD_OFFSET,
        State::DealerSelect => DEALER_SELECT_CARD,
        State::BidType => unreachable!(),
    }
}
fn card_offset(state: State, offset: i32) -> i32 {
    match state {
        State::Play => offset,
        State::BidCard => offset - BID_CARD_OFFSET,
        State::DealerSelect => offset - DEALER_SELECT_CARD,
        State::BidType => unreachable!(),
    }
}

fn bid_type_offset(bid_id: i32) -> BidType {
    match bid_id {
        BID_TYPE_EASY => BidType::Easy,
        BID_TYPE_TOP => BidType::Top,
        BID_TYPE_ZERO => BidType::Zero,
        BID_TYPE_DIFFERENCE => BidType::Difference,
        _ => unreachable!(),
    }
}

pub fn deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    let mut id = 0;
    for suit in all::<Suit>() {
        for value in 1..10 {
            deck.push(Card {
                id,
                value: value + 1,
                suit,
            });
            id += 1;
        }
    }
    deck.shuffle(&mut thread_rng());
    deck
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    GameOver,
    RemainingCards,
    Reorder,
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Location {
    #[default]
    Deck,
    Hand,
    Play,
    Bid, // each player bids by putting 2 cards from their hand onto the table in front of them
    RemainingCards, // the Dealer takes the remaining 2 cards and places them face up for everyone to see.
    TricksTaken,
    Score,
    ReorderHand,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    action_size: i32,
    pub hands: [Vec<Card>; 3],
    pub changes: Vec<Vec<Change>>,
    pub human_player: [bool; 3],
    pub tricks_taken: [i32; 3],
    pub bids: [Option<BidType>; 3],
    pub bid_cards: [[Option<Card>; 2]; 3],
    pub current_trick: [Option<Card>; 3],
    pub dealer_select: Vec<Card>,
    pub lead_suit: Option<Suit>,
    pub trump_suit: Option<Suit>,
    pub round: i32,
    pub scores_this_hand: [i32; 3],
    pub scores: [i32; 3],
    pub voids: [HashSet<Suit>; 3],
    pub current_player: i32,
    pub winner: Option<i32>,
    pub dealer: i32,
    pub state: State,
    lead_player: i32,
    #[serde(default)]
    pub no_changes: bool,
}

impl Game {
    /// Factory to create a default game
    pub fn new() -> Game {
        let mut game = Game::default();
        game.dealer = 2;
        game.current_player = 2;
        let mut game = game.deal();
        game.scores = [0, 0, 0];
        game.scores_this_hand = [0, 0, 0];
        game.changes.push(show_playable(&game));
        game
    }
    // Skip adding changes which are used to manipulate the UI
    // This is used to increase the speed of simulations
    pub fn with_no_changes(self: &mut Game) {
        self.no_changes = true;
    }

    fn deal(self: Game) -> Self {
        let mut new_game = self.clone();
        new_game.state = State::DealerSelect;
        new_game.round = self.round + 1;
        new_game.bids = [None, None, None];
        new_game.bid_cards = [[None, None], [None, None], [None, None]];
        new_game.trump_suit = None;
        new_game.lead_suit = None;
        new_game.current_trick = [None, None, None];
        new_game.tricks_taken = [0, 0, 0];
        new_game.hands = [vec![], vec![], vec![]];
        new_game.dealer = (new_game.dealer + 1) % 3;
        new_game.current_player = new_game.dealer;
        new_game.voids = [HashSet::new(), HashSet::new(), HashSet::new()];
        let mut cards = deck();
        let deal_index: usize = new_game.changes.len();
        let reorder_index = deal_index + 1;
        new_game.changes.push(vec![]); // deal_index
        new_game.changes.push(vec![]); // reorder_index
        new_game.hands = [vec![], vec![], vec![]];
        new_game.dealer_select = vec![];

        for y in 0..12 {
            for player in 0..3 {
                let card = cards.pop().expect("cards should be available here");
                if player == new_game.dealer && (y == 10 || y == 11) {
                    new_game.dealer_select.push(card);
                    new_game.changes[deal_index].push(Change {
                        change_type: ChangeType::RemainingCards,
                        object_id: card.id,
                        dest: Location::RemainingCards,
                        dest_offset: y,
                        player,
                        hand_offset: y - 10, // 0 for left card 1 for right card
                        length: 2,
                        ..Default::default()
                    });
                } else {
                    new_game.changes[deal_index].push(Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        dest_offset: player,
                        player,
                        hand_offset: y,
                        length: if player == self.dealer { 10 } else { 12 },
                        ..Default::default()
                    });
                    new_game.hands[player as usize].push(card);
                }
            }
        }

        new_game.hands[0].sort_by(card_sorter);
        new_game.changes[reorder_index].append(&mut reorder_hand(0, &new_game.hands[0]));
        new_game
    }

    pub fn clone_and_apply_move(self: Game, action: i32) -> Self {
        let mut new_game: Game = self.clone();

        // reset only after a move is made in the next round
        new_game.scores_this_hand = [0, 0, 0];

        // card from player to table or discard to draw deck
        new_game.changes = vec![vec![]];

        match new_game.state {
            State::BidType => {
                new_game.bids[new_game.current_player as usize] = Some(bid_type_offset(action));
                new_game.state = State::BidCard;
                new_game
            }
            State::DealerSelect => {
                let card_to_hand: Card;
                let card_to_play: Card;
                if action == DEALER_SELECT_CARD || action == DEALER_SELECT_CARD_NO_TRUMP {
                    card_to_hand = new_game.dealer_select[0];
                    card_to_play = new_game.dealer_select[1];
                } else {
                    card_to_hand = new_game.dealer_select[1];
                    card_to_play = new_game.dealer_select[0];
                }

                if action == DEALER_SELECT_CARD || action == DEALER_SELECT_CARD + 1 {
                    // add a dynamic element showing which trump was selected (1/4 size card in top left?)
                    new_game.trump_suit = Some(card_to_hand.suit);
                }

                // TODO: move card selected to play area as lead card

                // TODO: animation to move card selected as trump to dealer's hand
                new_game.hands[new_game.current_player as usize].push(card_to_hand);
                new_game.current_trick[new_game.current_player as usize] = Some(card_to_play);
                new_game.lead_suit = Some(card_to_play.suit);
                new_game.state = State::BidType;

                new_game
            }
            State::BidCard => {
                let card_id = card_offset(new_game.state, action);
                let card = &new_game.hands[new_game.current_player as usize]
                    .iter()
                    .find(|c| c.id == card_id)
                    .expect("this card has to be in the player's hand")
                    .clone();
                new_game.hands[new_game.current_player as usize].retain(|c| c.id != card_id);
                if new_game.bid_cards[new_game.current_player as usize][0] == None {
                    new_game.bid_cards[new_game.current_player as usize][0] = Some(*card);
                } else if new_game.bid_cards[new_game.current_player as usize][1] == None {
                    new_game.bid_cards[new_game.current_player as usize][1] = Some(*card);
                    new_game.current_player = (new_game.current_player + 1) % 3;
                    new_game.state = State::BidType;
                    if new_game.bid_cards[new_game.current_player as usize][1] != None {
                        // next player to bid has already bid
                        // first player to bid is always dealer and
                        // they were forced to play the card they didn't select
                        new_game.current_player = (new_game.current_player + 1) % 3;
                        new_game.state = State::Play;
                    }
                } else {
                    panic!("player has already bid two cards!")
                }

                // TODO: send bid card to table animation
                new_game
            }
            State::Play => {
                let card_id = card_offset(new_game.state, action);
                let card = &new_game.hands[new_game.current_player as usize]
                    .iter()
                    .find(|c| c.id == card_id)
                    .expect("this card has to be in the player's hand")
                    .clone();
                new_game.hands[new_game.current_player as usize].retain(|c| c.id != card_id);
                if !self.no_changes {
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
                }
                let last_change = new_game.changes.len() - 1;
                let mut changes = hide_playable(&new_game);
                new_game.changes[last_change].append(&mut changes);
                new_game.current_trick[new_game.current_player as usize] = Some(*card);

                if new_game.lead_suit.is_none() {
                    new_game.lead_suit = Some(card.suit);
                } else {
                    if Some(card.suit) != new_game.lead_suit {
                        // Player has revealed a void
                        new_game.voids[new_game.current_player as usize].insert(card.suit);
                    }
                }
                new_game.current_player = (new_game.current_player + 1) % 3;
                // end trick
                if new_game.current_trick.iter().flatten().count() == 3 {
                    if !new_game.no_changes {
                        println!(
                            "trick finished: {}",
                            new_game
                                .current_trick
                                .iter()
                                .flatten()
                                .map(|c| print_card(*c, false))
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                    }
                    let trick_winner = get_winner(
                        new_game.lead_suit,
                        new_game.trump_suit,
                        &new_game.current_trick,
                    );
                    let winning_card = new_game.current_trick[trick_winner as usize]
                        .expect("there has to be a trick_winner card");
                    new_game.tricks_taken[trick_winner as usize] += 1;
                    // winner of the trick leads
                    new_game.current_player = trick_winner;
                    new_game.lead_player = trick_winner;
                    if !self.no_changes {
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
                    }
                    new_game.changes.push(vec![]); // trick back to player
                    let offset: usize = new_game.changes.len() - 1;
                    for player in 0..3 {
                        let card = new_game.current_trick[player]
                            .expect("each player should have played a card");
                        if Some(card.suit) == new_game.lead_suit {
                            new_game.changes[offset].push(Change {
                                change_type: ChangeType::TricksToWinner,
                                object_id: card.id,
                                source_offset: player as i32,
                                dest: Location::TricksTaken,
                                player: trick_winner,
                                tricks_taken: new_game.tricks_taken[trick_winner as usize],
                                ..Default::default()
                            });
                        }
                    }

                    if new_game.hands.iter().all(|h| h.is_empty()) {
                        // hand end
                        for player in 0..3 {
                            let score = new_game.bids[player]
                                .expect("Must have bid here")
                                .score_for_tricks(
                                    new_game.bid_cards[player],
                                    new_game.tricks_taken[player],
                                );
                            //TODO animate score
                            new_game.scores[player] += score;
                            new_game.scores_this_hand[player] += score;
                        }
                        if new_game.round >= 6 {
                            // game end
                            // find winners - if human player is a winner set them as the exclusive winner
                            let max_score: i32 = *new_game.scores.iter().max().unwrap();
                            for player in 0..3 {
                                if new_game.scores[player] == max_score {
                                    new_game.winner = Some(player as i32);
                                    if new_game.human_player[player] {
                                        // if the human player is among the winners - set them as the winner
                                        break;
                                    }
                                }
                            }
                            return new_game;
                        }
                        return new_game.deal();
                    }

                    new_game.current_player = new_game.lead_player;
                    new_game.state = State::Play;

                    new_game.current_trick = [None, None, None];
                    new_game.lead_suit = None;
                }
                let change_offset = &new_game.changes.len() - 1;
                let mut new_changes = show_playable(&new_game);
                new_game.changes[change_offset].append(&mut new_changes);
                new_game
            }
        }
    }

    pub fn get_moves(self: &Game) -> Vec<i32> {
        if self.state == State::BidType {
            return (0..4).map(|x| x + BID_TYPE_OFFSET).collect();
        }
        if self.state == State::BidCard {
            return self.hands[self.current_player as usize]
                .iter()
                .map(|c| move_offset(self.state, c))
                .collect();
        }
        if self.state == State::DealerSelect {
            if self.dealer_select[0].suit == self.dealer_select[1].suit {
                return vec![
                    DEALER_SELECT_CARD,
                    DEALER_SELECT_CARD + 1,
                    DEALER_SELECT_CARD_NO_TRUMP,
                    DEALER_SELECT_CARD_NO_TRUMP + 1,
                ];
            }
            return vec![DEALER_SELECT_CARD, DEALER_SELECT_CARD + 1];
        }
        let actions: Vec<i32>;
        if self.lead_suit.is_some() {
            actions = self.hands[self.current_player as usize]
                .iter()
                .filter(|c| Some(c.suit) == self.lead_suit)
                .map(|c| move_offset(self.state, c))
                .collect();
            if !actions.is_empty() {
                return actions;
            }
        }
        self.hands[self.current_player as usize]
            .iter()
            .map(|c| move_offset(self.state, c))
            .collect()
    }
}
fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

pub fn get_winner(
    lead_suit: Option<Suit>,
    trump_suit: Option<Suit>,
    trick: &[Option<Card>; 3],
) -> i32 {
    let mut card_id_to_player: HashMap<i32, i32> = HashMap::new();
    for (player, card) in trick.iter().enumerate() {
        if let Some(card) = card {
            card_id_to_player.insert(card.id, player as i32);
        }
    }
    let mut cards: Vec<Card> = trick
        .iter() // Convert the Vec into an Iterator
        .filter_map(|&x| x) // filter_map will only pass through the Some values
        .collect();
    cards.sort_by_key(|c| std::cmp::Reverse(value_for_card(lead_suit, trump_suit, c)));
    *card_id_to_player
        .get(&cards.first().expect("there should be a winning card").id)
        .expect("cards_to_player missing card")
}

pub fn value_for_card(lead_suit: Option<Suit>, trump_suit: Option<Suit>, card: &Card) -> i32 {
    let mut bonus: i32 = 0;
    if Some(card.suit) == lead_suit {
        bonus += 100;
    }
    if trump_suit == Some(card.suit) {
        bonus += 200;
    }
    card.value + bonus
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
    changes
}

fn show_playable(new_game: &Game) -> Vec<Change> {
    if new_game.no_changes {
        return vec![];
    }
    let mut changes: Vec<Change> = vec![];

    if new_game.current_player == 0 {
        if new_game.state == State::DealerSelect {
            for card in new_game.dealer_select.clone().into_iter() {
                changes.push(Change {
                    object_id: card.id,
                    change_type: ChangeType::ShowPlayable,
                    dest: Location::Hand,
                    dest_offset: new_game.current_player,
                    ..Default::default()
                });
            }
        } else {
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
        changes
    } else {
        let mut hide_changes = hide_playable(&new_game);
        changes.append(&mut hide_changes);
        changes
    }
}

fn hide_playable(new_game: &Game) -> Vec<Change> {
    if new_game.no_changes {
        return vec![];
    }
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
    changes
}

impl ismcts::Game for Game {
    type Move = i32;
    type PlayerTag = i32;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();

        for p1 in 0..3 {
            for p2 in 0..3 {
                if p1 == self.current_player() || p2 == self.current_player() || p1 == p2 {
                    continue;
                }

                // Add hidden bid cards to player's hands so they can be swapped
                for player in [p1 as usize, p2 as usize] {
                    if self.bids[player] == Some(BidType::Easy) && self.bid_cards[player][1] != None
                    {
                        let bid_card = self.bid_cards[player][1].unwrap();
                        self.hands[player].push(bid_card);
                    }
                }

                let mut combined_voids: HashSet<Suit> =
                    HashSet::from_iter(self.voids[p1 as usize].iter().cloned());
                combined_voids.extend(self.voids[p2 as usize].iter());

                let mut new_hands = vec![
                    self.hands[p1 as usize].clone(),
                    self.hands[p2 as usize].clone(),
                ];

                // allow swapping of any cards that are not in the combined void set
                shuffle_and_divide_matching_cards(
                    |c: &Card| !combined_voids.contains(&c.suit),
                    &mut new_hands,
                    rng,
                );

                /*println!("shared voids: {:?}", combined_voids);
                println!(
                    "original hands: {}\n{}\n",
                    self.hands[p1 as usize]
                        .iter()
                        .map(|c| print_card(*c, false))
                        .collect::<Vec<_>>()
                        .join(" "),
                    self.hands[p2 as usize]
                        .iter()
                        .map(|c| print_card(*c, false))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                println!(
                    "new hands: {}\n{}\n",
                    new_hands[0]
                        .iter()
                        .map(|c| print_card(*c, false))
                        .collect::<Vec<_>>()
                        .join(" "),
                    new_hands[1]
                        .iter()
                        .map(|c| print_card(*c, false))
                        .collect::<Vec<_>>()
                        .join(" "),
                        );*/

                self.hands[p1 as usize] = new_hands[0].clone();
                self.hands[p2 as usize] = new_hands[1].clone();

                for player in [p1 as usize, p2 as usize] {
                    if self.bids[player] == Some(BidType::Easy) && self.bid_cards[player][1] != None
                    {
                        // randomly take one of the cards and make it the hidden card
                        self.bid_cards[player][1] = self.hands[player].pop();
                    }
                }
            }
        }
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.current_player
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.current_player + 1) % 3
    }

    fn available_moves(&self) -> Self::MoveList {
        self.get_moves()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        // FIXME - updating in place would be much faster
        let _ = mem::replace(self, self.clone().clone_and_apply_move(*mov));
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.winner == None {
            None
        } else {
            let mut sorted_scores = self.scores_this_hand.clone();
            sorted_scores.sort();
            sorted_scores.reverse();
            let scorer_count = sorted_scores.iter().filter(|&x| *x > 0).count();
            let high_score = sorted_scores[0];
            let mut score = self.scores_this_hand[player as usize];
            if score <= 0 {
                // Capping the score at -8
                score = min(-8, score);
                let normalized_score = (score.abs() as f64) / 8.0;
                // Normalizing the score to 0 - .2
                Some(0.2 * (1.0 - normalized_score))
            } else {
                // divide by number of > 0 scoring players to incentivize
                // minimizing the number of other winners
                let score = (score as f64 / high_score as f64) / scorer_count as f64;
                Some(0.2 + (0.8 * score))
            }
        }
    }
}

pub fn get_mcts_move(game: &Game, iterations: i32) -> i32 {
    let mut new_game = game.clone();
    new_game.round = 6;
    new_game.no_changes = true;
    let mut ismcts = IsmctsHandler::new(new_game);
    let parallel_threads: usize = 8;
    ismcts.run_iterations(
        parallel_threads,
        (iterations as f64 / parallel_threads as f64) as usize,
    );
    ismcts.best_move().expect("should have a move to make")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck() {
        let d = deck();
        assert_eq!(d.len(), 36);
    }

    #[test]
    fn test_get_winner() {
        assert_eq!(
            get_winner(
                Some(Suit::Blue),
                Some(Suit::Blue),
                &[
                    Some(Card {
                        id: 0,
                        value: 7,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 1,
                        value: 8,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 2,
                        value: 9,
                        suit: Suit::Blue
                    }),
                ]
            ),
            2
        );
        assert_eq!(
            get_winner(
                Some(Suit::Blue),
                Some(Suit::Blue),
                &[
                    Some(Card {
                        id: 0,
                        value: 9,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 1,
                        value: 8,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 2,
                        value: 1,
                        suit: Suit::Red
                    }),
                ]
            ),
            0
        );
        assert_eq!(
            get_winner(
                Some(Suit::Blue),
                Some(Suit::Red),
                &[
                    Some(Card {
                        id: 0,
                        value: 9,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 1,
                        value: 8,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 2,
                        value: 1,
                        suit: Suit::Red
                    }),
                ]
            ),
            2
        );
        assert_eq!(
            get_winner(
                Some(Suit::Blue),
                None,
                &[
                    Some(Card {
                        id: 0,
                        value: 9,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 1,
                        value: 8,
                        suit: Suit::Blue
                    }),
                    Some(Card {
                        id: 2,
                        value: 1,
                        suit: Suit::Red
                    }),
                ]
            ),
            0
        );
    }

    struct BidTestCase {
        bid_type: BidType,
        bid_cards: [Option<Card>; 2],
        tricks: i32,
        expected_score: i32,
    }

    #[test]
    fn test_bid_calculation() {
        let cases = vec![
            // successful top bid
            BidTestCase {
                bid_type: BidType::Top,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 2,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 2,
                expected_score: 8,
            },
            // failed top bid
            BidTestCase {
                bid_type: BidType::Top,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 2,
                expected_score: -2,
            },
            // successful zero bid
            BidTestCase {
                bid_type: BidType::Zero,
                bid_cards: [None, None],
                tricks: 0,
                expected_score: 6,
            },
            // failed zero bid
            BidTestCase {
                bid_type: BidType::Zero,
                bid_cards: [None, None],
                tricks: 2,
                expected_score: -4,
            },
            // successful easy bid - faceup
            BidTestCase {
                bid_type: BidType::Easy,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 3,
                expected_score: 4,
            },
            // successful easy bid - facedown
            BidTestCase {
                bid_type: BidType::Easy,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 4,
                expected_score: 2,
            },
            // successful easy bid - same value
            BidTestCase {
                bid_type: BidType::Easy,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 2,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 2,
                        id: 0,
                    }),
                ],
                tricks: 2,
                expected_score: 4,
            },
            // failed easy bid
            BidTestCase {
                bid_type: BidType::Easy,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 5,
                expected_score: -2,
            },
            // successful difference bid
            BidTestCase {
                bid_type: BidType::Difference,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 1,
                expected_score: 8,
            },
            // failed difference bid
            BidTestCase {
                bid_type: BidType::Difference,
                bid_cards: [
                    Some(Card {
                        suit: Suit::Red,
                        value: 3,
                        id: 0,
                    }),
                    Some(Card {
                        suit: Suit::Red,
                        value: 4,
                        id: 0,
                    }),
                ],
                tricks: 2,
                expected_score: -2,
            },
        ];
        for test_case in cases.iter() {
            assert_eq!(
                test_case
                    .bid_type
                    .score_for_tricks(test_case.bid_cards, test_case.tricks),
                test_case.expected_score
            );
        }
    }

    #[test]
    fn test_clone_and_apply_move() {
        let mut game = Game::new();
        assert_eq!(game.state, State::DealerSelect);
        game.dealer_select = vec![
            Card {
                id: 5,
                value: 8,
                suit: Suit::Red,
            },
            Card {
                id: 11,
                value: 5,
                suit: Suit::Blue,
            },
        ];

        let new_game = game.clone().clone_and_apply_move(DEALER_SELECT_CARD);
        assert_eq!(new_game.trump_suit, Some(Suit::Red));
        assert_eq!(new_game.state, State::BidType);

        let new_game = game.clone().clone_and_apply_move(DEALER_SELECT_CARD + 1);
        assert_eq!(new_game.trump_suit, Some(Suit::Blue));
        assert_eq!(new_game.state, State::BidType);

        game.dealer_select = vec![
            Card {
                id: 5,
                value: 8,
                suit: Suit::Red,
            },
            Card {
                id: 11,
                value: 5,
                suit: Suit::Red,
            },
        ];

        let new_game = game
            .clone()
            .clone_and_apply_move(DEALER_SELECT_CARD_NO_TRUMP);
        assert_eq!(new_game.trump_suit, None);
        assert_eq!(new_game.state, State::BidType);

        let new_game = game
            .clone()
            .clone_and_apply_move(DEALER_SELECT_CARD_NO_TRUMP + 1);
        assert_eq!(new_game.trump_suit, None);
        assert_eq!(new_game.state, State::BidType);
    }

    #[test]
    fn test_random_playthrough() {
        let mut game = Game::new();
        game.round = 6;
        while game.winner.is_none() {
            let mut moves = game.get_moves();
            moves.shuffle(&mut thread_rng());
            let action = *moves.first().unwrap();
            game = game.clone_and_apply_move(action);
        }
    }

    #[test]
    fn test_mcts_playthrough() {
        let mut iterations = vec![10, 250, 1000];
        let mut wins: HashMap<i32, i32> = HashMap::from_iter(iterations.iter().map(|i| (*i, 0)));
        let mut scores: HashMap<i32, i32> = HashMap::from_iter(iterations.iter().map(|i| (*i, 0)));
        for i in 0..100 {
            iterations.shuffle(&mut thread_rng());
            let mut game = Game::new();
            game.dealer = i % 3;
            game.current_player = i % 3;
            game = game.deal();
            game.round = 6;
            while game.winner.is_none() {
                let action = get_mcts_move(&game, iterations[game.current_player as usize]);
                game = game.clone_and_apply_move(action);
            }
            let max_score: i32 = *game.scores.iter().max().unwrap();
            for player in 0..3 {
                if game.scores[player] == max_score {
                    let wins = wins.get_mut(&iterations[player]).unwrap();
                    *wins += 1;
                }
                let scores = scores.get_mut(&iterations[player]).unwrap();
                *scores += game.scores[player];
            }
            println!("wins: {:?} scores: {:?}", wins, scores);
        }
    }
}
