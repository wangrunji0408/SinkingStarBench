# Level files

Each `.txt` file is one map; the file name is the level name. The build script scans and embeds all `.txt` files in numeric order.

Legend:

- ` ` (space) floor
- `#` wall
- `$` stone
- `_` switch
- `|` door
- `.` goal
- `A` warrior
- `B` thief
- `C` wizard

## Rules

- **Tiles**: Floor (` `), Wall (`#`), Switch (`_`), Door (`|`), Goal (`.`). Outside the map is a wall.
- **Objects**: Stones (`$`) can be pushed, pulled, or swapped. Actors have unique abilities and can also trigger switches or stand on goals.
- **Warrior** (`A`): Pushes a connected chain of objects ahead in the movement direction.
- **Thief** (`B`): Pulls only the single object immediately behind when moving forward.
- **Wizard** (`C`): Swaps places with the first object along the movement direction before a wall or closed door.
- **Doors**: Open when at least one switch exists and every switch is occupied by a stone or actor. When doors close, stones on door tiles are crushed and actors become trapped (cannot move on their own).
- **Win condition**: The set of actor positions equals the set of goal positions.

`show`, `play`, and `run` display an implicit ring of outer walls around the map.
