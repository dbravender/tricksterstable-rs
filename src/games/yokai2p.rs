/*
Game: Yokai Septet (2-player variant)
Yokai Septet Designers: yio, Muneyuki Yokouchi (横内宗幸)
2-player variant designer: Sean Ross
BoardGameGeek: https://boardgamegeek.com/boardgame/251433/yokai-septet
*/

use std::{cmp::Ordering, collections::HashSet};

use enum_iterator::{all, Sequence};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, PartialOrd, Ord, Clone, Copy, Sequence, Serialize, Deserialize, Hash, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum Suit {
    Green,
    Purple,
    Pink,
    Yellow,
    Black,
    Red,
    Blue,
}

pub fn suit_offset(suit: Suit) -> i32 {
    match suit {
        Suit::Green => 0,
        Suit::Purple => 1,
        Suit::Pink => 2,
        Suit::Yellow => 3,
        Suit::Black => 4,
        Suit::Red => 5,
        Suit::Blue => 6,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: i32,
    pub value: i32,
    pub suit: Suit,
}

pub fn deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    let mut id = 0;
    for suit in all::<Suit>() {
        for value in 1..=7 {
            deck.push(Card {
                id,
                value: value + suit_offset(suit),
                suit,
            });
            id += 1;
        }
    }
    deck.shuffle(&mut thread_rng());
    deck
}

// Map<int, Card> idToCard = {for (var card in deck(null)) card.id: card};

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    #[default]
    Deal,
    Play,
    TricksToWinner,
    Faceup,
    Trump,
    Shuffle,
    Score,
    ShowPlayable,
    HidePlayable,
    OptionalPause,
    ShowWinningCard,
    CaptureSeven,
    GameOver,
    RevealCard,
    Discard,
    Reorder,
}

#[derive(Debug, Clone, Copy, Sequence, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Location {
    #[default]
    Deck,
    Hand,
    Trump,
    Play,
    Faceup,
    TricksTaken,
    SevensPile,
    Score,
    StrawTop,
    StrawBottom,
    Discard,
    ReorderHand,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub change_type: ChangeType,
    #[serde(rename(serialize = "id", deserialize = "id"))]
    object_id: i32,
    dest: Location,
    tricks_taken: i32,
    start_score: i32,
    end_score: i32,
    hand_offset: usize,
    player: usize,
    length: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Discard,
    PlayCard,
    Swap,
    SelectSeven,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Yokai2pGame {
    state: State,
    trump_card: Option<Card>,
    hands: [Vec<Card>; 2],
    changes: Vec<Vec<Change>>,
    current_trick: [Option<Card>; 2],
    tricks_taken: [i32; 2],
    lead_suit: Option<Suit>,
    scores: [i32; 2],
    overall_scores: [i32; 2],
    pub voids: [HashSet<Suit>; 2],
    captured_sevens: [Vec<Card>; 2],
    straw_bottom: [Vec<Option<Card>>; 2],
    straw_top: [Vec<Option<Card>>; 2],
    current_player: usize,
    winner: Option<usize>,
    overall_winner: Option<usize>,
    lead_player: usize,
    round: i32,
    pub no_changes: bool, // save time when running simulations by skipping animation metadata
}

