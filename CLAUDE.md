# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based game engine library for card games, specifically designed to implement AI opponents using Monte Carlo Tree Search (ISMCTS) algorithms. It's part of the Trickster's Table mobile app ecosystem (iOS/Android) and serves as both a research platform and production component.

## Technology Stack

- **Rust 2021 Edition** (fixed to toolchain 1.80.1 via rust-toolchain.toml)
- **ISMCTS (Information Set Monte Carlo Tree Search)** for AI decision-making
- **Serde** for JSON serialization/deserialization
- **Custom game engine architecture** with standardized traits

## Common Development Commands

```bash
# Build the project
cargo build

# Run the main verification system
cargo run

# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench

# Run specific game example
cargo run --example <game_name>
# Available games: briscola, whist, hearts, spades, euchre, cribbage, pinochle, yokai_septet_2p, etc.

# Check code formatting
cargo fmt --check

# Run clippy linter
cargo clippy
```

## Architecture Overview

### Core Components

1. **Game Engine Pattern**: Each game implements standardized traits (`GameState`, `Player`, `Actions`) for ISMCTS integration
2. **ISMCTS Engine**: Centralized AI decision-making using Monte Carlo Tree Search with information sets
3. **Verification System**: Cross-validates Rust implementation against production Dart mobile app using JSON test data
4. **Modular Game Design**: Each game is self-contained in `src/games/` with shared utilities

### Key Directories

- `src/games/` - Individual game implementations (10+ card games)
- `src/ismcts/` - ISMCTS algorithm implementation
- `examples/` - Usage examples and test implementations
- `benches/` - Performance benchmarks
- `data/` - JSON test data for cross-validation with mobile app
- `test_results/` - Performance test outputs and comparisons

### Game Implementation Structure

Each game typically contains:
- Game state management
- Player actions and validation
- Scoring logic
- Game-specific ISMCTS optimizations
- JSON serialization for cross-platform compatibility

## Performance Considerations

- **Optimized for high-throughput AI simulations** (500-1000 ISMCTS iterations)
- **Benchmarking is critical** - run `cargo bench` after performance-related changes
- **Memory efficiency** - games use efficient data structures for rapid state cloning
- **Parallel processing** - ISMCTS can utilize multiple cores for simulations

## Cross-Platform Verification

The codebase includes a verification system that validates Rust AI decisions against the production Dart implementation:
- JSON test data in `data/` directory contains game states and expected moves
- Run `cargo run` to execute full verification suite
- Critical for ensuring consistency between research and production AI

## Special Notes

- **Toolchain locked to 1.80.1** - do not update without testing all games
- **Some games are production-ready** (e.g., Yokai Septet 2P) and used in live mobile app
- **AI evolved from neural networks to pure ISMCTS** for better performance and interpretability
- **Cross-validation with mobile app is mandatory** for any AI logic changes

## Testing Strategy

- Unit tests for individual game components
- Integration tests for ISMCTS with each game
- Performance benchmarks for AI decision speed
- Cross-platform verification against Dart implementation
- JSON-based test data ensures consistency across platforms

## Development Best Practices

- Always run rustfmt after edits
- keep as much logic as possible in the engine - always make logic changes in the engine and keep the frontent focused on simple updates as directed by the engine