# Sinking Star Bench

[中文版](README-CN.md) | English

An LLM coding agent benchmark based on *Order of the Sinking Star*, a sokoban-style puzzle game. The task: **"Beat this game"** — understand the rules from a CLI binary and solve all 12 levels.

## Results

| Model | Provider | Solved | Effort | Time | Tokens | Context | Tool Calls | Cost |
|-------|----------|:------:|:------:|------|--------|:-------:|:----------:|-----:|
| **Claude Opus 5** | Claude Code | **12/12** | High | 16 min | 3.7M | 79K | 63 | $3.85 |
| **DeepSeek v4 Pro** | DeepSeek Harness | **12/12** | High | 27 min⁴ | 3.8M | 98K | 81 | $0.08 |
| **GPT-5.6 Sol** | Codex | **12/12** | High | 40 min¹ | 5.5M | 147K | 34 | $17.19 |
| **DeepSeek v4 Flash** | Claude Code | **12/12** | Max | 117 min³ | 49.3M | 509K | 195 | $0.30 |
| **Kimi K3** | Claude Code | **12/12** | High | 163 min² | 10.2M | 274K | 74 | $7.28 |
| **DeepSeek v4 Pro Preview** | Claude Code | 9/12 | Max | 60 min | 26.6M | 234K | 176 | $0.41 |
| **Claude Fable 5** | Claude Code | N/A | N/A | — | — | — | — | Refused |

¹ Black-box only — binary reverse-engineering was explicitly disallowed. With reverse-engineering, GPT-5.6 Sol solved it in 9 min with 2.9M tokens at $7.97.

² With reverse-engineering, Kimi K3 solved it in 25 min with 2.2M tokens at $1.66.

³ DeepSeek v4 Flash is **not a pure black-box run**: 4 web searches surfaced *Heroes of Sokoban* walkthroughs, which handed it the Thief-pull and Wizard-swap mechanics directly (the game shares those mechanics). The binary was never reverse-engineered, but the core mechanics knowledge came from external search rather than gameplay observation.

⁴ Pure black-box on DeepSeek Harness (dsh, deepseek-v4-pro, reasoning effort high, **minimal agent preset**): **zero web searches**. With only the `bash` tool available, the model inferred all mechanics from gameplay, wrote a solver, and verified all 12 levels PASS.

### Key Observations

- **GPT-5.6 Sol**¹ solved all 12 levels via pure gameplay experimentation (no binary introspection). At 40 min and $17.19, it's slower and costlier than Opus 5's black-box approach, which completed in 16 min at $3.85 with far fewer tokens.
- **Claude Opus 5** matched GPT on all 12 levels using pure black-box experimentation — no disassembly. Built a simulator via differential fuzzing (1,440 random game sequences) against the binary until zero divergence, then BFS solver. At $3.85 it's the third cheapest 12/12 solver (after v4 Pro and v4 Flash).
- **Kimi K3**² solved all 12 levels in black-box mode — figuring out all three character mechanics (Warrior chain-push, Thief ranged-pull, Wizard position-swap) purely through gameplay observation, then solving all levels manually and programmatically. Took 163 min at $7.28.
- **DeepSeek v4 Flash**³ solved all 12 levels at $0.30 — the second-lowest cost — but with a major caveat: web searches surfaced *Heroes of Sokoban* walkthroughs that provided the Thief-pull and Wizard-swap mechanics directly, so it was not a pure black-box run. It still built a pty-driven play harness, simulator, and BFS solver through trial-and-error; took 117 min and 49.3M tokens (98.6% cache hit).
- **DeepSeek v4 Pro**⁴ solved all 12 levels as a pure black-box run on DeepSeek Harness with the minimal agent preset (only the `bash` tool) in 27 min at $0.08 — the lowest cost of any 12/12 solver. Zero web searches: mechanics inferred purely from gameplay, solver written and all 12 levels verified PASS. 3.8M tokens at 99.1% cache hit.
- **DeepSeek v4 Pro Preview** brute-forced with 26.6M tokens of trial-and-error. Solved 9/12 but couldn't crack the 3-button door puzzles (1-4/2-4/3-4). Despite massive token volume, it cost only $0.41 thanks to DeepSeek's ultra-low cache pricing ($0.0036/M).
- **Claude Fable 5** refused to execute the task.

## Game

A sokoban-like CLI puzzle with three character classes (Warrior, Thief, Wizard), push/pull/swap mechanics, and switch/door interactions across 12 levels.

See [`levels/README.md`](levels/README.md) for full rules.

## License

MIT