impl Yokai2pGame {
    pub fn deal(mut self) {
        self.lead_suit = None;
        self.round += 1;
        self.tricks_taken = [0, 0];
        self.hands = [vec![], vec![]];
        self.state = State::Discard;
        self.current_player = self.lead_player;
        self.lead_player = (self.lead_player + 1) % 2;
        self.captured_sevens = [vec![], vec![]];
        self.voids = [HashSet::new(), HashSet::new()];
        self.changes.extend(vec![]); // deal
        let deal_index = self.changes.len() - 1 as usize;
        let mut cards = deck();
        let trump_card = cards.pop().unwrap();
        self.add_change(
            deal_index,
            Change {
                change_type: ChangeType::Trump,
                object_id: trump_card.id,
                dest: Location::Trump,
                ..Default::default()
            },
        );
        self.straw_bottom = [vec![], vec![]];
        for y in 0..7 {
            for player in 0..2 as usize {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::StrawBottom,
                        player,
                        hand_offset: y,
                        length: 7,
                        ..Default::default()
                    },
                );
                self.straw_bottom[player].push(Some(card));
            }
        }
        // To avoid having to deal with moving 7s - okayed with Sean
        let removed_sevens: Vec<Card> = cards.iter().filter(|c| c.value == 7).cloned().collect();
        cards.retain(|c| c.value != 7);
        // End dealing with sevens
        self.straw_top = [vec![], vec![]];
        for y in 0..6 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::StrawTop,
                        player,
                        hand_offset: y,
                        length: 6,
                        ..Default::default()
                    },
                );
                self.straw_top[player].push(Some(card));
            }
        }
        // To avoid having to deal with moving 7s - okayed with Sean
        cards.extend(removed_sevens);
        cards.shuffle(&mut thread_rng());
        // End dealing with sevens
        self.hands = [vec![], vec![]];
        for y in 0..11 {
            for player in 0..2 {
                let card = cards.pop().unwrap();
                self.hands[player].push(card);
            }
        }
        self.hands[0].sort_by(card_sorter);
        for y in 0..11 {
            for player in 0..2 {
                let card = self.hands[player][y];
                self.add_change(
                    deal_index,
                    Change {
                        change_type: ChangeType::Deal,
                        object_id: card.id,
                        dest: Location::Hand,
                        player,
                        hand_offset: y,
                        length: 11,
                        ..Default::default()
                    },
                );
            }
        }
        self.show_playable();
    }

    #[inline]
    fn add_change(&mut self, index: usize, change: Change) {
        if !self.no_changes {
            self.changes[index].push(change);
        }
    }

    fn show_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.changes.len() - 1;
        if self.current_player == 0 {
            for id in self.get_moves() {
                self.add_change(
                    change_index,
                    Change {
                        object_id: id,
                        change_type: ChangeType::ShowPlayable,
                        dest: Location::Hand,
                        player: self.current_player,
                        ..Default::default()
                    },
                );
            }
        } else {
            self.hide_playable();
        }
    }

    fn hide_playable(&mut self) {
        if self.changes.is_empty() {
            self.changes = vec![vec![]];
        }
        let change_index = self.changes.len() - 1;
        let mut cards = self.hands[0].clone();
        cards.extend(self.exposed_straw_bottoms(0));
        cards.extend(
            self.straw_top[0]
                .iter()
                .filter_map(|x| *x)
                .collect::<Vec<_>>(),
        );
        for card in cards {
            self.add_change(
                change_index,
                Change {
                    object_id: card.id,
                    change_type: ChangeType::HidePlayable,
                    dest: Location::Hand,
                    player: self.current_player,
                    ..Default::default()
                },
            );
        }
    }

    fn exposed_straw_bottoms(&self, player: usize) -> HashSet<Card> {
        let mut exposed_cards: HashSet<Card> = HashSet::new();
        for (i, card) in self.straw_bottom[player].iter().cloned().enumerate() {
            if card.is_none() {
                continue;
            }
            let left_open: bool;
            let right_open: bool;
            if i == 0 {
                left_open = true;
            } else {
                left_open = self.straw_top[player][i - 1].is_none();
            }
            if i == 6 {
                right_open = true;
            } else {
                right_open = self.straw_top[player][i].is_none();
            }
            if left_open && right_open {
                exposed_cards.insert(card.unwrap());
            }
        }
        return exposed_cards;
    }

    fn get_moves(&self) -> Vec<i32> {
        if self.state == State::Discard {
            return self.hands[self.current_player]
                .iter()
                .filter(|c| c.value != 7) // can't discard 7s
                .map(|c| c.id)
                .collect();
        }
        // must follow
        let mut playable_cards = self.visible_straw(self.current_player);
        playable_cards.extend(self.hands[self.current_player].clone());

        if let Some(lead_suit) = self.lead_suit {
            let moves: Vec<i32> = playable_cards
                .iter()
                .filter(|c| c.suit == lead_suit)
                .map(|c| c.id)
                .collect();
            if !moves.is_empty() {
                return moves;
            }
        }
        return playable_cards.iter().map(|c| c.id).collect();
    }

    fn visible_straw(&self, player: usize) -> Vec<Card> {
        let mut visible: Vec<Card> = self.straw_top[player].iter().filter_map(|x| *x).collect();
        visible.extend(self.exposed_straw_bottoms(player));
        return visible;
    }

    fn hidden_straw(&self, player: usize) -> HashSet<Card> {
        let exposed_straw_bottoms = self.exposed_straw_bottoms(player);
        return self.straw_bottom[player]
            .iter()
            .filter_map(|x| *x)
            .filter(|x| !exposed_straw_bottoms.contains(x))
            .collect();
    }

    pub fn reveal_straw_bottoms(&self, player: usize) -> Vec<Change> {
        return self
            .exposed_straw_bottoms(player)
            .iter()
            .map(|c| Change {
                change_type: ChangeType::RevealCard,
                dest: Location::Hand,
                object_id: c.id,
                ..Default::default()
            })
            .collect();
    }

    pub fn reorder_hand(&self, player: usize) -> Vec<Change> {
        let length = self.hands[self.current_player].len();
        self.hands[self.current_player]
            .iter()
            .enumerate()
            .map(|(hand_offset, card)| Change {
                change_type: ChangeType::Reorder,
                dest: Location::ReorderHand,
                object_id: card.id,
                player,
                hand_offset,
                length,
                ..Default::default()
            })
            .collect()
    }
}

