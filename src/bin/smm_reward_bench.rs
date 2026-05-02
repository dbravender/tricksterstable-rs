// Benchmark different ISMCTS reward functions for SMM.
//
// Each reward function is tested head-to-head against the current default.
// All strategies play as MCTS with the same iteration count; one random player fills seat 2.

use ismcts::IsmctsHandler;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use std::time::Instant;
use tricksterstable_rs::games::smm::{ResultMode, SMMGame, State};

const ITERATIONS: usize = 1000;
const PARALLEL_THREADS: usize = 8;
const GAMES_PER_MATCHUP: usize = 200;

// ---------------------------------------------------------------------------
// Reward wrapper: a thin wrapper around SMMGame that overrides `result()`
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RewardKind {
    /// Current default: win=1, tie=0, loss=-1, coin bonus 0.05
    Default,
    /// Score-margin: normalized score difference instead of binary
    ScoreMargin,
    /// Rank-based: 1st=1.0, 2nd=0.0, 3rd=-1.0 (with fractional ties)
    RankBased,
    /// Cards-remaining penalty: binary win/loss + penalty for cards left in hand
    CardsPenalty,
    /// Combined: score margin + cards penalty + coin bonus
    Combined,
    /// Tie-friendly: ties for first count as 0.5 instead of 0.0
    TieFriendly,
    /// Aggressive margin: larger spread between win margins
    AggressiveMargin,
}

impl std::fmt::Display for RewardKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewardKind::Default => write!(f, "Default (win/loss/coin)"),
            RewardKind::ScoreMargin => write!(f, "ScoreMargin"),
            RewardKind::RankBased => write!(f, "RankBased"),
            RewardKind::CardsPenalty => write!(f, "CardsPenalty"),
            RewardKind::Combined => write!(f, "Combined"),
            RewardKind::TieFriendly => write!(f, "TieFriendly"),
            RewardKind::AggressiveMargin => write!(f, "AggressiveMargin"),
        }
    }
}

#[derive(Clone)]
struct RewardGame {
    game: SMMGame,
    reward: RewardKind,
}

impl RewardGame {
    fn new(game: SMMGame, reward: RewardKind) -> Self {
        Self { game, reward }
    }

    fn compute_result(&self, player: usize) -> Option<f64> {
        if self.game.state != State::GameOver {
            return None;
        }

        let scores = &self.game.scores;
        let player_score = scores[player] as f64;
        let max_score = *scores.iter().max().unwrap() as f64;
        let min_score = *scores.iter().min().unwrap() as f64;
        let total: f64 = scores.iter().map(|&s| s as f64).sum();
        let hand_size = self.game.hands[player].len() as f64;
        let has_coin = self.game.has_lucky_coin[player];

        match &self.reward {
            RewardKind::Default => {
                // Current implementation
                let base = if player_score == max_score {
                    if scores.iter().filter(|&&s| s as f64 == player_score).count() > 1 {
                        0.0
                    } else {
                        1.0
                    }
                } else {
                    -1.0
                };
                let coin_bonus = if has_coin { 0.05 } else { 0.0 };
                Some(base + coin_bonus)
            }

            RewardKind::ScoreMargin => {
                // Normalize score to [-1, 1] based on distance from mean
                let mean = total / 3.0;
                let range = if max_score > min_score {
                    max_score - min_score
                } else {
                    1.0
                };
                Some((player_score - mean) / range)
            }

            RewardKind::RankBased => {
                // Rank players: 1st=1.0, 2nd=0.0, 3rd=-1.0
                // Ties share the average of their ranks
                let mut sorted: Vec<f64> = scores.iter().map(|&s| s as f64).collect();
                sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());

                let mut rank_sum = 0.0;
                let mut count = 0.0;
                for (i, &s) in sorted.iter().enumerate() {
                    if s == player_score {
                        rank_sum += i as f64;
                        count += 1.0;
                    }
                }
                let avg_rank = rank_sum / count; // 0.0 = first, 1.0 = second, 2.0 = third
                Some(1.0 - avg_rank) // maps to 1.0, 0.0, -1.0
            }

            RewardKind::CardsPenalty => {
                // Binary win/loss but penalize remaining cards
                let base = if player_score == max_score {
                    if scores.iter().filter(|&&s| s as f64 == player_score).count() > 1 {
                        0.0
                    } else {
                        1.0
                    }
                } else {
                    -1.0
                };
                // Penalty: up to -0.3 for having cards remaining (max 15 cards)
                let cards_penalty = -0.3 * (hand_size / 15.0);
                Some(base + cards_penalty)
            }

            RewardKind::Combined => {
                // Score margin (primary) + cards penalty + coin bonus
                let mean = total / 3.0;
                let range = if max_score > min_score {
                    max_score - min_score
                } else {
                    1.0
                };
                let margin = (player_score - mean) / range;
                let cards_penalty = -0.2 * (hand_size / 15.0);
                let coin_bonus = if has_coin { 0.05 } else { 0.0 };
                Some(margin + cards_penalty + coin_bonus)
            }

            RewardKind::TieFriendly => {
                // Like default but ties for first = 0.5
                let base = if player_score == max_score {
                    if scores.iter().filter(|&&s| s as f64 == player_score).count() > 1 {
                        0.5
                    } else {
                        1.0
                    }
                } else {
                    -1.0
                };
                let coin_bonus = if has_coin { 0.05 } else { 0.0 };
                Some(base + coin_bonus)
            }

            RewardKind::AggressiveMargin => {
                // Strongly reward winning by large margins
                let base = if player_score == max_score {
                    if scores.iter().filter(|&&s| s as f64 == player_score).count() > 1 {
                        0.3
                    } else {
                        1.0
                    }
                } else {
                    -1.0
                };
                // Bonus proportional to lead over second place
                let mut others: Vec<f64> = scores
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| i != player)
                    .map(|(_, &s)| s as f64)
                    .collect();
                others.sort_by(|a, b| b.partial_cmp(a).unwrap());
                let second_best = others[0];
                let gap = (player_score - second_best) / 9.0; // max possible gap ~9
                Some(base + gap * 0.3)
            }
        }
    }
}

