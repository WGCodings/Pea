<div align="center">


<img src="./assets/logo.png" width="150" />

# Pea

*A UCI chess engine written in Rust*

[![License: GPL v3][license-badge]][license-link]
[![Release][release-badge]][release-link]
[![Release][lichess-badge0]][lichess-link0]
[![Release][lichess-badge1]][lichess-link1]

</div>

---

## About

This project started as a way to get familiar with Rust around two years go and then, as these things tend to go, the hobby got a little bit out of hand.
This is currently my third attempt to make a chess engine and again I should've done a lot of things differently, but I think it is at a stage where it is release worthy.

Move generation and board representation are handled by the [shakmaty][shakmaty] library. 
As a Rust beginner, I wanted to focus on search and evaluation first rather than getting bogged down in the details of legal move generation. 
Writing my own move generator is on the roadmap and maybe the start of my fourth attempt to make an engine?
Also I'm totally not familiar with SIMD logics so the fast(er) inference code you will see is copied and rewritten
from better engines like [Akimbo][Akimbo] and [Viridithas][Viridithas].

I started training two NNUEs from scratch. One with  (768 -> 1536)x2 -> 1x8 (Net 0) and one 
(768 -> 64)x2 -> 1 (Net 1). For more information about this progression see the CHANGELOG.md. 
However as of todat I'm working with a main (768x4hm -> 1536)x2 -> (32 -> 16 -> 1)x8 multilayer net.
You can play the latest generations of my networks on lichess : [Net 0][lichess-link0], [Net 1][lichess-link1].

---

## Features (not very up-to-date)

### Search
- Negamax with Principal Variation Search
- Aspiration windows
- Transposition table
- Lazy SMP multithreading
- Pondering *(deleted)*

#### Pruning & Reductions
- Reverse futility pruning / Static null move pruning
- Null move pruning
- Razoring
- Futility pruning
- History pruning
- Hanging piece pruning
- Late move pruning
- Late move reductions
- Internal iterative reduction
- Mate distance pruning

#### Extensions
- Singular extensions
- Double & triple extensions

#### Move Ordering
- Static Exchange Evaluation (SEE)
- Killer moves
- History heuristic
- 1, 2, 4 ply continuation history

#### Other
- Improving heuristic
- Quiescence search with SEE
- Time management based on move and eval stability
- WDL normalization
- SIMD code for faster evaluation

### Evaluation
- NNUE trained with the excellent [Bullet] crate
- SPSA trainer *(deleted)*
- correction histories


### UCI Support

The following UCI commands are implemented:

| Command                                                         | Notes                       |
|-----------------------------------------------------------------|-----------------------------|
| `uci`                                                           | Done                        |
| `isready`                                                       | Done                        |
| `ucinewgame`                                                    | Done                        |
| `position <fen\|startpos> [moves ...]`                          | Done                        |
| `go <wtime, btime, winc, binc, movetime, depth, ponder, nodes>` | Need to implement movestogo |
| `ponderhit`                                                     | removed                     |
| `stop`                                                          | Done                        |
| `quit`                                                          | Done                        |
| `setoption name <Hash \| Threads \| Ponder \| Move Overhead>`   | Done                        |
| `perft <depth>`                                                 | Done                        |
| `normalizescore`                                                | Done                        |
| `UCI_ShowWDL`                                                   | Done                        |
---

## Rating

Below is a table of Elo estimates from having the engine play against other engines and itself. Time controls are listed as `time / increment`, in seconds.
[Stash][Stash] (and all its versions) have been used to estimate the rating of this engine.