// state = State.playCard;

/*
@JsonSerializable()
class Game implements GameState<Move, Player> {



  @override
  Game cloneAndApplyMove(Move move, Node<Move, Player>? root) {
    if (!getMoves().contains(move)) {
      parent = null;
      d.log('game: $this move: $move legalMoves: ${getMoves()}');
    }
    var newGame = clone();
    // reset previous MCTS round winner
    newGame.winner = null;
    // reset previous MCTS round scores
    newGame.scores = {0: 0, 1: 0};
    newGame.changes = [[]]; // card from player to table
    newGame.currentTrick = Map.from(currentTrick);
    List<Card> currentHand = newGame.hands[currentPlayer!];
    newGame.hands[currentPlayer!] = currentHand;
    var card = idToCard[move]!;
    newGame.playedCards[newGame.currentPlayer!].add(card);
    if (newGame.state == State.discard) {
      currentHand.remove(card);
      newGame.changes.add([
        Change(id: card.id, type: ChangeType.discard, dest: Location.discard),
        ...newGame.reorderHand(currentPlayer!),
      ]);
      if (newGame.hands.every((h) => h.length == 10)) {
        newGame.state = State.playCard;
      }
      newGame.currentPlayer = (currentPlayer! + 1) % 2;
      showPlayable(this, newGame);
      return newGame;
    }
    List<Card?> strawBottom = newGame.strawBottom[currentPlayer!];
    List<Card?> strawTop = newGame.strawTop[currentPlayer!];
    if (strawBottom.contains(card)) {
      int index = strawBottom.indexOf(card);
      strawBottom[index] = null;
    } else if (strawTop.contains(card)) {
      int index = strawTop.indexOf(card);
      strawTop[index] = null;
    } else {
      currentHand.remove(card);
    }
    newGame.changes[0].addAll([
      Change(
          type: ChangeType.play,
          id: move,
          dest: Location.play,
          player: currentPlayer!),
      ...newGame.reorderHand(currentPlayer!),
    ]);
    newGame.currentTrick[currentPlayer!] = card;
    if (newGame.leadSuit != null && card.suit != newGame.leadSuit) {
      // Player has revealed a void
      newGame.voids[currentPlayer!][newGame.leadSuit!] = true;
    }
    newGame.leadSuit ??= card.suit;
    newGame.currentPlayer = (currentPlayer! + 1) % 2;
    hidePlayable(this, newGame);
    // end trick
    if (newGame.currentTrick.length == 2) {
      int trickWinner = getWinner(
          newGame.leadSuit!, newGame.trumpCard!, newGame.currentTrick);
      Card winningCard = newGame.currentTrick[trickWinner]!;
      newGame.tricksTaken[trickWinner] = newGame.tricksTaken[trickWinner]! + 1;
      // winner of the trick leads
      newGame.currentPlayer = trickWinner;
      newGame.changes.add([
        Change(
            type: ChangeType.showWinningCard,
            id: winningCard.id,
            dest: Location.play),
        Change(type: ChangeType.optionalPause, id: 0, dest: Location.play),
        ...newGame.revealStrawBottoms(0),
        ...newGame.revealStrawBottoms(1),
      ]);
      List<Change> sevenChanges = [];
      List<Change> trickChanges = [];
      newGame.changes.addAll(
        [
          sevenChanges, // sevens
          trickChanges, // trick to team
        ],
      );
      newGame.currentTrick.forEach((player, card) {
        if (card.value == 7) {
          newGame.capturedSevens[trickWinner].add(card);
          sevenChanges.add(Change(
              type: ChangeType.captureSeven,
              id: card.id,
              dest: Location.sevensPile,
              handOffset: newGame.capturedSevens[trickWinner].indexOf(card),
              player: trickWinner));
        } else {
          trickChanges.add(Change(
              type: ChangeType.tricksToWinner,
              id: card.id,
              dest: Location.tricksTaken,
              player: trickWinner,
              tricksTaken: newGame.tricksTaken[trickWinner]!));
        }
      });
      newGame.currentTrick = {};
      newGame.leadSuit = null;

      int? handWinningPlayer;

      // player with >= 4 sevens wins the round
      newGame.capturedSevens.asMap().forEach((player, c7s) {
        if (c7s.length >= 4) {
          handWinningPlayer = player;
        }
      });

      // player with >= 7 tricks loses

      if (handWinningPlayer == null) {
        List<List<Card>> overallHands = [
          newGame.hands[0] +
              newGame.strawBottom[0].whereType<Card>().toList() +
              newGame.strawTop[0].whereType<Card>().toList(),
          newGame.hands[1] +
              newGame.strawBottom[1].whereType<Card>().toList() +
              newGame.strawTop[1].whereType<Card>().toList()
        ];
        if (overallHands.every((x) => x.isEmpty)) {
          handWinningPlayer = newGame.currentPlayer;
        }
        newGame.tricksTaken.forEach((player, tricks) {
          if (tricks >= 13) {
            // the other player won
            handWinningPlayer = (player + 1) % 2;
          }
        });
        if (handWinningPlayer != null) {
          List<Card> sevens = [];
          for (var hand in newGame.hands) {
            sevens.addAll(hand.where((x) => x.value == 7));
          }
          for (var pile in newGame.strawTop) {
            sevens.addAll(pile.whereType<Card>().where((x) => x.value == 7));
          }
          for (var pile in newGame.strawBottom) {
            sevens.addAll(pile.whereType<Card>().where((x) => x.value == 7));
          }
          newGame.capturedSevens[handWinningPlayer!].addAll(sevens);
          for (var seven in sevens) {
            sevenChanges.add(Change(
                type: ChangeType.captureSeven,
                id: seven.id,
                dest: Location.sevensPile,
                handOffset:
                    newGame.capturedSevens[handWinningPlayer!].indexOf(seven),
                player: handWinningPlayer!));
          }
        }
      }

      if (handWinningPlayer != null) {
        var c7s = newGame.capturedSevens[handWinningPlayer!];
        newGame.scores[handWinningPlayer!] =
            newGame.scores[handWinningPlayer]! +
                scoreSevens(c7s, newGame.trumpCard!);
        newGame.overallScores[handWinningPlayer!] =
            newGame.overallScores[handWinningPlayer]! +
                scoreSevens(c7s, newGame.trumpCard!);
        newGame.changes.add([
          Change(
              id: 0,
              type: ChangeType.score,
              dest: Location.score,
              startScore: overallScores[handWinningPlayer]!,
              endScore: newGame.overallScores[handWinningPlayer]!,
              player: handWinningPlayer!),
          Change(type: ChangeType.optionalPause, id: 0, dest: Location.play),
        ]);

        int? gameWinner;

        if (handWinningPlayer != null) {
          newGame.winner = handWinningPlayer;
        }

        newGame.overallScores.forEach((player, score) {
          if (score >= 7) {
            gameWinner = player;
            newGame.winner = player;
          }
        });

        if (gameWinner != null) {
          newGame.overallWinner = gameWinner;
          newGame.winner = gameWinner;
          newGame.changes.add(
              [Change(type: ChangeType.gameOver, id: 0, dest: Location.deck)]);
          return newGame;
        } else {
          newGame.changes.add(
              [Change(type: ChangeType.shuffle, id: 0, dest: Location.deck)]);
          newGame.deal();
          return newGame;
        }
      }
    }
    showPlayable(this, newGame);
    return newGame;
  }


*/

