use crate::routes::Route;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// 回放 Steam 版关卡，导出视频、截图或游戏状态。
///
/// 在二进制所在目录运行：
///   ./sinking-star list-levels
///   echo -n 'RDRRRURURRRD' | ./sinking-star replay heroes1_1
///
/// 上面的示例无需解法文件，成功时会看到 "won":true 和 "frames":106。
/// 本程序运行于 macOS；Apple Silicon 需要 Rosetta 2。回放和状态流无需安装游戏。
/// 如果文件没有执行权限，先运行 chmod +x ./sinking-star。
/// 导出画面需要配套游戏文件和 D3DMetal，设置方法见 ./sinking-star render --help。
#[derive(Parser)]
#[command(
    verbatim_doc_comment,
    disable_version_flag = true,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// 输出二进制对应的游戏版本及 Steam 游戏 EXE 的 SHA-256（JSON）。
    #[arg(long)]
    pub version: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// 回放一条路线，输出帧数和胜负。
    ///
    /// 路线由连续的 ASCII 按键组成，区分大小写。允许空串，不允许空白（含末尾换行）、BOM、注释和文件头。
    /// 不写 -i 时会一直读到标准输入结束；交互输入可按 Ctrl-D 结束。用 </dev/null 可以回放空路线。
    ///
    ///   U/D/L/R   按初始镜头方向移动
    ///   X         动作
    ///   C         切换到下一个可选家族
    ///   V         拿取
    ///   F         龙动作
    ///   Z         撤销
    ///   S / s     按下 / 松开 Shift（跑步）
    ///   K / k     按下 / 松开 Ctrl（慢速）
    ///   T数字     等待操作完成后，再推进指定帧数
    ///
    /// 连续按键可进入队列。T、模拟进入关卡的 V 和路线末尾会先等队列清空、角色移动完成。
    /// 例如 RT10X 表示向右，走完后再等 10 帧，最后执行动作。T0 只等待操作完成。
    /// 帧数为十进制非负整数；游戏每秒 60 帧。按住 Ctrl 时模拟时间为四分之一。
    /// 通关或触发场景切换后立即停止。
    ///
    /// 标准输出为一行 JSON，含 frames（实际帧数）、won（是否赢得谜题）、target_level（切换目标，否则为 null）和 replay_ns（执行操作的纳秒数）。
    /// 大地图终点位于入口时另含 standing_level；离开入口后不输出该字段。
    /// 成功执行返回 0（未通关也返回 0）；无法读取路线或格式错误时返回 2，并输出 {"error":"..."}。
    #[command(verbatim_doc_comment)]
    Replay {
        /// 关卡 ID，区分大小写。
        level: String,
        #[command(flatten)]
        input: Route,
        /// 在 JSON 的 state 字段中输出序列执行后的关卡快照。
        #[arg(long)]
        state: bool,
        /// 大地图专用：站在可进入的入口上按 V 即视为通关，并报告沿途关卡。
        #[arg(long)]
        instant_solve: bool,
        /// 查看角色和附近的机关状态；半径以格计，只适用于 overworld。
        #[arg(long, value_name = "CELLS", value_parser = clap::value_parser!(u8).range(0..=32))]
        inspect_radius: Option<u8>,
        /// 保存逐操作的二进制状态轨迹。
        ///
        /// 同时生成 PATH.json，记录 version、queue_counts 和 ranges。二进制轨迹没有文件头；
        /// 每次操作写入队列记录（每条 28 字节，含家族 UID、动作及 x/y）、u32 累计帧数、
        /// u64 堆游标，以及按 ranges 顺序拼接的原始内存。所有整数为小端，无填充。
        /// 空路线生成空轨迹；开头的 T0 可记录初始状态。末尾自动收尾不另写轨迹条目。
        /// 轨迹只包含部分内存，不能直接用于渲染。
        /// 请同时保留两个文件，只比较同一构建的轨迹。
        #[arg(long, value_name = "PATH", verbatim_doc_comment)]
        trace: Option<PathBuf>,
    },
    /// 导出视频、截图或游戏状态。
    ///
    /// 不需要图形环境的示例：回放后查看状态流的最后一行。
    ///   echo -n 'RDRRRURURRRD' | ./sinking-star render heroes1_1 --render-stream | tail -n 1
    ///
    /// 导出视频或截图需要 Windows Steam 版游戏文件和 D3DMetal。程序会尝试探测常见安装位置。
    /// 如果找不到游戏，将 GAME_ROOT 设为包含 sinking_star.exe、data/animation.package
    /// 和 data-pc 的目录。EXE 必须与 ./sinking-star --version 报告的 SHA-256 一致。
    /// D3DMetal 会在 /Applications/CrossOver.app、~/Applications/CrossOver.app 和 Apple 系统目录中探测。
    /// 可以用 D3DMETAL_PATH 指定 D3DMetal.framework 或其中的库文件，覆盖探测结果。
    /// 示例（先将路径替换为自己的安装路径）：
    ///   export GAME_ROOT="/path/to/Order of the Sinking Star"
    ///   export D3DMETAL_PATH="/path/to/D3DMetal.framework"
    ///   echo -n 'RDRRRURURRRD' | ./sinking-star render heroes1_1 -o demo.mp4
    ///   echo -n 'RDRRRURURRRD' > route.txt
    ///   ./sinking-star render heroes1_1 -i route.txt -o final.png
    ///   ./sinking-star render --inputs heroes1_1.txt heroes2_2b.txt --output-dir videos
    ///
    /// MP4 使用 HEVC，默认 1280×720、30 fps，并生成同名 JSON。
    /// 通关后继续更新场景，直到原版调度器不再被最后的动作阻塞。
    /// PNG/JPEG 只导出最后一帧；未通关的路线在输入结束时停止。
    /// 渲染尺寸用 --resolution 指定，视频宽高必须是偶数。
    /// 批量输入的文件名去掉扩展名后必须是唯一的关卡 ID，每关输出 <ID>.mp4。
    /// 可用 VERIFIED_JSON 提供一个将关卡 ID 映射到预期胜利帧数的 JSON 对象，供批量验证。
    ///
    /// --render-stream 输出逐行 JSON：初始场景、采样时的变化、结束记录。
    /// --render-keys 输出单个 JSON 对象，包含按键事件和结果。两者都不需要 -o。
    #[command(verbatim_doc_comment)]
    Render(Render),
    /// 列出关卡 ID，每行一个。
    ListLevels,
}