| Version      | Estimate (8/0.08) | Estimate blitz (60/0.6) | [CCRL Blitz](https://computerchess.org.uk/ccrl/404/) (120/1) | [CCRL 40/15](https://computerchess.org.uk/4040/) (40/15) |
|--------------|-------------------|-------------------------|--------------------------------------------------------------|----------------------------------------------------------|
| v1.0 (Net 1) | 500-800           | /                       | /                                                            | /                                                        |
| v2.0 (Net 1) | 1257              | /                       | 1331                                                         | /                                                        |
| v3.0 (Net 1) | 1981              | /                       | /                                                            | /                                                        |
| v4.0 (Net 1) | 2271              | /                       | /                                                            | /                                                        |
| v6.0 (Net 1) | 2811              | /                       | /                                                            | /                                                        |
| v7.0 (Net 1) | 2850              | /                       | /                                                            | /                                                        |
| v8.0 (Net 0) | 2993              | /                       | /                                                            | /                                                        |
| v9.0 (Net 0) | 3193              | /                       | 3264                                                         | 3223                                                     |
| v9.1 (Net 0) | 3388              | /                       | 3383                                                         | 3344                                                     |
| v10.0.0      | 3563              | 3541                    | /                                                            | /                                                        |
---


## Building

Pea is made with Rust v1.93.1 and easily build with [Cargo][cargo]. Clone the repository and build from the project root.

```bash
git clone https://github.com/WGCodings/Pea.git
```

**Standard build:**
```bash
cargo build --release
```

**Optimized for your CPU:**
```bash
# Windows 
$env:RUSTFLAGS="-C target-cpu=native -C target-feature=+avx2,+bmi2"; cargo build --release
```

The native build enables AVX2 and BMI2 instructions and runs noticeably faster, probably more than 2x. 

**Simplest with Makefile:**

```bash
# Makefile
make release
```

---

## Usage

Running the binary directly drops you into a UCI prompt.
You can use a GUI of your own liking to play against or test it.


---

## Contributing

You can support the Pea by donating compute on my Openbench instance https://openbench.wgcodings.com/ 
or if you have an interest in collaboration, you can always leave me a message.
You can contact me by sending an email to warregevers@hotmail.com.


## Roadmap

- [X] Solve NNUE bias towards White
- [ ] Investigate better NNUE architectures
- [X] Data generator for NNUE training
- [x] King buckets in NNUE
- [ ] Experiment with features for output buckets NNUE
- [X] Make a make file
- [ ] Self-made move generation and board representation
- [x] Correction history (pawn, material done, minor/major/threats/... TODO)
- [x] Successful run of SPSA
- [ ] Clean up code, especially datatypes (too many `as` casts)
- [ ] Better time manager
- [X] Integration with OpenBench
- [ ] Capture history
- [ ] Fix missing PV lines due to cutoffs
- [x] Store evals in TT before recursive search
- [x] Mate Distance Pruning
- [ ] Threat moves and extensions in NMP
- [ ] Probcut
- [x] Try improving heuristic in LMR
- [ ] NMP Verification
- [ ] Contempt
- [x] WDL normalization (#3)
- [ ] Variable window choice in Asp Window

## Acknowledgements

[Simbelmyne][simbelmyne] by Sam Roelants was a major source of inspiration and learning throughout this project. A lot of ideas and how to implement them in Rust came from there.
For some more advanced ideas (like SIMD, accumulator stacks, finny etc) I also grateful to the authors of [viridithas][viridithas] and [akimbo][akimbo]. 
Thanks to [Stockfish' WDL Model][StockFishWDLModel] I was able to normalize the eval score and provide WDL information for every position.
Many thanks to Andy Grant for the [Openbench][Openbench] testing framework, with which testing is a real pleasure.

Thanks to the communities at the **Engine Programming**  Discord servers, the Chess Programming Wiki and TalkChess forum for all the ideas and information.

---

## License

This project is licensed under the [GNU General Public License v3.0][license-link].

<!-- Badges -->
[license-badge]: https://img.shields.io/github/license/WGCodings/Pea?style=for-the-badge&color=blue
[license-link]: https://github.com/WGCodings/Pea/blob/main/LICENSE

[release-badge]: https://img.shields.io/github/v/release/WGCodings/Pea?style=for-the-badge&color=violet
[release-link]: https://github.com/WGCodings/Pea/releases/latest

[lichess-badge0]: https://img.shields.io/badge/Play-Net_0-green?logo=lichess&style=for-the-badge
[lichess-link0]: https://lichess.org/@/PeaNet0

[lichess-badge1]: https://img.shields.io/badge/Play-Net_1-green?logo=lichess&style=for-the-badge
[lichess-link1]: https://lichess.org/@/PeaNet1

<!-- Links -->
[shakmaty]: https://github.com/niklasf/shakmaty
[simbelmyne]: https://github.com/sroelants/simbelmyne
[viridithas]: https://github.com/cosmobobak/viridithas
[akimbo]: https://github.com/jw1912/akimbo
[StockFishWDLModel]: https://github.com/official-stockfish/WDL_model
[Openbench]: https://github.com/AndyGrant/OpenBench
[stash]: https://gitlab.com/mhouppin/stash-bot
[cargo]: https://doc.rust-lang.org/cargo
[arena]: http://www.playwitharena.de
[cutechess]: https://cutechess.com
[shredder]: https://www.shredderchess.com
[bullet]: https://github.com/jw1912/bullet