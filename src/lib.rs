use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write as _};

use serde::Serialize;
use thiserror::Error;

include!(concat!(env!("OUT_DIR"), "/embedded_levels.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

impl Pos {
    fn step(self, direction: Direction) -> Option<Self> {
        let (dx, dy) = direction.delta();
        Some(Self {
            x: self.x.checked_add_signed(dx)?,
            y: self.y.checked_add_signed(dy)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    Warrior,
    Thief,
    Wizard,
}

impl ActorKind {
    pub fn symbol(self) -> char {
        match self {
            Self::Warrior => 'A',
            Self::Thief => 'B',
            Self::Wizard => 'C',
        }
    }
}

impl fmt::Display for ActorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Warrior => "战士",
            Self::Thief => "盗贼",
            Self::Wizard => "巫师",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    Switch,
    Door,
    Goal,
}

impl Tile {
    fn symbol(self) -> char {
        match self {
            Self::Floor => '-',
            Self::Wall => '#',
            Self::Switch => '_',
            Self::Door => '|',
            Self::Goal => '.',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Actor {
    pub kind: ActorKind,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct Level {
    pub name: String,
    pub width: usize,
    pub height: usize,
    tiles: Vec<Tile>,
    initial_actors: Vec<Actor>,
    initial_stones: HashSet<Pos>,
}

impl Level {
    pub fn actors(&self) -> &[Actor] {
        &self.initial_actors
    }

    pub fn stone_count(&self) -> usize {
        self.initial_stones.len()
    }

    pub fn goal_count(&self) -> usize {
        self.tiles
            .iter()
            .filter(|&&tile| tile == Tile::Goal)
            .count()
    }

    pub fn switch_count(&self) -> usize {
        self.tiles
            .iter()
            .filter(|&&tile| tile == Tile::Switch)
            .count()
    }

    fn tile(&self, pos: Pos) -> Tile {
        if pos.x >= self.width || pos.y >= self.height {
            Tile::Wall
        } else {
            self.tiles[pos.y * self.width + pos.x]
        }
    }
}

#[derive(Debug, Error)]
pub enum LevelError {
    #[error("未找到任何关卡")]
    NoLevels,
    #[error("第 {line} 行的关卡名重复：{name}")]
    DuplicateName { line: usize, name: String },
    #[error("关卡 {name} 没有地图")]
    EmptyMap { name: String },
    #[error("关卡 {name} 第 {line} 行宽度为 {actual}，应为 {expected}")]
    UnevenWidth {
        name: String,
        line: usize,
        actual: usize,
        expected: usize,
    },
    #[error("关卡 {name} 第 {line} 行包含未知字符 {character:?}")]
    InvalidTile {
        name: String,
        line: usize,
        character: char,
    },
    #[error("关卡 {name} 中没有角色")]
    NoActors { name: String },
}

pub fn parse_levels(input: &str) -> Result<Vec<Level>, LevelError> {
    let lines: Vec<&str> = input.lines().collect();
    let mut starts = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if is_level_name(trimmed) {
            starts.push((index, trimmed.to_owned()));
        }
    }
    if starts.is_empty() {
        return Err(LevelError::NoLevels);
    }

    let mut names = HashSet::new();
    let mut levels = Vec::with_capacity(starts.len());
    for (start_index, (header, name)) in starts.iter().enumerate() {
        if !names.insert(name.clone()) {
            return Err(LevelError::DuplicateName {
                line: header + 1,
                name: name.clone(),
            });
        }
        let end = starts
            .get(start_index + 1)
            .map_or(lines.len(), |(index, _)| *index);
        let rows: Vec<(usize, &str)> = lines[header + 1..end]
            .iter()
            .enumerate()
            .filter_map(|(offset, row)| {
                let row = row.trim_end_matches('\r');
                (!row.trim().is_empty()).then_some((header + 2 + offset, row))
            })
            .collect();
        if rows.is_empty() {
            return Err(LevelError::EmptyMap { name: name.clone() });
        }
        let width = rows[0].1.chars().count();
        let height = rows.len();
        let mut tiles = Vec::with_capacity(width * height);
        let mut actors = Vec::new();
        let mut stones = HashSet::new();
        for (y, (line_number, row)) in rows.into_iter().enumerate() {
            let actual = row.chars().count();
            if actual != width {
                return Err(LevelError::UnevenWidth {
                    name: name.clone(),
                    line: line_number,
                    actual,
                    expected: width,
                });
            }
            for (x, character) in row.chars().enumerate() {
                let pos = Pos { x, y };
                let tile = match character {
                    '-' => Tile::Floor,
                    '#' => Tile::Wall,
                    '_' => Tile::Switch,
                    '|' => Tile::Door,
                    '.' => Tile::Goal,
                    '$' => {
                        stones.insert(pos);
                        Tile::Floor
                    }
                    'A' | 'B' | 'C' => {
                        actors.push(Actor {
                            kind: match character {
                                'A' => ActorKind::Warrior,
                                'B' => ActorKind::Thief,
                                'C' => ActorKind::Wizard,
                                _ => unreachable!(),
                            },
                            pos,
                        });
                        Tile::Floor
                    }
                    _ => {
                        return Err(LevelError::InvalidTile {
                            name: name.clone(),
                            line: line_number,
                            character,
                        });
                    }
                };
                tiles.push(tile);
            }
        }
        if actors.is_empty() {
            return Err(LevelError::NoActors { name: name.clone() });
        }
        levels.push(Level {
            name: name.clone(),
            width,
            height,
            tiles,
            initial_actors: actors,
            initial_stones: stones,
        });
    }
    Ok(levels)
}

/// 读取编译时从 `levels/*.txt` 嵌入可执行文件的全部关卡。
pub fn embedded_levels() -> Result<Vec<Level>, LevelError> {
    let mut combined = String::new();
    for (name, map) in EMBEDDED_LEVEL_SOURCES {
        writeln!(combined, "{name}").expect("writing to String cannot fail");
        combined.push_str(map);
        if !map.ends_with('\n') {
            combined.push('\n');
        }
        combined.push('\n');
    }
    parse_levels(&combined)
}

fn is_level_name(value: &str) -> bool {
    let Some((left, right)) = value.split_once('-') else {
        return false;
    };
    !left.is_empty()
        && !right.is_empty()
        && left.chars().all(|c| c.is_ascii_digit())
        && right.chars().all(|c| c.is_ascii_digit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn delta(self) -> (isize, isize) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move(Direction),
    Undo,
    Reset,
    Trigger,
    SwitchActor,
}

impl Action {
    pub fn symbol(self) -> char {
        match self {
            Self::Move(Direction::Up) => 'W',
            Self::Move(Direction::Down) => 'S',
            Self::Move(Direction::Left) => 'A',
            Self::Move(Direction::Right) => 'D',
            Self::Undo => 'Z',
            Self::Reset => 'R',
            Self::Trigger => 'X',
            Self::SwitchActor => 'C',
        }
    }
}

#[derive(Debug, Error)]
#[error("输入序列第 {index} 个字符 {character:?} 无效；可用 WASD、方向箭头、Z、R、X、C")]
pub struct ActionParseError {
    pub index: usize,
    pub character: char,
}

pub fn parse_actions(input: &str) -> Result<Vec<Action>, ActionParseError> {
    let mut actions = Vec::new();
    for (index, character) in input.chars().enumerate() {
        let action = match character {
            'w' | 'W' | '↑' => Action::Move(Direction::Up),
            's' | 'S' | '↓' => Action::Move(Direction::Down),
            'a' | 'A' | '←' => Action::Move(Direction::Left),
            'd' | 'D' | '→' => Action::Move(Direction::Right),
            'z' | 'Z' => Action::Undo,
            'r' | 'R' => Action::Reset,
            'x' | 'X' => Action::Trigger,
            'c' | 'C' => Action::SwitchActor,
            c if c.is_whitespace() || c == ',' => continue,
            _ => {
                return Err(ActionParseError {
                    index: index + 1,
                    character,
                });
            }
        };
        actions.push(action);
    }
    Ok(actions)
}

#[derive(Debug, Clone)]
struct Snapshot {
    actors: Vec<Actor>,
    stones: HashSet<Pos>,
    selected: usize,
    action_len: usize,
}

#[derive(Debug, Clone)]
pub struct Game<'a> {
    level: &'a Level,
    actors: Vec<Actor>,
    stones: HashSet<Pos>,
    selected: usize,
    history: Vec<Snapshot>,
    actions: Vec<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionResult {
    Moved,
    Undone,
    Reset,
    ActorSwitched,
    NoEffect,
}

impl ActionResult {
    pub fn changed(self) -> bool {
        !matches!(self, Self::NoEffect)
    }
}

impl<'a> Game<'a> {
    pub fn new(level: &'a Level) -> Self {
        Self {
            level,
            actors: level.initial_actors.clone(),
            stones: level.initial_stones.clone(),
            selected: 0,
            history: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn level(&self) -> &Level {
        self.level
    }

    pub fn actors(&self) -> &[Actor] {
        &self.actors
    }

    pub fn stones(&self) -> &HashSet<Pos> {
        &self.stones
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_actor(&self) -> &Actor {
        &self.actors[self.selected]
    }

    pub fn action_sequence(&self) -> String {
        self.actions.iter().collect()
    }

    pub fn doors_open(&self) -> bool {
        let mut switches = self
            .level
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| **tile == Tile::Switch)
            .map(|(index, _)| Pos {
                x: index % self.level.width,
                y: index / self.level.width,
            })
            .peekable();
        switches.peek().is_some() && switches.all(|pos| self.object_at(pos).is_some())
    }

    pub fn won(&self) -> bool {
        let goals: HashSet<Pos> = self
            .level
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| **tile == Tile::Goal)
            .map(|(index, _)| Pos {
                x: index % self.level.width,
                y: index / self.level.width,
            })
            .collect();
        let actor_positions: HashSet<Pos> = self.actors.iter().map(|actor| actor.pos).collect();
        !goals.is_empty() && goals == actor_positions
    }

    pub fn apply(&mut self, action: Action) -> ActionResult {
        match action {
            Action::Undo => self.undo(),
            Action::Reset => self.reset(),
            Action::SwitchActor => self.switch_actor(),
            Action::Trigger => ActionResult::NoEffect,
            Action::Move(direction) => self.move_selected(direction),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            actors: self.actors.clone(),
            stones: self.stones.clone(),
            selected: self.selected,
            action_len: self.actions.len(),
        }
    }

    fn commit(&mut self, before: Snapshot, action: Action) {
        self.history.push(before);
        self.actions.push(action.symbol());
    }

    fn undo(&mut self) -> ActionResult {
        let Some(snapshot) = self.history.pop() else {
            return ActionResult::NoEffect;
        };
        self.actors = snapshot.actors;
        self.stones = snapshot.stones;
        self.selected = snapshot.selected;
        self.actions.truncate(snapshot.action_len);
        ActionResult::Undone
    }

    fn reset(&mut self) -> ActionResult {
        if self.actors == self.level.initial_actors
            && self.stones == self.level.initial_stones
            && self.selected == 0
        {
            return ActionResult::NoEffect;
        }
        let before = self.snapshot();
        self.actors.clone_from(&self.level.initial_actors);
        self.stones.clone_from(&self.level.initial_stones);
        self.selected = 0;
        self.commit(before, Action::Reset);
        ActionResult::Reset
    }

    fn switch_actor(&mut self) -> ActionResult {
        if self.actors.len() < 2 {
            return ActionResult::NoEffect;
        }
        let before = self.snapshot();
        self.selected = (self.selected + 1) % self.actors.len();
        self.commit(before, Action::SwitchActor);
        ActionResult::ActorSwitched
    }

    fn move_selected(&mut self, direction: Direction) -> ActionResult {
        if self.actor_trapped(self.selected) {
            return ActionResult::NoEffect;
        }
        let before = self.snapshot();
        let doors_were_open = self.doors_open();
        let moved = match self.selected_actor().kind {
            ActorKind::Warrior => self.move_warrior(direction),
            ActorKind::Thief => self.move_thief(direction),
            ActorKind::Wizard => self.move_wizard(direction),
        };
        if moved {
            self.resolve_door_closure(doors_were_open);
            self.commit(before, Action::Move(direction));
            ActionResult::Moved
        } else {
            ActionResult::NoEffect
        }
    }

    fn move_warrior(&mut self, direction: Direction) -> bool {
        let Some(next) = self.selected_actor().pos.step(direction) else {
            return false;
        };
        if !self.passable(next) {
            return false;
        }
        if self.object_at(next).is_none() {
            self.actors[self.selected].pos = next;
            return true;
        }

        let mut chain = vec![next];
        let mut cursor = next;
        loop {
            let Some(after) = cursor.step(direction) else {
                return false;
            };
            if !self.passable(after) {
                return false;
            }
            if self.object_at(after).is_none() {
                for &pos in chain.iter().rev() {
                    self.move_object(pos, pos.step(direction).expect("checked above"));
                }
                self.actors[self.selected].pos = next;
                return true;
            }
            chain.push(after);
            cursor = after;
        }
    }

    fn move_thief(&mut self, direction: Direction) -> bool {
        let old = self.selected_actor().pos;
        let Some(next) = old.step(direction) else {
            return false;
        };
        if !self.passable(next) || self.object_at(next).is_some() {
            return false;
        }
        let behind = old.step(direction.opposite());
        self.actors[self.selected].pos = next;
        if let Some(behind) = behind
            && self.object_at(behind).is_some()
        {
            self.move_object(behind, old);
        }
        true
    }

    fn move_wizard(&mut self, direction: Direction) -> bool {
        let old = self.selected_actor().pos;
        let mut cursor = old;
        while let Some(next) = cursor.step(direction) {
            if !self.passable(next) {
                break;
            }
            if self.object_at(next).is_some() {
                self.move_object(next, old);
                self.actors[self.selected].pos = next;
                return true;
            }
            cursor = next;
        }

        let Some(next) = old.step(direction) else {
            return false;
        };
        if self.passable(next) && self.object_at(next).is_none() {
            self.actors[self.selected].pos = next;
            true
        } else {
            false
        }
    }

    fn object_at(&self, pos: Pos) -> Option<Object> {
        if self.stones.contains(&pos) {
            Some(Object::Stone)
        } else {
            self.actors
                .iter()
                .position(|actor| actor.pos == pos)
                .map(Object::Actor)
        }
    }

    fn move_object(&mut self, from: Pos, to: Pos) {
        match self.object_at(from).expect("source contains an object") {
            Object::Stone => {
                self.stones.remove(&from);
                self.stones.insert(to);
            }
            Object::Actor(index) => self.actors[index].pos = to,
        }
    }

    fn passable(&self, pos: Pos) -> bool {
        match self.level.tile(pos) {
            Tile::Wall => false,
            Tile::Door => self.doors_open(),
            Tile::Floor | Tile::Switch | Tile::Goal => true,
        }
    }

    /// 角色位于已经关闭的门上时被卡住，不能主动移动。
    pub fn actor_trapped(&self, index: usize) -> bool {
        self.actors
            .get(index)
            .is_some_and(|actor| self.level.tile(actor.pos) == Tile::Door && !self.doors_open())
    }

    fn resolve_door_closure(&mut self, doors_were_open: bool) {
        if !doors_were_open || self.doors_open() {
            return;
        }
        let crushed: Vec<Pos> = self
            .stones
            .iter()
            .copied()
            .filter(|&pos| self.level.tile(pos) == Tile::Door)
            .collect();
        for pos in crushed {
            self.stones.remove(&pos);
        }
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        for y in 0..self.level.height {
            for x in 0..self.level.width {
                let pos = Pos { x, y };
                let character = if let Some(actor) = self.actors.iter().find(|a| a.pos == pos) {
                    actor.kind.symbol()
                } else if self.stones.contains(&pos) {
                    '$'
                } else {
                    self.level.tile(pos).symbol()
                };
                output.push(character);
            }
            if y + 1 < self.level.height {
                output.push('\n');
            }
        }
        output
    }

    pub fn report(&self) -> GameReport {
        let mut stones: Vec<Pos> = self.stones.iter().copied().collect();
        stones.sort_by_key(|pos| (pos.y, pos.x));
        GameReport {
            level: self.level.name.clone(),
            grid: self.render().lines().map(str::to_owned).collect(),
            actors: self.actors.clone(),
            stones,
            selected_actor: self.selected,
            trapped_actors: (0..self.actors.len())
                .filter(|&index| self.actor_trapped(index))
                .collect(),
            doors_open: self.doors_open(),
            won: self.won(),
            actions: self.action_sequence(),
        }
    }

    pub fn describe(&self) -> String {
        let mut output = self.render();
        let actor = self.selected_actor();
        write!(
            output,
            "\n关卡 {}  当前角色 {}({}) @ ({}, {})  门 {}  通关 {}\n动作 {}",
            self.level.name,
            actor.kind.symbol(),
            actor.kind,
            actor.pos.x,
            actor.pos.y,
            if self.doors_open() {
                "开"
            } else if self.actor_trapped(self.selected) {
                "关（当前角色被卡住）"
            } else {
                "关"
            },
            if self.won() { "是" } else { "否" },
            self.action_sequence()
        )
        .expect("writing to String cannot fail");
        output
    }
}

#[derive(Debug, Clone, Copy)]
enum Object {
    Stone,
    Actor(usize),
}

#[derive(Debug, Serialize)]
pub struct GameReport {
    pub level: String,
    pub grid: Vec<String>,
    pub actors: Vec<Actor>,
    pub stones: Vec<Pos>,
    pub selected_actor: usize,
    pub trapped_actors: Vec<usize>,
    pub doors_open: bool,
    pub won: bool,
    pub actions: String,
}

pub fn level_map(levels: &[Level]) -> HashMap<&str, &Level> {
    levels
        .iter()
        .map(|level| (level.name.as_str(), level))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn one_level(map: &str) -> Level {
        parse_levels(&format!("1-1\n{map}\n")).unwrap().remove(0)
    }

    #[test]
    fn parses_bundled_levels() {
        let levels = embedded_levels().unwrap();
        assert_eq!(levels.len(), 12);
        assert_eq!(levels[0].name, "1-1");
        assert_eq!(levels[11].name, "3-4");
    }

    #[test]
    fn warrior_pushes_a_chain() {
        let level = one_level("A$$-.");
        let mut game = Game::new(&level);
        assert_eq!(
            game.apply(Action::Move(Direction::Right)),
            ActionResult::Moved
        );
        assert_eq!(game.render(), "-A$$.");
        assert_eq!(
            game.apply(Action::Move(Direction::Right)),
            ActionResult::Moved
        );
        assert_eq!(game.render(), "--A$$");
    }

    #[test]
    fn thief_pulls_the_object_behind() {
        let level = one_level("$B--.");
        let mut game = Game::new(&level);
        game.apply(Action::Move(Direction::Right));
        assert_eq!(game.render(), "-$B-.");
    }

    #[test]
    fn wizard_swaps_with_first_visible_object() {
        let level = one_level("C--$.");
        let mut game = Game::new(&level);
        game.apply(Action::Move(Direction::Right));
        assert_eq!(game.render(), "$--C.");
    }

    #[test]
    fn stones_and_actors_both_trigger_switches() {
        let level = one_level("A$_|.");
        let mut game = Game::new(&level);
        assert!(!game.doors_open());
        game.apply(Action::Move(Direction::Right));
        assert!(game.doors_open());
        assert_eq!(game.render(), "-A$|.");
        game.apply(Action::Move(Direction::Right));
        assert!(game.doors_open());
    }

    #[test]
    fn closing_door_crushes_stone() {
        let level = one_level("A$_|--");
        let mut game = Game::new(&level);
        game.apply(Action::Move(Direction::Right));
        game.apply(Action::Move(Direction::Right));
        assert!(game.stones().contains(&Pos { x: 3, y: 0 }));
        game.apply(Action::Move(Direction::Left));
        assert!(!game.doors_open());
        assert!(game.stones().is_empty());
        assert_eq!(game.render(), "-A_|--");
    }

    #[test]
    fn actor_caught_by_closing_door_cannot_move() {
        let level = one_level("A$_|--");
        let mut game = Game::new(&level);
        game.apply(Action::Move(Direction::Right));
        game.apply(Action::Move(Direction::Right));
        game.apply(Action::Move(Direction::Right));
        assert!(game.actor_trapped(0));
        let before = game.render();
        assert_eq!(
            game.apply(Action::Move(Direction::Right)),
            ActionResult::NoEffect
        );
        assert_eq!(game.render(), before);
        assert_eq!(game.apply(Action::Undo), ActionResult::Undone);
        assert!(!game.actor_trapped(0));
    }

    #[test]
    fn reset_can_be_undone_and_sequence_replays() {
        let level = one_level("A--.");
        let mut game = Game::new(&level);
        game.apply(Action::Move(Direction::Right));
        game.apply(Action::Reset);
        assert_eq!(game.action_sequence(), "DR");
        assert_eq!(game.apply(Action::Undo), ActionResult::Undone);
        assert_eq!(game.render(), "-A-.");
        assert_eq!(game.action_sequence(), "D");
    }

    #[test]
    fn actor_on_only_goal_wins() {
        let level = one_level("A.");
        let mut game = Game::new(&level);
        game.apply(Action::Move(Direction::Right));
        assert!(game.won());
    }
}