impl ismcts::Game for Yokai2pGame {
    type Move = i32;
    type PlayerTag = i32;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        let rng = &mut thread_rng();
        let mut remaining_cards: Vec<Card> = vec![];
        let mut hidden_straw_bottoms: [HashSet<Card>; 2] = [HashSet::new(), HashSet::new()];

        for player in 0..2 {
            if player != self.current_player {
                remaining_cards.extend(self.hands[player].iter());
            }

            hidden_straw_bottoms[player] =
                HashSet::from_iter(self.straw_bottom[player].iter().filter_map(|&x| x))
                    .difference(&self.exposed_straw_bottoms(player))
                    .cloned()
                    .collect();

            remaining_cards.extend(hidden_straw_bottoms[player].iter());
        }

        remaining_cards.shuffle(rng);

        for player in 0..2 {
            let original_hand_length: usize = self.hands[player].len();
            if player != self.current_player {
                let mut pc = extract_short_suited_cards(&remaining_cards, &self.voids[player]);
                self.hands[player] = vec![];
                pc.cards.shuffle(rng);
                for _ in 0..original_hand_length {
                    let card = pc.cards.pop().unwrap();
                    self.hands[player].push(card);
                }
                remaining_cards.extend(pc.leftovers);
                remaining_cards.extend(pc.cards);
            }
            assert!(original_hand_length == self.hands[player].len());
        }

