# Sinking Star Bench

[中文版](README-CN.md) | English

An LLM coding agent benchmark based on *Order of the Sinking Star*, a sokoban-style puzzle game. The task: **"Beat this game"** — understand the rules from a CLI binary and solve all 12 levels.

## Results

| Model | Provider | Solved | Time | Tokens | Context | Tool Calls | Cost |
|-------|----------|:------:|------|--------|:-------:|:----------:|-----:|
| **GPT-5.6 Sol** | Codex | **12/12** | 9 min | 2.9M | 950K | 46 | $7.97 |
| **Claude Opus 4.8** | Claude Code | **12/12** | 36 min | 19.7M | 143K | 101 | $17.23 |
| **DeepSeek v4 Pro** | Claude Code | 9/12 | 60 min | 48.3M | 234K | 201 | $0.62 |
| **Claude Fable 5** | Claude Code | N/A | — | — | — | — | Refused |

Pricing used: GPT-5.6 Sol $5/$30 per M input/output (cached 50% off), Claude Opus 4.8 $5/$25 (cache write $6.25, cache read $0.50), DeepSeek v4 Pro $0.435/$0.87 (cache read $0.0036).

### Key Observations

- **GPT-5.6 Sol** is the clear winner: 7× fewer tokens than Opus, 17× fewer than DeepSeek. Its edge came from binary reverse-engineering (`strings`/`nm`) to deduce mechanics, then a one-shot BFS solver.
- **Claude Opus 4.8** took the most methodical approach — use the game binary as an oracle → derive rules → build a native simulator → exhaustive BFS. Most expensive ($17.23) and slowest (36 min).
- **DeepSeek v4 Pro** brute-forced with 48M tokens of trial-and-error. Solved 9/12 but couldn't crack the 3-button door puzzles (1-4/2-4/3-4). Despite massive token volume, it was the cheapest ($0.62) due to DeepSeek's ultra-low cache pricing ($0.0036/M).
- **Claude Fable 5** refused to execute, triggering automatic fallback to Opus 4.8.

## Game

A sokoban-like CLI puzzle with three character classes (Warrior, Thief, Wizard), push/pull/swap mechanics, and switch/door interactions across 12 levels.

See [`levels/README.md`](levels/README.md) for full rules.

## License

MIT