// Implement the ISMCTS Game trait for RewardGame
impl ismcts::Game for RewardGame {
    type Move = i32;
    type PlayerTag = usize;
    type MoveList = Vec<i32>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        <SMMGame as ismcts::Game>::randomize_determination(&mut self.game, observer);
    }

    fn current_player(&self) -> Self::PlayerTag {
        <SMMGame as ismcts::Game>::current_player(&self.game)
    }

    fn next_player(&self) -> Self::PlayerTag {
        <SMMGame as ismcts::Game>::next_player(&self.game)
    }

    fn available_moves(&self) -> Self::MoveList {
        <SMMGame as ismcts::Game>::available_moves(&self.game)
    }

    fn make_move(&mut self, mov: &Self::Move) {
        <SMMGame as ismcts::Game>::make_move(&mut self.game, mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        self.compute_result(player)
    }
}

fn mcts_move(game: &SMMGame, reward: &RewardKind) -> i32 {
    let mut sim_game = game.clone();
    sim_game.no_changes = true;
    sim_game.result_mode = ResultMode::RoundOnly;
    let rg = RewardGame::new(sim_game, reward.clone());
    let mut ismcts = IsmctsHandler::new(rg);
    ismcts.run_iterations(PARALLEL_THREADS, ITERATIONS / PARALLEL_THREADS);
    ismcts.best_move().expect("should have a move")
}

fn random_move(game: &SMMGame) -> i32 {
    let mut moves = game.get_moves();
    moves.shuffle(&mut thread_rng());
    *moves.first().expect("should have a move")
}

/// Head-to-head: P0 = strategy_a, P1 = strategy_b, P2 = random
fn play_game(strategy_a: &RewardKind, strategy_b: &RewardKind) -> [i32; 3] {
    let mut game = SMMGame::new();
    game.no_changes = true;

    while game.winner.is_none() {
        let moves = game.get_moves();
        if moves.is_empty() {
            break;
        }

        let mov = match game.current_player {
            0 => mcts_move(&game, strategy_a),
            1 => mcts_move(&game, strategy_b),
            _ => random_move(&game),
        };
        game.apply_move(mov);
    }
    game.scores
}