        remaining_cards.shuffle(rng);
        for player in 0..2 {
            for i in 0..self.straw_bottom[player].len() {
                let card = self.straw_bottom[player][i];
                if !card.is_none() && hidden_straw_bottoms[player].contains(&card.unwrap()) {
                    self.straw_bottom[player][i] = remaining_cards.pop();
                }
            }
        }

        assert!(remaining_cards.is_empty());
    }

    fn current_player(&self) -> Self::PlayerTag {
        todo!()
    }

    fn next_player(&self) -> Self::PlayerTag {
        todo!()
    }

    fn available_moves(&self) -> Self::MoveList {
        todo!()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        todo!()
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        todo!()
    }
}

pub fn get_winner(lead_suit: Suit, trump_card: Card, trick: [Option<Card>; 2]) -> i32 {
    if value_for_card(lead_suit, trump_card, trick[0].unwrap())
        > value_for_card(lead_suit, trump_card, trick[1].unwrap())
    {
        0
    } else {
        1
    }
}

pub fn value_for_card(lead_suit: Suit, trump_card: Card, card: Card) -> i32 {
    if card.value == 1 && card.suit == Suit::Green {
        return 1000;
    }
    if card.suit == lead_suit {
        return card.value + 50;
    }
    if card.suit == trump_card.suit {
        return card.value + 100;
    }
    return card.value;
}

pub fn seven_value(suit: &Suit) -> i32 {
    match suit {
        Suit::Green => 0,
        Suit::Purple => 0,
        Suit::Pink => 1,
        Suit::Yellow => 1,
        Suit::Black => 1,
        Suit::Red => 2,
        Suit::Blue => 2,
    }
}

pub fn score_sevens(sevens: Vec<Card>, trump_card: Card) -> i32 {
    sevens
        .iter()
        .filter(|&card| card.suit != trump_card.suit)
        .map(|card| seven_value(&card.suit))
        .sum()
}

pub fn card_sorter(a: &Card, b: &Card) -> Ordering {
    match a.suit.cmp(&b.suit) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.value.cmp(&b.value),
    }
}

struct PossibleCards {
    cards: Vec<Card>,
    leftovers: Vec<Card>,
}

pub fn extract_short_suited_cards(
    remaining_cards: &Vec<Card>,
    voids: &HashSet<Suit>,
) -> PossibleCards {
    let mut leftovers: Vec<Card> = vec![];

    let mut possible_cards = remaining_cards.clone();

    for suit in voids {
        possible_cards.retain(|card| {
            let belongs_to_suit = card.suit == *suit;
            if belongs_to_suit {
                leftovers.push(*card);
            }
            !belongs_to_suit
        });
    }
    return PossibleCards {
        cards: possible_cards,
        leftovers,
    };
}
