# Sinking Star Bench

[中文版](README-CN.md) | English

An LLM coding agent benchmark based on *Order of the Sinking Star*, a sokoban-style puzzle game. The task: **"Beat this game. Reverse-engineering the binary is forbidden."** — infer the rules from the game's own CLI and solve all 12 levels.

## Results

| # | Model | Provider | Solved | Effort | Time | Tokens | Context | Tool Calls | Cost |
|---|-------|----------|:------:|:------:|------|--------|:-------:|:----------:|-----:|
| 🏅 | **GPT-6 Astra** | Codex | **12/12** | High | 3 min | 0.46M | 31K | 18 | $1.02 |
| 🥈 | **DeepSeek v4.1 Flash** | DSH | **12/12** | High | 8 min | 5.8M | 117K | 86 | $0.07 |
| 🥉 | **Claude Fable 5.1** | CC | **12/12** | High | 11 min | 1.49M | 70K | 30 | $2.94 |
| 4 | **Claude Opus 5** | CC | **12/12** | High | 16 min | 3.7M | 79K | 63 | $3.85 |
| 5 | **DeepSeek v4 Pro** | DSH | **12/12** | High | 27 min | 3.8M | 98K | 81 | $0.08 |
| 6 | **GPT-5.6 Sol** | Codex | **12/12** | High | 40 min | 5.5M | 147K | 34 | $17.19 |
| 7 | **DeepSeek v4 Flash** | CC | **12/12** | Max | 117 min | 49.3M | 509K | 195 | $0.30 |
| 8 | **Kimi K3** | CC | **12/12** | High | 163 min | 10.2M | 274K | 74 | $7.28 |
| 9 | **DeepSeek v4 Pro Preview** | CC | 9/12 | Max | 60 min | 26.6M | 234K | 176 | $0.41 |
| 10 | **Claude Fable 5** | CC | N/A | N/A | — | — | — | — | Refused |

Harness: CC = Claude Code, DSH = DeepSeek Harness.

### Key Observations

- **GPT-6 Astra** — the fastest 12/12 and the most token-efficient: pure black-box, zero web searches, no binary introspection. Probed the public CLI, inferred the Warrior/Thief/Wizard mechanics from gameplay alone, wrote a solver, and verified every level through the official `run` interface (`Won yes`).
- **DeepSeek v4.1 Flash** — pure black-box on DeepSeek Harness with the **standard agent preset** (write/edit/read available): zero web searches, no binary introspection. Rebuilt the game as a simulator from CLI probing, validated it by differential fuzzing (~3,900 action sequences, 0 mismatches), then BFS-solved all 12 and confirmed each through the official `run` interface.
- **Claude Fable 5.1** — pure black-box: rebuilt the game as a simulator from CLI observations, validated it by differential fuzzing (0 mismatches across all 12 levels), BFS-solved every level and confirmed each via `run` (`Won yes` ×12).
- **Claude Opus 5** — pure black-box, no disassembly: built a simulator via differential fuzzing (1,440 random game sequences) against the binary until zero divergence, then a BFS solver.
- **DeepSeek v4 Pro** — pure black-box on DeepSeek Harness with the **minimal agent preset** (only the `bash` tool available): zero web searches, mechanics inferred purely from gameplay, solver written and all 12 levels verified PASS.
- **GPT-5.6 Sol** — solved all 12 levels via pure gameplay experimentation, no binary introspection; slower and costlier than Opus 5's black-box approach.
- **DeepSeek v4 Flash** — **not a pure black-box run**: web searches surfaced *Heroes of Sokoban* walkthroughs that handed it the Thief-pull and Wizard-swap mechanics directly (the two games share those mechanics). The binary was never reverse-engineered, but the core mechanics came from external search rather than gameplay observation. It still built a pty-driven play harness, simulator and BFS solver through trial and error.
- **Kimi K3** — black-box: figured out all three character mechanics (Warrior chain-push, Thief ranged-pull, Wizard position-swap) purely through gameplay observation, then solved all levels manually and programmatically.
- **DeepSeek v4 Pro Preview** — brute-forced with trial-and-error, inferring rules purely from experiments. Solved 9/12 but never cracked the 3-button door puzzles (1-4/2-4/3-4).
- **Claude Fable 5** — refused to execute the task.

## Game

A sokoban-like CLI puzzle with three character classes (Warrior, Thief, Wizard), push/pull/swap mechanics, and switch/door interactions across 12 levels.

See [`levels/README.md`](levels/README.md) for full rules.

## License

MIT
