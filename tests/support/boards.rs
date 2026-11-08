//! 흉내 낸 tytower의 보드 API(`/api/brain/boards`). 계약(tytower 쪽 `boards-contract.md`)을 따른다.
//!
//! - 목록: `updated` 내림차순. 항목은 `{id, name, updated, cards}`(cards는 개수).
//! - 보기: 없으면 404. `bump_revision_after_next_get()`을 부르면 다음 GET 뒤에 revision이 한 번 올라간다.
//!   같은 시점에 다른 기기가 저장한 것처럼 만들어서, 이름 바꾸기가 stale 응답을 받는 상황을 재현한다.
//! - 저장(PUT): 본문 `id`가 경로와 같아야 하고(400), 모르는 필드는 400, `name`은 공백을 뺀 1~80자.
//!   `revision`이 서버 값과 다르면 409 `{"error":"stale","current_revision":n}`이고 아무것도 쓰지 않는다.
//!   맞으면 `revision + 1`로 저장하고 `{id, revision, updated}`를 준다. 없는 보드는 만든다(기준 revision 0).
//! - 지우기(DELETE): 204, 없으면 404.
//! - 경로의 id가 규칙에 안 맞으면 400(`invalid_id`). CLI는 이런 요청을 보내지 않아야 하고, 테스트가 그걸 본다.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::{Reply, Req};

/// 저장할 때 서버가 찍는 시각(테스트에서 고정한다).
pub const SAVED_AT: &str = "2026-11-08T05:30:00Z";

#[derive(Clone)]
struct Stored {
    board: Value,
    updated: String,
}

#[derive(Default)]
struct State {
    boards: BTreeMap<String, Stored>,
    bump_after_next_get: bool,
}

/// 보드 저장소. `Mock::start`의 핸들러 안에서 `route`를 부르고, 테스트는 `board`로 결과를 본다.
pub struct Boards {
    state: Mutex<State>,
}

impl Boards {
    /// 보드가 하나도 없는 서버.
    pub fn empty() -> Arc<Boards> {
        Self::with(Vec::new())
    }

    /// 주어진 보드들을 가진 서버. 각 보드에 마지막 수정 시각(`updated`)을 함께 준다.
    pub fn with(seeds: Vec<(Value, &str)>) -> Arc<Boards> {
        let mut state = State::default();
        for (board, updated) in seeds {
            let id = board["id"].as_str().expect("a seed board needs an id").to_string();
            state.boards.insert(id, Stored { board, updated: updated.to_string() });
        }
        Arc::new(Boards { state: Mutex::new(state) })
    }

    /// 지금 서버가 가진 보드(없으면 `None`).
    pub fn board(&self, id: &str) -> Option<Value> {
        self.state.lock().unwrap().boards.get(id).map(|s| s.board.clone())
    }

    /// 다음 GET 뒤에 revision을 한 번 올린다.
    pub fn bump_revision_after_next_get(&self) {
        self.state.lock().unwrap().bump_after_next_get = true;
    }

    /// `/api/brain/boards`로 시작하는 경로를 처리한다. 다른 경로면 `None`.
    pub fn route(&self, req: &Req) -> Option<Reply> {
        let rest = req.path().strip_prefix("/api/brain/boards")?;
        if rest.is_empty() {
            return Some(match req.method.as_str() {
                "GET" => self.list(),
                _ => reply(405, "method_not_allowed"),
            });
        }
        let id = rest.strip_prefix('/')?;
        if !valid_id(id) {
            return Some(reply(400, "invalid_id"));
        }
        Some(match req.method.as_str() {
            "GET" => self.get(id),
            "PUT" => self.put(id, req),
            "DELETE" => self.delete(id),
            _ => reply(405, "method_not_allowed"),
        })
    }

    fn list(&self) -> Reply {
        let state = self.state.lock().unwrap();
        let mut rows: Vec<(&String, &Stored)> = state.boards.iter().collect();
        rows.sort_by(|a, b| b.1.updated.cmp(&a.1.updated));
        let boards: Vec<Value> = rows
            .iter()
            .map(|(id, s)| {
                json!({
                    "id": id,
                    "name": s.board["name"],
                    "updated": s.updated,
                    "cards": s.board["cards"].as_array().map_or(0, Vec::len),
                })
            })
            .collect();
        Reply::json(200, json!({ "boards": boards }))
    }

