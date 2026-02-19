# Engine Development Log


## Version v04

Formed a baseline.

Version **v04** includes alpha beta search with a simple piece only eval + mobility + tempo bonus.

I think the mobility bonus gave a +40 ELO and the tempo bonus another 15 ELO.  
Also check-extensions were a big win. It makes the search around 50% slower but gives better results.

These are the results of v04 with check extensions vs v03 (mobility+tempo) vs v02 (mobility).

---

### Results

| Rank | Name              | Elo | +/- | Games | Score  | Draw  |
|------|-------------------|-----|-----|-------|--------|-------|
| 1    | v04-checkext      | 48  | 16  | 1000  | 56.9%  | 46.6% |
| 2    | v03               | -15 | 16  | 1000  | 47.9%  | 46.1% |
| 3    | v02               | -33 | 16  | 1000  | 45.3%  | 45.7% |


---


## Version v05

Added pv move ordering and mvv-lva ordering.

Score of v04-checkext vs v05-pv:  
**386 - 138 - 476 [0.624]**

- v04-checkext playing White: 95 - 114 - 291  [0.481] 500
- v04-checkext playing Black: 291 - 24 - 185  [0.767] 500
- White vs Black: 119 - 405 - 476  [0.357] 1000

Elo difference: 88.0 +/- 15.6  
LOS: 100.0 %  
DrawRatio: 47.6 %


## Version v06

Version **v06** focused mainly on cleaning up the code and improving rule handling.

Added threefold repetition and 50 move rule because the amount of draws were getting out of hand and hard to interpret during testing.

Score of v06-repetition vs v05-betteruci:  
**175 - 132 - 193 [0.543]**

- v06-repetition playing White: 89 - 49 - 112  [0.580] 250
- v06-repetition playing Black: 86 - 83 - 81  [0.506] 250
- White vs Black: 172 - 135 - 193  [0.537] 500

Elo difference: 30.0 +/- 23.9  
LOS: 99.3 %  
DrawRatio: 38.6 %


## Version v07

Added a transposition table.

This gave roughly **+50 ELO**, but unfortunately the match logs were lost as well as the binaries for that version.

---

## Version v08

Integrated an NNUE network:

- 512 hidden units
- Single output
- Trained on ~300 million Lichess positions using the Bullet NNUE trainer.

Score of v08_nnue512 vs v07_tt:  
**308 - 69 - 27 [0.796]**

- v08_nnue512 playing White: 156 - 22 - 24  [0.832] 202
- v08_nnue512 playing Black: 152 - 47 - 3  [0.760] 202
- White vs Black: 203 - 174 - 27  [0.536] 404

Elo difference: 236.3 +/- 40.1  
LOS: 100.0 %  
DrawRatio: 6.7 %

This was a very nice upgrade. The gains are so massive because I literally had only a piece val and mobility eval.

---



## Version v09

Made NNUE fully incremental instead of reconstructing accumulators from scratch every node.

This roughly **doubled NPS**.

Score of nnue512-incremental vs v08_nnue512: 
**94 - 49 - 57 [0.388]**

Elo difference: 79.5 +/- 41.5  
LOS: 100.0 %  
DrawRatio: 28.5 %

200 of 200 games finished.



---
## Version v10

Added killer moves.

Score of v10_killer vs nnue512-incremental:  
**810 - 503 - 687 [0.577]**

Elo difference: 53.8 +/- 12.4  
LOS: 100.0 %  
DrawRatio: 34.4 %

2000 of 2000 games finished.

Killer moves gave a solid and stable gain.


---

## Version v11–v12

Added:

- History moves
- Counter moves
- Fixed bug in threefold repetition

History and counter moves did not produce measurable improvement yet — likely due to remaining search weaknesses.

Score of v12-fixedthreefold vs v10_killer:  
**667 - 673 - 660 [0.498]**

Elo difference: -1.0 +/- 12.5  
LOS: 43.5 %  
DrawRatio: 33.0 %

2000 of 2000 games finished.

Strength is essentially unchanged, but repetition handling is now correct and stable.

---

### Current Status

Engine now includes:

- Alpha-beta search
- Check extensions
- Transposition table
- NNUE (incremental)
- Killer moves
- History + Counter moves
- Correct repetition + 50 move rule handling

Search still has large room for improvement. In the 5+0.1s self play matches It can only get to around depth 6. 
