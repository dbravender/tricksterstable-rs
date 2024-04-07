# tricksterstable-rs

Trickster's Table is an app that is written in Dart/Flutter and available on iOS and Android:

- iOS: https://apps.apple.com/us/app/tricksters-table/id1668506875
- Android: https://play.google.com/store/apps/details?id=app.playagame.tiger

This repository contains an experimental Rust implementation of some games featured in the app. Other games were implemented in Dart and trained using https://github.com/davidADSP/SIMPLE/pull/34. The goal of writing the engines in Rust is that rust is fast so more simulations can be run in the same amount of time (using [Monte Carlo Tree Search](https://en.wikipedia.org/wiki/Monte_Carlo_tree_search)).