fn run_matchup(label: &str, strategy_a: &RewardKind, strategy_b: &RewardKind) {
    let start = Instant::now();
    let mut score_totals = [0i64; 3];
    let mut win_counts = [0usize; 3];
    let mut tie_counts = [0usize; 3];

    for i in 0..GAMES_PER_MATCHUP {
        if i % 10 == 0 {
            eprint!(".");
        }
        let scores = play_game(strategy_a, strategy_b);
        let max = *scores.iter().max().unwrap();
        for p in 0..3 {
            score_totals[p] += scores[p] as i64;
            if scores[p] == max {
                if scores.iter().filter(|&&s| s == max).count() == 1 {
                    win_counts[p] += 1;
                } else {
                    tie_counts[p] += 1;
                }
            }
        }
    }
    eprintln!();

    let elapsed = start.elapsed();
    println!("{}", label);
    let labels = [
        format!("  P0 ({})", strategy_a),
        format!("  P1 ({})", strategy_b),
        "  P2 (Random)".to_string(),
    ];
    for p in 0..3 {
        println!(
            "{}: wins={}/{} ({:.1}%), ties={}, avg_score={:.1}",
            labels[p],
            win_counts[p],
            GAMES_PER_MATCHUP,
            100.0 * win_counts[p] as f64 / GAMES_PER_MATCHUP as f64,
            tie_counts[p],
            score_totals[p] as f64 / GAMES_PER_MATCHUP as f64
        );
    }
    println!("  time={:.1}s\n", elapsed.as_secs_f64());
}

fn main() {
    println!("=== SMM Reward Function Benchmark ===");
    println!(
        "ISMCTS iterations: {}, threads: {}, games per matchup: {}\n",
        ITERATIONS, PARALLEL_THREADS, GAMES_PER_MATCHUP
    );

    let challengers = vec![
        RewardKind::ScoreMargin,
        RewardKind::RankBased,
        RewardKind::CardsPenalty,
        RewardKind::Combined,
        RewardKind::TieFriendly,
        RewardKind::AggressiveMargin,
    ];

    // Each challenger vs Default
    for challenger in &challengers {
        // Position A (P0=challenger, P1=default)
        run_matchup(
            &format!("--- {} vs Default (challenger=P0) ---", challenger),
            challenger,
            &RewardKind::Default,
        );

        // Swap positions to control for seat advantage
        run_matchup(
            &format!("--- {} vs Default (challenger=P1) ---", challenger),
            &RewardKind::Default,
            challenger,
        );
    }

    // Summary: top challengers vs each other
    println!("=== Round Robin Among All Strategies ===\n");
    let all = [
        RewardKind::Default,
        RewardKind::ScoreMargin,
        RewardKind::RankBased,
        RewardKind::CardsPenalty,
        RewardKind::Combined,
        RewardKind::TieFriendly,
        RewardKind::AggressiveMargin,
    ];

    // Accumulate total wins across all matchups
    let mut total_wins: Vec<(String, usize, usize)> =
        all.iter().map(|s| (format!("{}", s), 0, 0)).collect();

    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let scores = (0..GAMES_PER_MATCHUP)
                .map(|g| {
                    if g % 10 == 0 {
                        eprint!(".");
                    }
                    play_game(&all[i], &all[j])
                })
                .collect::<Vec<_>>();
            eprintln!();

            let mut wins_i = 0;
            let mut wins_j = 0;
            for s in &scores {
                let max = *s.iter().max().unwrap();
                if s[0] == max && s.iter().filter(|&&x| x == max).count() == 1 {
                    wins_i += 1;
                }
                if s[1] == max && s.iter().filter(|&&x| x == max).count() == 1 {
                    wins_j += 1;
                }
            }

            println!(
                "{} vs {}: {} wins {}, {} wins {}",
                all[i], all[j], all[i], wins_i, all[j], wins_j
            );

            total_wins[i].1 += wins_i;
            total_wins[i].2 += GAMES_PER_MATCHUP;
            total_wins[j].1 += wins_j;
            total_wins[j].2 += GAMES_PER_MATCHUP;
        }
    }

    println!("\n=== Overall Win Rates (round robin) ===");
    total_wins.sort_by(|a, b| {
        let rate_a = a.1 as f64 / a.2 as f64;
        let rate_b = b.1 as f64 / b.2 as f64;
        rate_b.partial_cmp(&rate_a).unwrap()
    });
    for (name, wins, games) in &total_wins {
        println!(
            "  {}: {}/{} ({:.1}%)",
            name,
            wins,
            games,
            100.0 * *wins as f64 / *games as f64
        );
    }

    println!("\nDone.");
}
