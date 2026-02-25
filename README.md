# ♟️ Fast Pea Pea

Warning this README is written by AI, expect spectacular writing.

A UCI-compatible chess engine written in **Rust**.

This engine is built as a performance-driven learning project. 
Currently using an NNUE trained on existing data found on the net. 
However when search is optimized I will start from scratch and train a NNUE purely from self play.  
I currently use the shakmaty crate for move generation and board representation. 
In the future I might switch to a self made system but for now I like to focus on the engine logic.

You can challenge me on Lichess here: https://lichess.org/@/MCPeaSearch.

---

## 🚀 Features

### 🧠 Core Engine
- Bitboard-based move generation (via `shakmaty`)
- UCI protocol compatible
- Multi-PV support
- Depth-based and time-based search
- Built-in `perft` command for validation

---

### 🔎 Search
- Iterative Deepening
- Negamax with Alpha-Beta pruning
- Quiescence Search
- PV Search
- Razoring
- Transposition table
- Efficient Move Ordering:
  - PV move priority
  - TT 
  - Killer moves
  - History moves
  - Counter moves
  - MVV-LVA capture sorting
- More to follow soon

---

### 📊 Evaluation
- NNUE (trained using the bullet crate)
  - 1536 neurons hidden layer
  - 8 output buckets
  - data from stockfish (later will switch to self play data)

---

## 🧪 Testing & Benchmarking

The engine includes:

- `perft <depth>` command
- `go depth <n>` for reproducible benchmarks
- `go movetime <time>`
- NPS reporting 
- Node count reporting
- Time measurement

---

## 🛠 Tech Stack

| Component | Technology |
|------------|------------|
| Language | Rust (stable) |
| Move Generation | `shakmaty` |
| Protocol | UCI |
| Build System | Cargo |

Performance-focused build in release mode.

---

## 📦 Building

Install Rust:

```bash
rustup update
```
```bash
git clone https://github.com/WGCodings/FastPeaPea.git
cargo build --release
```

## 📜 License

Fast Pea Pea is licensed under the [MIT license](https://opensource.org/licenses/MIT).