#[derive(Args)]
pub struct Render {
    /// 单关导出时的关卡 ID（见 list-levels）。
    #[arg(required_unless_present_any = ["inputs", "version"], conflicts_with_all = ["inputs", "version"])]
    pub level: Option<String>,
    /// 与 replay 相同，站在可进入的大地图入口上按 V 即通关。
    #[arg(long, requires = "level")]
    pub instant_solve: bool,
    #[command(flatten)]
    pub input: Route,
    /// 批量路线文件，文件名去掉扩展名后为关卡 ID。
    #[arg(long, value_name = "FILE", num_args = 1.., requires = "output_dir", conflicts_with_all = ["version", "output", "stream", "route"])]
    pub inputs: Vec<PathBuf>,
    /// 单关输出文件（.mp4、.png、.jpg 或 .jpeg）。
    #[arg(short = 'o', long, value_name = "PATH", required_unless_present_any = ["inputs", "version", "render_keys", "render_stream", "movie_keys", "movie_stream", "movie_image_stream"], conflicts_with_all = ["version", "stream"])]
    pub output: Option<PathBuf>,
    /// 批量输出目录，与 --inputs 一起使用。
    #[arg(long, value_name = "DIR", requires = "inputs")]
    pub output_dir: Option<PathBuf>,
    /// 批量导出的并行数（1–6）。
    #[arg(long, default_value_t = 2, requires = "inputs", value_parser = clap::value_parser!(u8).range(1..=6))]
    pub jobs: u8,
    /// 输出宽高，各为 16–8192（默认 1280x720）。
    #[arg(long, value_name = "WIDTHxHEIGHT", conflicts_with_all = ["version", "stream"])]
    pub resolution: Option<Dimensions>,
    /// 视频帧率，必须是 60 的正因数（默认 30）。
    #[arg(long, value_name = "N", value_parser = parse_fps, conflicts_with_all = ["version", "render_keys", "render_stream", "movie_keys"])]
    pub fps: Option<u32>,
    /// HEVC 恒定质量，0–100，越高越清晰（默认 92）。不设目标码率。
    #[arg(long, value_name = "Q", value_parser = clap::value_parser!(u8).range(0..=100), conflicts_with_all = ["version", "stream"])]
    pub quality: Option<u8>,
    #[arg(long, hide = true, conflicts_with_all = ["inputs", "version", "stream"])]
    pub expected_ticks: Option<u64>,
    /// 向标准输出写入按键事件 JSON。
    #[arg(long, group = "stream", conflicts_with = "version")]
    render_keys: bool,
    /// 向标准输出写入逐行 JSON 状态。
    #[arg(long, group = "stream", conflicts_with = "version")]
    render_stream: bool,
    #[arg(long, hide = true, group = "stream", conflicts_with = "version")]
    movie_keys: bool,
    #[arg(long, hide = true, group = "stream", conflicts_with = "version")]
    movie_stream: bool,
    #[arg(long, hide = true, group = "stream", conflicts_with = "version")]
    movie_image_stream: bool,
    /// 输出渲染器版本信息（JSON）。
    #[arg(long, conflicts_with = "route")]
    pub version: bool,
}

impl Render {
    pub fn stream_mode(&self) -> Option<&'static str> {
        if self.render_keys {
            Some("--render-keys")
        } else if self.render_stream {
            Some("--render-stream")
        } else if self.movie_keys {
            Some("--movie-keys")
        } else if self.movie_image_stream {
            Some("--movie-image-stream")
        } else if self.movie_stream {
            Some("--movie-stream")
        } else {
            None
        }
    }
}

pub const GAME_FPS: u32 = 60;
pub const DEFAULT_FPS: u32 = 30;

fn parse_fps(s: &str) -> Result<u32, &'static str> {
    s.parse::<u32>()
        .ok()
        .filter(|&n| n != 0 && GAME_FPS.is_multiple_of(n))
        .ok_or("fps must be a positive divisor of 60")
}

#[derive(Clone, Copy)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}
impl Default for Dimensions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
        }
    }
}
impl Dimensions {
    pub fn rgba_len(self) -> usize {
        self.width as usize * self.height as usize * 4
    }
}
impl std::fmt::Display for Dimensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}
impl std::str::FromStr for Dimensions {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let error = "expected WIDTHxHEIGHT with each dimension between 16 and 8192";
        let (w, h) = s.split_once('x').ok_or(error)?;
        let parse = |n: &str| {
            n.parse::<u32>()
                .ok()
                .filter(|n| (16..=8192).contains(n))
                .ok_or(error)
        };
        Ok(Self {
            width: parse(w)?,
            height: parse(h)?,
        })
    }
}
