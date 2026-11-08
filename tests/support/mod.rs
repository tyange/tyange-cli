//! 통합 테스트 도구: 흉내 낸 tytower(std TcpListener)와 `tyange` 바이너리 실행기.
//! 외부 크레이트 없이 돈다. 요청은 모두 기록해 두어서 테스트가 보낸 헤더와 본문을 확인할 수 있다.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::Value;

/// 받은 요청 한 개.
#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    /// 경로와 쿼리(`/api/brain/search?q=rust`).
    pub target: String,
    /// `Authorization` 헤더 값 그대로.
    pub auth: Option<String>,
    pub body: String,
}

impl Req {
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }

    pub fn query(&self) -> &str {
        self.target.split_once('?').map(|(_, q)| q).unwrap_or("")
    }

    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// 돌려줄 응답.
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(status: u16, value: Value) -> Reply {
        Reply { status, content_type: "application/json", body: value.to_string().into_bytes() }
    }

    pub fn bytes(status: u16, content_type: &'static str, body: &[u8]) -> Reply {
        Reply { status, content_type, body: body.to_vec() }
    }
}

/// 흉내 낸 tytower. `url`로 가리키고, 받은 요청은 `requests()`로 본다.
pub struct Mock {
    pub url: String,
    seen: Arc<Mutex<Vec<Req>>>,
}

impl Mock {
    pub fn start(handler: impl Fn(&Req) -> Reply + Send + Sync + 'static) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let addr = listener.local_addr().expect("mock addr");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<dyn Fn(&Req) -> Reply + Send + Sync> = Arc::new(handler);
        let log = seen.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let handler = handler.clone();
                let log = log.clone();
                thread::spawn(move || serve(stream, handler.as_ref(), &log));
            }
        });
        Mock { url: format!("http://{addr}"), seen }
    }

    pub fn requests(&self) -> Vec<Req> {
        self.seen.lock().unwrap().clone()
    }

    /// 메서드와 경로(쿼리 제외)가 같은 요청 수.
    pub fn count(&self, method: &str, path: &str) -> usize {
        self.requests().iter().filter(|r| r.method == method && r.path() == path).count()
    }
}

fn serve(mut stream: TcpStream, handler: &dyn Fn(&Req) -> Reply, log: &Mutex<Vec<Req>>) {
    let Ok(clone) = stream.try_clone() else { return };
    let mut reader = BufReader::new(clone);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    let mut auth = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name == "content-length" {
                content_length = value.parse().unwrap_or(0);
            } else if name == "authorization" {
                auth = Some(value.to_string());
            }
        }
    }
    let mut body = vec![0u8; content_length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let req = Req { method, target, auth, body: String::from_utf8_lossy(&body).into_owned() };
    log.lock().unwrap().push(req.clone());

    let reply = handler(&req);
    let reason = match reply.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Status",
    };
    let head = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reply.status,
        reply.content_type,
        reply.body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.flush();
}

/// 이미 닫힌 포트. 거기로 연결하면 "연결 거부"가 난다(tytower가 꺼져 있는 상황).
pub fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    format!("http://{addr}")
}

/// 테스트마다 따로 쓰는 임시 폴더. 같은 이름이 겹치지 않게 PID와 번호를 붙인다.
pub fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tyange-tower-it-{}-{n}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// `tyange` 한 번 실행한 결과.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    /// 표준 출력과 표준 에러를 합친 것(토큰이 어디에도 없는지 볼 때 쓴다).
    pub fn all_output(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// `tyange`를 실행한다. 이 PC의 설정(env 파일, TYANGE_·TYTOWER_ 변수)은 물려받지 않고,
/// 홈과 설정 폴더는 임시 폴더로 바꾼다. `env`로 준 값만 적용된다.
pub fn tyange(args: &[&str], env: &[(&str, &str)]) -> Run {
    let home = scratch("home");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tyange"));
    cmd.args(args);
    for (key, _) in std::env::vars() {
        if key.starts_with("TYANGE_") || key.starts_with("TYTOWER_") {
            cmd.env_remove(key);
        }
    }
    cmd.env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("APPDATA", &home)
        .env("XDG_CONFIG_HOME", &home)
        .envs(env.iter().copied());
    let out = cmd.output().expect("run tyange");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}
