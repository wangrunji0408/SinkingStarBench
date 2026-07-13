# Order of the Sinking Star CLI

Build:

```sh
cargo build --release
```

Each level in `levels/` is one map file. These `.txt` files are automatically scanned and embedded into the binary at build time — the finished binary does not need the level files at runtime.

Commands:

```sh
# Play interactively (omit level name to choose from a list)
cargo run -- play
cargo run -- play 1-1
cargo run -- play 1-1 --save solutions/1-1.txt

# Execute an action sequence from stdin (coordinates are 0-based (x, y))
cargo run --release -- run 1-1 < solutions/1-1.txt
printf 'WASDD' | cargo run --release -- run 1-1

# Show / list levels
cargo run -- show 2-3
cargo run -- list
```

Input accepts `WASD` or `↑↓←→`, `Z` undo, `R` reset, `X` trigger (no effect yet), `C` switch actor. Whitespace and commas in batch input are ignored.

## Rules

- **Tiles**: Floor (` `), Wall (`#`), Switch (`_`), Door (`|`), Goal (`.`). Outside the map is a wall.
- **Objects**: Stones (`$`) can be pushed, pulled, or swapped. Actors have unique abilities and can also trigger switches or stand on goals.
- **Warrior** (`A`): Pushes a connected chain of objects ahead in the movement direction.
- **Thief** (`B`): Pulls only the single object immediately behind when moving forward.
- **Wizard** (`C`): Swaps places with the first object along the movement direction before a wall or closed door.
- **Doors**: Open when at least one switch exists and every switch is occupied by a stone or actor. When doors close, stones on door tiles are crushed and actors become trapped (cannot move on their own).
- **Win condition**: The set of actor positions equals the set of goal positions.

`show`, `play`, and `run` display an implicit ring of outer walls around the map.
