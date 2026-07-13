use std::fs;
use std::io::{self, IsTerminal, Write};
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
#[command(name = "sinking-star", version, about = "《沉星之序》关卡 CLI")]
struct Cli {
    /// 使用外部合集关卡文件，而非编译进程序的关卡
    #[arg(long, global = true, value_name = "FILE")]
    levels: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 实时游玩；省略 LEVEL 时先选择关卡
    Play {
        level: Option<String>,

        /// 通关后直接将动作序列保存到指定文件
        #[arg(long, value_name = "FILE")]
        save: Option<PathBuf>,
    },

    /// 对关卡一次性执行输入序列并输出最终状态
    Run {
        level: String,
        /// WASD/方向箭头移动，Z 撤销，R 重置，X 机关，C 切换角色
        inputs: String,

        /// 输出便于程序读取的 JSON
        #[arg(long)]
        json: bool,
    },

    /// 展示某个关卡的初始状态
    Show { level: String },

    /// 列出全部关卡
    List {
        /// 输出 JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let levels = load_levels(cli.levels.as_deref())?;

    match cli.command {
        Command::Play { level, save } => play(&levels, level.as_deref(), save.as_deref()),
        Command::Run {
            level,
            inputs,
            json,
        } => run(&levels, &level, &inputs, json),
        Command::Show { level } => show(&levels, &level),
        Command::List { json } => list(&levels, json),
    }
}

fn load_levels(external: Option<&Path>) -> Result<Vec<Level>> {
    if let Some(path) = external {
        let source = fs::read_to_string(path)
            .with_context(|| format!("无法读取关卡文件 {}", path.display()))?;
        return parse_levels(&source)
            .with_context(|| format!("无法解析关卡文件 {}", path.display()));
    }
    embedded_levels().context("无法解析编译进程序的关卡")
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
            anyhow!("没有关卡 {name:?}；可用关卡：{available}")
        })
}

fn run(levels: &[Level], name: &str, inputs: &str, json: bool) -> Result<()> {
    let level = find_level(levels, name)?;
    let actions = parse_actions(inputs)?;
    let mut game = Game::new(level);
    for action in actions {
        game.apply(action);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&game.report())?);
    } else {
        println!("{}", game.describe());
    }
    Ok(())
}

fn show(levels: &[Level], name: &str) -> Result<()> {
    let level = find_level(levels, name)?;
    println!("{}", Game::new(level).describe());
    Ok(())
}

fn list(levels: &[Level], json: bool) -> Result<()> {
    if json {
        let names: Vec<_> = levels.iter().map(|level| &level.name).collect();
        println!("{}", serde_json::to_string_pretty(&names)?);
    } else {
        for level in levels {
            println!("{}", level.name);
        }
    }
    Ok(())
}

fn play(levels: &[Level], requested: Option<&str>, save: Option<&Path>) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("play 需要交互式终端；自动执行请使用 run")
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

    println!("通关！动作序列：{}", game.action_sequence());
    if let Some(path) = save {
        save_solution(path, &game.action_sequence())?;
        println!("已保存到 {}", path.display());
    } else {
        prompt_save(level, &game.action_sequence())?;
    }
    Ok(())
}

fn choose_level(levels: &[Level]) -> Result<&Level> {
    println!("可用关卡：");
    for level in levels {
        println!("{}", level.name);
    }
    print!("输入关卡名：");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    find_level(levels, input)
}

fn draw(game: &Game<'_>) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    write!(stdout, "《沉星之序》  关卡 {}\r\n\r\n", game.level().name)?;
    for line in game.render_bordered().lines() {
        write!(stdout, "{line}\r\n")?;
    }
    write!(stdout, "\r\n")?;
    let actor = game.selected_actor();
    write!(
        stdout,
        "当前：{}({}) @ ({}, {}){}    门：{}    动作：{}\r\n",
        actor.kind.symbol(),
        actor.kind,
        actor.pos.x,
        actor.pos.y,
        if game.actor_trapped(game.selected()) {
            " [被卡住]"
        } else {
            ""
        },
        if game.doors_open() { "开" } else { "关" },
        game.action_sequence()
    )?;
    write!(
        stdout,
        "WASD/方向键 移动 · Z 撤销 · R 重置 · C 换人 · Q 退出\r\n"
    )?;
    if game.won() {
        write!(stdout, "\r\n★ 通关！\r\n")?;
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
    print!("保存动作序列？[y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if !answer.trim().eq_ignore_ascii_case("y") {
        return Ok(());
    }
    let default = PathBuf::from("solutions").join(format!("{}.txt", level.name));
    print!("文件路径 [{}]：", default.display());
    io::stdout().flush()?;
    let mut path = String::new();
    io::stdin().read_line(&mut path)?;
    let path = if path.trim().is_empty() {
        default
    } else {
        PathBuf::from(path.trim())
    };
    save_solution(&path, sequence)?;
    println!("已保存到 {}", path.display());
    Ok(())
}

fn save_solution(path: &Path, sequence: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| format!("无法创建目录 {}", parent.display()))?;
    }
    fs::write(path, format!("{sequence}\n"))
        .with_context(|| format!("无法保存动作序列到 {}", path.display()))
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
