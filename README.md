<div align="center">


<img src="./assets/logo.png" width="150" />

# Pea

*A UCI chess engine written in Rust*

[![License: GPL v3][license-badge]][license-link]
[![Release][release-badge]][release-link]
[![Release][lichess-badge]][lichess-link]


</div>

---

## About

This project started as a way to get familiar with Rust around two years go — and then, as these things tend to go, it got a little bit out of hand. 
What began as tinkering with search algorithms has grown into a reasonably capable chess engine with NNUE evaluation, multithreading, and pondering support.
This is currently my third attempt to make a chess engine and again I should've done a lot of things differently, but I think it is at a stage where it is release worthy.

Move generation and board representation are handled by the [shakmaty][shakmaty] library. 
As a Rust beginner, I wanted to focus on search and evaluation first rather than getting bogged down in the details of legal move generation. 
Writing my own move generator is on the roadmap and maybe the start of my fourth attempt to make an engine?

---

## Features

### Search
- Negamax with Principal Variation Search
- Aspiration windows
- Transposition table (lockless)
- Lazy SMP multithreading
- Pondering *(~90% complete)*

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

### Evaluation
- NNUE with architecture `(768 → 1536) × 2 → 1 × 8` trained with the excellent [Bullet] crate
- SPSA trainer *(work in progress)*


### UCI Support

The following UCI commands are implemented:

| Command | Notes                       |
|---|-----------------------------|
| `uci` | Done                        |
| `isready` | Done                        |
| `ucinewgame` | Done                        |
| `position <fen\|startpos> [moves ...]` | Done                        |
| `go <wtime, btime, winc, binc, movetime, depth, ponder>` | Need to implement movestogo |
| `ponderhit` | 90% done                    |
| `stop` | Done                        |
| `quit` | Done                        |
| `setoption name <Hash \| Threads \| Ponder \| Move Overhead>` | Done                        |
| `perft <depth>` | Done                        |

---

## Rating

Below is a table of Elo estimates from having the engine play against other engines and itself. Time controls are listed as `time / increment`, in seconds.
[Stash][Stash] (and all its versions) have been used to estimate the rating of this engine.


| Version | Estimate (5/0.1) | [CCRL](https://computerchess.org.uk/ccrl/4040/) (40/15) | [CCRL Blitz](https://computerchess.org.uk/ccrl/404b/) (2/1) |
|-------|------------------|---------------------------------------------------------|-------------------------------------------------------------|
| v1.0  | 3379*            | None   yet                                              | None            yet                                         |

*Results from a match against Stash v36 which should be around 3384 ELO -> see below

--------------------------------------------------

Results of PEA vs STASH36 (5+0.1, 1t, 256MB, book.epd):

Elo: -4.84 +/- 6.08, nElo: -5.86 +/- 7.36

LOS: 5.95 %, DrawRatio: 38.71 %, PairsRatio: 0.98

Games: 8550, Wins: 3213, Losses: 3332, Draws: 2005, Points: 4215.5 (49.30 %)

Ptnml(0-2): [585, 735, 1655, 814, 486], WL/DD Ratio: 6.26

LLR: -2.96 (-100.5%) (-2.94, 2.94) [0.00, 5.00]

--------------------------------------------------




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

**Recommended — optimized for your CPU:**
```bash
# Linux / macOS
RUSTFLAGS="-C target-cpu=native -C target-feature=+avx2,+bmi2" cargo build --release

# Windows (PowerShell)
$env:RUSTFLAGS="-C target-cpu=native -C target-feature=+avx2,+bmi2"; cargo build --release
```

The native build enables AVX2 and BMI2 instructions and runs noticeably faster. It is strongly recommended unless you are distributing the binary to other machines.


---

## Usage

Running the binary directly drops you into a UCI prompt. In practice, you'll want to use a UCI-compatible frontend:

- [Arena][arena]
- [Cutechess][cutechess]
- [Shredder][shredder]


The engine is also available to play on [lichess][lichess-link].

---


## Roadmap

- [ ] Self-made move generation and board representation
- [ ] Correction history
- [ ] Successful run of SPSA
- [ ] Investigate better NNUE architectures (two adversarial networks from scratch?)
- [ ] Data generator for NNUE training
- [ ] Clean up code, especially datatypes (too many `as` casts)
- [ ] Better time manager
- [ ] Integration with OpenBench
- [ ] Capture history
- [ ] Solve NNUE bias towards White 
- [ ] Fix missing PV lines due to cutoffs
- [ ] Store evals in TT before recursive search
- [ ] Mate Distance Pruning

## Acknowledgements

[Simbelmyne][simbelmyne] by Sam Roelants was a major source of inspiration and learning throughout this project. A lot of ideas and how to implement them in Rust came from there.

Thanks to the communities at the **Engine Programming**  Discord servers, the Chess Programming Wiki and TalkChess forum where an enormous amount of collective knowledge lives out in the open.

---

## License

This project is licensed under the [GNU General Public License v3.0][license-link].

<!-- Badges -->
[license-badge]: https://img.shields.io/github/license/WGCodings/Pea?style=for-the-badge&color=blue
[license-link]: https://github.com/WGCodings/Pea/blob/main/LICENSE

[release-badge]: https://img.shields.io/github/v/release/WGCodings/Pea?style=for-the-badge&color=violet
[release-link]: https://github.com/WGCodings/Pea/releases/latest

[lichess-badge]: https://img.shields.io/badge/Play-latest-green?logo=lichess&style=for-the-badge
[lichess-link]: https://lichess.org/@/MCPeaSearch

<!-- Links -->
[shakmaty]: https://github.com/niklasf/shakmaty
[simbelmyne]: https://github.com/sroelants/simbelmyne
[stash]: https://gitlab.com/mhouppin/stash-bot
[cargo]: https://doc.rust-lang.org/cargo
[arena]: http://www.playwitharena.de
[cutechess]: https://cutechess.com
[shredder]: https://www.shredderchess.com
[bullet]: https://github.com/jw1912/bullet