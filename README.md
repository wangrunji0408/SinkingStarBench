# Sinking Star Bench

[中文版](README-CN.md) | English

An LLM coding-agent benchmark built on *Order of the Sinking Star*, a sokoban-style puzzle game. The agent gets one binary and an empty directory, and must beat it **without reverse-engineering the binary** — learning every rule by feeding routes to the game's own `replay` command and reading back the public state.

The benchmark has two versions, kept side by side:

| Version | Challenge | Runs | README |
|---|---|---|---|
| **v1** | 12 fixed levels, 3 hours | 14 models / 6 agents | [`v1/README.md`](v1/README.md) |
| **v2** | Overworld + 90+ levels, 3-hour window | 6 models / 4 agents | this page |

---

## v2 (2026-11)

v2 replaces the fixed 12-level set with the full game: an **overworld map** that gates levels behind keys, doors, freezers and elevators, plus a per-level `target_level_key`. The agent starts from `intro`, must reach the overworld, collect keys by entering each level and pressing `V`, and then solve as many levels as it can. Score is **levels solved first, total frames second**.

Everything runs on the same binary (`sinking-star-challenge`, built from [`v2/`](v2/README.md)); harnesses differ only in how the agent is driven.

### Results

| # | Model | Harness | Effort | Solved | Frames | Time | Map entries | Tokens in / out |
|:-:|-------|:-------:|:------:|:------:|-------:|:----:|:-----------:|-----------------|
| 🥇 | **GPT-6.2 Astra** | Codex | High | **93** | 65,617 | 180 min | 98 | 75.2M / 0.30M |
| 🥈 | **Claude Fable 5.2** | Claude Code | High | **69** | 36,879 | 180 min | 64 | 199.9M / 0.78M |
| 🥉 | **GPT-6.2 Sol** | Codex | High | **61** | 35,241 | 179 min | 87 | 100.1M / 0.47M |
| 4 | **Claude Opus 5.5** | Claude Code | High | 22 | 11,203 | 193 min | 22 | 96.3M / 0.28M |
| 5 | **Gemini 4 Carbon** | pi | High | 21 | 10,589 | 180 min | 22 | 68.5M / 0.33M |
| 6 | **DeepSeek V4.1 Flash** | DSH | High | 18 | 9,705 | 168 min | 17 | 301.5M / 1.23M |

`Tokens in` includes prompt tokens and cache reads; harnesses report usage differently, so treat the column as an order-of-magnitude signal, not a precise comparison.

### What separates the runs

**The map, not the puzzles, is the ceiling.** Map entries are a hard upper bound on the score: the runs that stopped at 17 / 22 entries topped out at 18 / 21–22 levels, while the 98-entry run reached 93. Entry count tracks the score far better than any measure of per-level skill.

**Mechanic discovery is the unlock, not the search.** Every breakthrough in the logs lines up with a newly understood rule, and the solve follows within minutes:

- *A closed gate destroys crystals standing on it* → DeepSeek solved `heroes1_16_v3` with 78 keys four minutes later.
- *A wand pushes rocks through walls*; *two sunk mirrors are needed to reach a reflection position* → Astra cleared `right_hand_man` and `mirror_6_alt` immediately after.
- *Sight can bounce off a second mirror*; *a plain mirror bends sight but does not teleport* → Fable solved `mirror_10_alt2` (459 frames) and `mundane_mirror` (1,007 frames) in one pass.
- Opus 5.5's own diagnosis when its abstract model stalled while the real simulator kept exploring: *"my model has 2,300 states, the real one has 4,100 — I'm missing a mechanic."* It was right; adding the rule ended a 70-minute block.

The converse also holds: Fable's longest plateau (21 minutes, 08:07–08:28) was not solving but **map modelling** — patching its simulator for `Elevator_Platform`/`Lily` entities and probing stand positions with `T0…T300`.

**Frame efficiency is a separate, later game.** Astra and Fable both spent much of their final hour compressing routes instead of finding new levels. Opus found that turns are slower than straight moves and added a turn penalty to its shortest-path search (−1–4% per level); it also cut `intro` to 2,158 frames by noticing that reading a sign only requires facing it, so the `T120` waits Astra and Sol kept were pure waste.

### Reproducing

```sh
cd v2
cargo build --locked --profile challenge --bin sinking-star-challenge
cp target/x86_64-apple-darwin/challenge/sinking-star-challenge .
```

macOS on Apple Silicon (Rosetta 2), with the Windows Steam build of the game available at build time. The resulting binary needs no game install to run.

Scoring, key handling and replay format are documented in [`v2/README.md`](v2/README.md).

### Caveats

- Runs used two builds of the challenge binary: v2's overworld key mechanism is newer than the one used by the first four runs. Cross-run comparisons should be read with that in mind.
- Codex and `claude -p` are single-turn harnesses and stop when the model decides it is done; DSH was launched through a goal loop that kept re-prompting. Time used therefore reflects harness behaviour as much as model behaviour.
- Per-level timestamps come from APFS birth times of the submitted route files. Rewritten routes keep their birth time, so frame compression does not inflate later progress.
