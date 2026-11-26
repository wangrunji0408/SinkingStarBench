# Sinking Star Bench

[中文版](README-CN.md) | English

An LLM coding-agent benchmark built on *[Order of the Sinking Star](https://www.orderofthesinkingstar.com/)*, a sokoban-style puzzle game. The agent gets one binary and an empty directory and must beat it **without reverse-engineering the binary** — every rule has to be inferred from the public state returned by the game's own `replay` command.

The benchmark uses the full game: an overworld map gating 90+ levels behind keys, doors, freezers and elevators.

## The challenge

Agents start from `intro`, break into the overworld, collect level keys by entering each level and pressing `V`, then solve as many levels as they can within a **3-hour window**. Ranking is **levels solved first, total frames second**.

The binary ships with **900+ levels** — the full game has more than 1,000 hand-crafted puzzles, and every level reachable from the overworld is playable in the challenge. For scale: the best run in the table below solved 93 of them.

All runs use the same binary, built from the sources in [`v2/`](v2/README.md); only the harness differs. The binary is self-contained at runtime — no game install needed.

## Results

| # | Model | Harness | Thinking | Solved | Frames | Time | Map entries | Cost |
|:-:|-------|:-------:|:--------:|:------:|-------:|:----:|:-----------:|-----:|
| 🥇 | **GPT-6.2 Astra** | Codex | High | **93** | 65,617 | 180 min | 98 | $45.85 |
| 🥈 | **Claude Fable 5.2** | Claude Code | High | **69** | 36,879 | 180 min | 64 | $109.29 |
| 🥉 | **GPT-6.2 Sol** | Codex | High | **61** | 35,241 | 179 min | 87 | $24.04 |
| 4 | **Claude Opus 5.5** | Claude Code | High | 22 | 11,203 | 193 min | 22 | $51.61 |
| 5 | **Gemini 4 Carbon** | pi | High | 21 | 10,589 | 180 min | 22 | $52.96 |
| 6 | **DeepSeek V4.1 Flash** | DSH | High | 18 | 9,705 | 168 min | 17 | $1.75 |

Cost is API-equivalent at catalog prices, not subscription spend; long-context tiers are applied per request.

![Progress](v2/figures/progress.svg)

## How it works

The replay tool does not reimplement the game. It loads the original Windows `sinking_star.exe` into a private x86-64 address space (fixed segments for world, heap and data, asserted at startup), and drives the **game's own code**: it writes keystrokes into the game's input queue, calls the original per-frame functions (`run_phase1`/`run_phase2`, `run_prepare`, `run_schedule`, `overworld::advance`), and reads state straight out of the engine's memory.

That is what makes the benchmark trustworthy: every rule — push order, mirror reflection, gate and freezer interactions — is whatever the shipped game does, not a model of it. Rendering is optional and only used for video export (through D3DMetal); replay and scoring need no game install.

The only thing that is new is the interface: an ASCII key string in, and one JSON line out (`frames`, `won`, `visited`, position, per-entity fields, terrain runs). That is enough for an agent to probe mechanics with tiny experiments, which is exactly how the models in the table above learned the rules.

## How each model approached it

**GPT-6.2 Astra — map first, then bespoke solvers.** It spent the early hours extending the overworld route rather than polishing levels, ending with 98 entries, and wrote a dedicated search model per level family. Most of its breakthroughs came from targeted physics experiments (pushing a rock through a wall with the wand, sinking two mirrors to reach a reflection cell) followed by immediate solves. Its final hour went into frame compression rather than new levels.

**Claude Fable 5.2 — build the simulator, then widen the map.** It wrote its own state simulator and overworld model early, then kept patching entity handling (`Elevator_Platform`, `Lily`) as it explored. Its longest stall (21 minutes) was map modelling, not puzzle solving; once a rule was fixed, levels fell in batches (38 → 64 entries in 20 minutes, 32 → 56 levels). Best frame efficiency among the top three (534 frames/level) and the most expensive run per level.

**GPT-6.2 Sol — abstract hard, solve carefully.** It built 2D abstractions and A* search per level, validated each candidate against the real replay, and generally produced cleaner routes than Astra at the cost of speed: with 87 entries it still only solved 61 levels. The most cost-efficient of the frontier runs ($0.39/level), but it under-used its map.

**Claude Opus 5.5 — one generic solver, no per-level engineering.** Its toolkit was just map rendering plus a plain Sokoban BFS; it auto-collected keys by walking the map. That produced the best frames-per-level in the field (509) and the shortest `intro` (2,158 frames, by noticing signs only need to be faced, not waited on), but it only reached 22 map entries and spent 70 minutes blocked on one level because its abstract model was missing a mechanic — a limitation it diagnosed itself.

**Gemini 4 Carbon — tools first, batch second.** It invested in scaffolding before touching levels, then solved in batches. Solid per-level quality (504 frames/level) but the smallest map alongside Opus, and the most expensive per level ($2.52).

**DeepSeek V4.1 Flash — custom fast simulator, slow start.** It built a deterministic simulator running ~14 state transitions per second and ran BFS on top of it. The intro cost it over half an hour and the map stayed at 17 entries, but per-level quality was competitive (539) and the run was two orders of magnitude cheaper than the frontier models ($0.10/level).

## Reproducing

```sh
cd v2
cargo build --locked --profile challenge --bin sinking-star-challenge
cp target/x86_64-apple-darwin/challenge/sinking-star-challenge .
```

macOS on Apple Silicon (Rosetta 2); the Windows Steam build of the game is required at build time only. Scoring, key handling and replay format are documented in [`v2/README.md`](v2/README.md).

## Notes

- Runs used two builds of the challenge binary: the overworld key mechanism is newer than the build used by the first four runs.
- Codex and `claude -p` are single-turn harnesses that stop when the model decides it is done; DSH was driven by a goal loop. Time used reflects harness behaviour as much as model behaviour.
- Per-level timestamps come from file creation times, so frame rewrites are not counted as new progress.

## Previous versions

An earlier build of this benchmark used **12 hand-crafted hero levels** instead of the full game. Agents were given a CLI that printed each level as a character map, and had to infer the rules from that text representation alone; 14 model runs are recorded there. See [`v1/`](v1/README.md) ([中文](v1/README-CN.md)).
