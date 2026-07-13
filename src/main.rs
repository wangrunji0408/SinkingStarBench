use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use sinking_star::{Action, Direction, Game, Level, embedded_levels, parse_actions, parse_levels};

#[derive(Debug, Parser)]
#[command(name = "sinking-star", version, about = "Order of the Sinking Star — level CLI")]
struct Cli {
    /// Use an external level collection file instead of embedded levels
    #[arg(long, global = true, value_name = "FILE")]
    levels: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Play interactively; choose a level when LEVEL is omitted
    Play {
        level: Option<String>,

        /// Save the action sequence to a file after clearing the level
        #[arg(long, value_name = "FILE")]
        save: Option<PathBuf>,
    },

    /// Execute an action sequence from stdin and print the final state
    Run { level: String },

    /// Show the initial state of a level
    Show { level: String },

    /// List all levels
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let levels = load_levels(cli.levels.as_deref())?;

    match cli.command {
        Command::Play { level, save } => play(&levels, level.as_deref(), save.as_deref()),
        Command::Run { level } => run(&levels, &level),
        Command::Show { level } => show(&levels, &level),
        Command::List => list(&levels),
    }
}

fn load_levels(external: Option<&Path>) -> Result<Vec<Level>> {
    if let Some(path) = external {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read level file {}", path.display()))?;
        return parse_levels(&source)
            .with_context(|| format!("failed to parse level file {}", path.display()));
    }
    embedded_levels().context("failed to parse embedded levels")
}

fn find_level<'a>(levels: &'a [Level], name: &str) -> Result<&'a Level> {
    levels
        .iter()
        .find(|level| level.name == name)
        .ok_or_else(|| {
            let available = levels
                .iter()
                .map(|level| level.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow!("no level {name:?}; available: {available}")
        })
}

fn run(levels: &[Level], name: &str) -> Result<()> {
    let level = find_level(levels, name)?;
    let mut inputs = String::new();
    io::stdin()
        .read_to_string(&mut inputs)
        .context("failed to read actions from stdin")?;
    let actions = parse_actions(&inputs)?;
    let mut game = Game::new(level);
    for action in actions {
        game.apply(action);
    }
    println!("{}", game.describe());
    Ok(())
}

fn show(levels: &[Level], name: &str) -> Result<()> {
    let level = find_level(levels, name)?;
    println!("{}", Game::new(level).describe());
    Ok(())
}

fn list(levels: &[Level]) -> Result<()> {
    for level in levels {
        println!("{}", level.name);
    }
    Ok(())
}

fn play(levels: &[Level], requested: Option<&str>, save: Option<&Path>) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("play requires an interactive terminal; use run for batch execution")
    }
    let level = match requested {
        Some(name) => find_level(levels, name)?,
        None => choose_level(levels)?,
    };
    let mut game = Game::new(level);
    {
        let _terminal = TerminalGuard::enter()?;
        loop {
            draw(&game)?;
            if game.won() {
                break;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if is_quit(key) {
                return Ok(());
            }
            if let Some(action) = key_to_action(key) {
                game.apply(action);
            }
        }
    }

    println!("Cleared! Action sequence: {}", game.action_sequence());
    if let Some(path) = save {
        save_solution(path, &game.action_sequence())?;
        println!("Saved to {}", path.display());
    } else {
        prompt_save(level, &game.action_sequence())?;
    }
    Ok(())
}

fn choose_level(levels: &[Level]) -> Result<&Level> {
    println!("Available levels:");
    for level in levels {
        println!("{}", level.name);
    }
    print!("Enter level name: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    find_level(levels, input)
}

fn draw(game: &Game<'_>) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    write!(stdout, "Order of the Sinking Star  Level {}\r\n\r\n", game.level().name)?;
    for line in game.render_bordered().lines() {
        write!(stdout, "{line}\r\n")?;
    }
    write!(stdout, "\r\n")?;
    let actor = game.selected_actor();
    write!(
        stdout,
        "Current: {}({}) @ ({}, {}){}    Doors: {}    Actions: {}\r\n",
        actor.kind.symbol(),
        actor.kind,
        actor.pos.x,
        actor.pos.y,
        if game.actor_trapped(game.selected()) {
            " [trapped]"
        } else {
            ""
        },
        if game.doors_open() { "open" } else { "closed" },
        game.action_sequence()
    )?;
    write!(
        stdout,
        "WASD/Arrows move · Z undo · R reset · C switch actor · Q quit\r\n"
    )?;
    if game.won() {
        write!(stdout, "\r\n★ Cleared!\r\n")?;
    }
    stdout.flush()?;
    Ok(())
}

fn key_to_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Up => Some(Action::Move(Direction::Up)),
        KeyCode::Down => Some(Action::Move(Direction::Down)),
        KeyCode::Left => Some(Action::Move(Direction::Left)),
        KeyCode::Right => Some(Action::Move(Direction::Right)),
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'w' => Some(Action::Move(Direction::Up)),
            's' => Some(Action::Move(Direction::Down)),
            'a' => Some(Action::Move(Direction::Left)),
            'd' => Some(Action::Move(Direction::Right)),
            'z' => Some(Action::Undo),
            'r' => Some(Action::Reset),
            'x' => Some(Action::Trigger),
            'c' => Some(Action::SwitchActor),
            _ => None,
        },
        _ => None,
    }
}

fn is_quit(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
        || matches!(key.code, KeyCode::Char('q' | 'Q'))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C')))
}

fn prompt_save(level: &Level, sequence: &str) -> Result<()> {
    print!("Save action sequence? [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if !answer.trim().eq_ignore_ascii_case("y") {
        return Ok(());
    }
    let default = PathBuf::from("solutions").join(format!("{}.txt", level.name));
    print!("File path [{}]: ", default.display());
    io::stdout().flush()?;
    let mut path = String::new();
    io::stdin().read_line(&mut path)?;
    let path = if path.trim().is_empty() {
        default
    } else {
        PathBuf::from(path.trim())
    };
    save_solution(&path, sequence)?;
    println!("Saved to {}", path.display());
    Ok(())
}

fn save_solution(path: &Path, sequence: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    fs::write(path, format!("{sequence}\n"))
        .with_context(|| format!("failed to write solution to {}", path.display()))
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
