// Directions are counterclockwise from screen-right; rotation comes from the scene.
#[derive(Clone, Copy)]
pub enum Key {
    Direction(u8),
    Action,
    Swap,
    Take,
    Dragon,
    Undo,
    Slow(bool),
    Run(bool),
    Frames(u32),
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Key>, String> {
    use Key::*;
    let mut keys = Vec::new();
    let mut i = 0;
    let mut total = 0u32;
    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        i += 1;
        keys.push(match b {
            b'R' => Direction(0),
            b'U' => Direction(1),
            b'L' => Direction(2),
            b'D' => Direction(3),
            b'X' => Action,
            b'C' => Swap,
            b'V' => Take,
            b'F' => Dragon,
            b'Z' => Undo,
            b'S' => Run(true),
            b's' => Run(false),
            b'K' => Slow(true),
            b'k' => Slow(false),
            b'T' => {
                let mut n = 0u32;
                if !bytes.get(i).is_some_and(u8::is_ascii_digit) {
                    return Err(format!("expected frame count after T at byte {start}"));
                }
                while let Some(&digit @ b'0'..=b'9') = bytes.get(i) {
                    n = n
                        .checked_mul(10)
                        .and_then(|x| x.checked_add((digit - b'0') as u32))
                        .ok_or_else(|| format!("frame count overflow at byte {start}"))?;
                    i += 1;
                }
                total = total
                    .checked_add(n)
                    .ok_or_else(|| format!("total frame count overflow at byte {start}"))?;
                Frames(n)
            }
            _ => return Err(format!("invalid key 0x{b:02x} at byte {start}")),
        });
    }
    Ok(keys)
}

#[derive(clap::Args)]
pub struct Route {
    /// ASCII 路线文件；省略时从标准输入读到 EOF。按键格式见 replay --help。
    #[arg(short = 'i', long = "input", value_name = "ROUTE")]
    route: Option<std::path::PathBuf>,
}

impl Route {
    pub fn read(self) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        if let Some(path) = self.route {
            return std::fs::read(path);
        }
        let mut bytes = Vec::new();
        std::io::stdin().lock().read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