    fn get(&self, id: &str) -> Reply {
        let mut state = self.state.lock().unwrap();
        let Some(stored) = state.boards.get(id) else {
            return reply(404, "not_found");
        };
        // 응답은 바뀌기 전 모습이다. 그 뒤에 revision을 올린다.
        let answer = stored.board.clone();
        if state.bump_after_next_get {
            state.bump_after_next_get = false;
            if let Some(s) = state.boards.get_mut(id) {
                let next = revision_of(&s.board) + 1;
                s.board["revision"] = json!(next);
            }
        }
        Reply::json(200, answer)
    }

    fn put(&self, id: &str, req: &Req) -> Reply {
        let Ok(body) = serde_json::from_str::<Value>(&req.body) else {
            return reply(400, "invalid_json");
        };
        if let Err(code) = check_body(&body, id) {
            return reply(400, code);
        }
        // 읽기, 검사, 쓰기를 한 번에(계약: 같은 보드에 한 번에 한 요청).
        let mut state = self.state.lock().unwrap();
        let current = state.boards.get(id).map_or(0, |s| revision_of(&s.board));
        if body["revision"].as_u64() != Some(current) {
            return Reply::json(409, json!({ "error": "stale", "current_revision": current }));
        }
        let mut saved = body.clone();
        saved["name"] = json!(body["name"].as_str().unwrap_or("").trim());
        saved["revision"] = json!(current + 1);
        state.boards.insert(id.to_string(), Stored { board: saved, updated: SAVED_AT.into() });
        Reply::json(200, json!({ "id": id, "revision": current + 1, "updated": SAVED_AT }))
    }

    fn delete(&self, id: &str) -> Reply {
        let mut state = self.state.lock().unwrap();
        if state.boards.remove(id).is_some() {
            Reply::bytes(204, "application/json", b"")
        } else {
            reply(404, "not_found")
        }
    }
}

fn reply(status: u16, code: &str) -> Reply {
    Reply::json(status, json!({ "error": code }))
}

fn revision_of(board: &Value) -> u64 {
    board["revision"].as_u64().unwrap_or(0)
}

/// 서버의 id 규칙: `[a-z0-9][a-z0-9-]{0,39}`.
fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (1..=40).contains(&bytes.len())
        && bytes.iter().enumerate().all(|(i, b)| b.is_ascii_lowercase() || b.is_ascii_digit() || (i > 0 && *b == b'-'))
}

/// 저장 본문의 검사(계약의 "검증" 절). 틀리면 400에 실을 코드를 돌려준다.
fn check_body(body: &Value, id: &str) -> Result<(), &'static str> {
    const KNOWN: [&str; 6] = ["id", "name", "revision", "cards", "edges", "view"];
    let fields = body.as_object().ok_or("invalid_body")?;
    if fields.keys().any(|k| !KNOWN.contains(&k.as_str())) {
        return Err("unknown_field");
    }
    if body["id"].as_str() != Some(id) {
        return Err("id_mismatch");
    }
    let name_len = body["name"].as_str().map_or(0, |n| n.trim().chars().count());
    if !(1..=80).contains(&name_len) {
        return Err("invalid_name");
    }
    if body["revision"].as_u64().is_none() {
        return Err("invalid_revision");
    }
    let cards = body["cards"].as_array().ok_or("invalid_cards")?;
    if cards.len() > 2000 {
        return Err("too_many_cards");
    }
    let mut note_ids = HashSet::new();
    for card in cards {
        let note = card["noteId"].as_str().filter(|n| (1..=512).contains(&n.len())).ok_or("invalid_card")?;
        let placed = card["x"].as_f64().is_some()
            && card["y"].as_f64().is_some()
            && card["w"].as_f64().is_some_and(|w| (120.0..=800.0).contains(&w));
        if !placed {
            return Err("invalid_card");
        }
        if !note_ids.insert(note) {
            return Err("duplicate_card");
        }
    }
    let edges = body["edges"].as_array().ok_or("invalid_edges")?;
    if edges.len() > 5000 {
        return Err("too_many_edges");
    }
    for edge in edges {
        let linked = |key: &str| edge[key].as_str().is_some_and(|n| note_ids.contains(n));
        if edge["kind"] != "manual" || !linked("from") || !linked("to") {
            return Err("invalid_edge");
        }
    }
    let view = &body["view"];
    if view["x"].as_f64().is_none() || view["y"].as_f64().is_none() {
        return Err("invalid_view");
    }
    if !view["zoom"].as_f64().is_some_and(|z| (0.1..=4.0).contains(&z)) {
        return Err("invalid_view");
    }
    Ok(())
}
