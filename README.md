# tricksterstable-rs

Trickster's Table is an app that is written in Dart/Flutter and available on iOS and Android.

- List of upcoming games and more information: https://boardgamegeek.com/geeklist/226363/tricksters-table-androidios-app
- iOS: https://apps.apple.com/us/app/tricksters-table/id1668506875
- Android: https://play.google.com/store/apps/details?id=app.playagame.tiger
- [Implementation details of each game in the app](games.md)

This repository contains implementations of some of the games featured in the app. Other games were implemented in Dart and trained using https://github.com/davidADSP/SIMPLE/pull/34. The goal of writing the engines in Rust is that Rust is fast so more simulations can be run in the same amount of time (using [Monte Carlo Tree Search](https://en.wikipedia.org/wiki/Monte_Carlo_tree_search)).
