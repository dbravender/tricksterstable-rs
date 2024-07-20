# Game Engine Implementations in Trickster's Table

| Game                  | Neural network trained with SIMPLE | ISMCTS in Dart (guided by neural network) | ISMCTS in Rust (no neural network) | Heuristics / Notes                                                                                                                                               |
|-----------------------|------------------------------------|-------------------------------------------|------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Boast or Nothing      | ✅                                  | ✅                                         | ❌                                  | ❌                                                                                                                                                                |
| Yokai Septet 2-player | ✅ in versions before 1.0.61        | ✅                                         | ✅ used in 1.0.61+                  | ❌                                                                                                                                                                |
| Potato Man            | ✅                                  | ✅                                         | ❌                                  | ✅ don't waste Potato Man or play Evil Potato when it could be taken                                                                                              |
| Magic Trick           | ✅                                  | ✅                                         | ❌                                  | ✅ determining valid hands is only done around mid-game because I couldn't find or come up with an efficient enough algorithm to do this earlier                  |
| Yokai Septet 4-player | ✅                                  | ✅                                         | ❌                                  | ✅ cards selected to pass are based on a heuristic which was informed by a survey of experienced players evaluating which cards they would pass from random hands |
| Short Zoot Suit       | ✅                                  | ✅                                         | ✅ (not yet used)                   | ✅ never pass on a draw when losing, pass when winning (heuristic not used in pure ISMCTS Rust implementation)                                                    |
| Dealer's Dilemma      | ❌                                  | ❌                                         | ✅                                  | ❌                                                                                                                                                                |

* Magic Trick implementation (in Dart): https://github.com/dbravender/magictrick
* Version of SIMPLE used to train AIs: https://github.com/davidADSP/SIMPLE/pull/34
* Dart ISMCTS library: https://github.com/dbravender/dartmcts
* Rust ISMCTS library: https://github.com/Deliquescence/ismcts

Going forward, I plan to write the game engines in Rust. Rust is much more efficent than Dart (especially the way my library is written) so more simulations can be run in the same amount of time. The overhead of getting inferences from the neural network makes it so only ~200 simulations can be run and the neural networks have a lot of bias. The results for pure ISMCTS appear to be on par or better with a tree search where 500-1000 iterations are run and training neural networks takes a lot of time and experimentation to get similar (or worse) results.

Interesting articles and projects on this topic

* [Reducing the burden of knowledge: Simulation-based methods in imperfect information games](https://www.aifactory.co.uk/newsletter/2013_01_reduce_burden.htm)
* [RLCard: A Toolkit for Reinforcement Learning in Card Games](https://rlcard.org/) - if I ever implement a shedding game I might need to use this if ISMCTS doesn't work
* [Why is machine learning 'hard'?](https://ai.stanford.edu/~zayd/why-is-machine-learning-hard.html) - many of the reasons I moved off of neural networks and back to just ISMCTS (for now) are expounded upon here
