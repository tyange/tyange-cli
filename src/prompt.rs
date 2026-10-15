use std::io::{BufRead, IsTerminal, Write};
use std::process::Command;

use crate::error::AppError;

/// 한 줄을 읽는다. 터미널이면 `label`을 stderr에 보여 주고, 파이프면 조용히 읽는다.
pub fn read_line(label: &str) -> Result<String, AppError> {
    if std::io::stdin().is_terminal() {
        eprint!("{label}");
        std::io::stderr().flush()?;
    }
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

/// 한 줄을 읽되 터미널이면 입력이 화면에 보이지 않게 한다(`stty -echo`). 파이프면 그냥 읽는다.
pub fn read_secret(label: &str) -> Result<String, AppError> {
    let interactive = std::io::stdin().is_terminal();
    if interactive {
        eprint!("{label}");
        std::io::stderr().flush()?;
        let _ = Command::new("stty").arg("-echo").status();
    }

    let mut line = String::new();
    let read = std::io::stdin().lock().read_line(&mut line);

    if interactive {
        let _ = Command::new("stty").arg("echo").status();
        eprintln!();
    }
    read?;
    Ok(line.trim().to_string())
}

pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal()
}
