//! `tower brain boards …`: 보드 목록·보기·이름 바꾸기·지우기. 계약은 tytower 쪽 `boards-contract.md`(서버 기준)를 따른다.
//!
//! 카드 배치와 연결선·화면 위치를 바꾸는 일은 공간 작업이라 화면에서만 한다(`docs/tower-parity.md`).
//! 이름 바꾸기는 GET으로 받은 `revision`을 그대로 실어 PUT한다. 그사이 바뀌었으면 서버가 409를 주고,
//! 여기서는 "다시 하라"는 말로 알린다. 보드 id는 보내기 전에 계약의 규칙으로 검사한다(`tower::check_board_id`).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::cli::BoardsCommand;
use crate::commands::tower::{print_json, when};
use crate::error::AppError;
use crate::tower::{check_board_id, Tower};

/// 보드 이름 길이의 상한(글자 수, 공백을 뺀 뒤). 계약의 `name` 규칙.
pub const NAME_MAX: usize = 80;

/// `GET /api/brain/boards`의 한 줄.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BoardSummary {
    pub id: String,
    pub name: String,
    pub updated: Option<String>,
    pub cards: usize,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BoardList {
    pub boards: Vec<BoardSummary>,
}

/// `GET /api/brain/boards/{id}`의 사람이 읽을 부분. 나머지 필드는 `--json`에서 그대로 나간다.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct BoardView {
    pub id: String,
    pub name: String,
    pub revision: u64,
    pub cards: Vec<CardView>,
    pub edges: Vec<EdgeView>,
    pub view: ViewPos,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CardView {
    #[serde(rename = "noteId")]
    pub note_id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EdgeView {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ViewPos {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

pub fn run(tower: &Tower, command: BoardsCommand, json: bool) -> Result<(), AppError> {
    match command {
        BoardsCommand::Create { id, name } => create(tower, &id, &name, json)?,
        BoardsCommand::List => {
            let value = tower.brain_boards()?;
            if json {
                return print_json(&value);
            }
            let list: BoardList = serde_json::from_value(value)?;
            for line in list_lines(&list) {
                println!("{line}");
            }
        }
        BoardsCommand::Show { id } => {
            let value = tower.brain_board(&id).map_err(|e| not_found(e, &id))?;
            if json {
                return print_json(&value);
            }
            let board: BoardView = serde_json::from_value(value)?;
            for line in board_lines(&board) {
                println!("{line}");
            }
        }
        BoardsCommand::Rename { id, name } => rename(tower, &id, &name, json)?,
        BoardsCommand::Delete { id, yes } => delete(tower, &id, yes, json)?,
    }
    Ok(())
}

/// 빈 보드를 만든다. 새 보드는 revision 0으로 PUT한다. 같은 id가 있으면 서버가 409로 막으므로
/// GET으로 먼저 확인하지 않는다(경쟁 없이 한 번의 요청으로 정해진다). 이미 있으면 아무것도 쓰지 않는다.
fn create(tower: &Tower, id: &str, name: &str, json: bool) -> Result<(), AppError> {
    check_board_id(id)?;
    let name = check_name(name)?;
    let saved = tower.brain_board_save(id, &new_board(id, &name)).map_err(|e| create_error(e, id))?;
    if json {
        return print_json(&saved);
    }
    println!("보드를 만들었습니다: {id} \"{name}\" (revision {})", revision_text(&saved));
    Ok(())
}

/// 새 보드의 저장 본문. 계약의 `Board` 필드만 보낸다: revision 0, 카드와 연결선은 비어 있고, 화면은 가운데 원점 1배율.
pub fn new_board(id: &str, name: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "revision": 0,
        "cards": [],
        "edges": [],
        "view": {"x": 0, "y": 0, "zoom": 1},
    })
}

/// 같은 id가 이미 있으면(409) 안내한다. 서버는 이때 아무것도 쓰지 않는다.
fn create_error(error: AppError, id: &str) -> AppError {
    match error {
        AppError::Api { status: 409, .. } => AppError::Invalid(format!(
            "보드 id '{id}'는 이미 있습니다. 내용은 tower brain boards show {id}로 보고, 이름은 tower brain boards rename {id} <이름>으로 바꿉니다(아무것도 만들거나 바꾸지 않았습니다)"
        )),
        other => other,
    }
}

/// 이름을 바꾼다. 먼저 읽어서 revision을 얻고, 그 revision을 실어 저장한다(서버가 맞는지 본다).
fn rename(tower: &Tower, id: &str, name: &str, json: bool) -> Result<(), AppError> {
    check_board_id(id)?;
    let name = check_name(name)?;
    let current = tower.brain_board(id).map_err(|e| not_found(e, id))?;
    let old_name = current["name"].as_str().unwrap_or("").to_string();
    let body = renamed_board(&current, id, &name)?;
    let saved = tower.brain_board_save(id, &body).map_err(|e| save_error(e, id))?;
    if json {
        return print_json(&saved);
    }
    println!(
        "보드 이름을 바꿨습니다: {id} \"{old_name}\" → \"{name}\" (revision {})",
        revision_text(&saved)
    );
    Ok(())
}

/// 지운다. 되돌릴 수 없어서 `--yes`가 없으면 요청을 보내지 않는다(`cms delete`, `netfile rm`과 같다).
fn delete(tower: &Tower, id: &str, yes: bool, json: bool) -> Result<(), AppError> {
    check_board_id(id)?;
    if !yes {
        return Err(AppError::Invalid(format!(
            "'{id}' 보드를 지웁니다. 되돌릴 수 없으니 정말이면 --yes를 붙이세요"
        )));
    }
    tower.brain_board_delete(id).map_err(|e| not_found(e, id))?;
    if json {
        return print_json(&json!({ "id": id, "deleted": true }));
    }
    println!("보드를 지웠습니다: {id}");
    Ok(())
}

/// 보드 이름을 검사하고 공백을 뺀다. 빈 이름이나 80자를 넘는 이름은 요청 전에 거부한다.
pub fn check_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    let len = trimmed.chars().count();
    if (1..=NAME_MAX).contains(&len) {
        Ok(trimmed.to_string())
    } else {
        Err(AppError::Invalid(format!("보드 이름은 공백을 뺀 뒤 1~{NAME_MAX}자여야 합니다(지금 {len}자)")))
    }
}

/// 저장 요청 본문. 계약의 `Board` 필드만 보낸다(모르는 필드는 서버가 400으로 거부한다).
/// 카드·연결선·화면 위치는 받은 그대로 돌려보낸다. 셋 중 하나라도 모양이 다르면 보내지 않는다.
/// 빈 값으로 덮어쓰면 보드의 내용이 사라지기 때문이다.
pub fn renamed_board(current: &Value, id: &str, name: &str) -> Result<Value, AppError> {
    let unreadable = || AppError::Invalid("보드 응답을 읽지 못했습니다(revision, 카드, 연결선, 화면 위치 중 빠진 것이 있음)".into());
    let revision = current["revision"].as_u64().ok_or_else(unreadable)?;
    if !current["cards"].is_array() || !current["edges"].is_array() || !current["view"].is_object() {
        return Err(unreadable());
    }
    Ok(json!({
        "id": id,
        "name": name,
        "revision": revision,
        "cards": current["cards"],
        "edges": current["edges"],
        "view": current["view"],
    }))
}

/// 404는 "찾지 못했다"로 바꾼다. 목록을 보라는 말을 붙인다.
fn not_found(error: AppError, id: &str) -> AppError {
    match error {
        AppError::Api { status: 404, .. } => {
            AppError::Invalid(format!("보드를 찾지 못했습니다: {id} (tower brain boards list로 확인하세요)"))
        }
        other => other,
    }
}

/// 저장할 때 409(revision이 맞지 않음)는 다시 하라는 말로 바꾼다. 서버는 이때 아무것도 쓰지 않는다.
fn save_error(error: AppError, id: &str) -> AppError {
    match error {
        AppError::Api { status: 409, .. } => AppError::Invalid(format!(
            "보드가 그사이 다른 곳에서 바뀌어 이름을 바꾸지 않았습니다(revision이 맞지 않음). 다시 시도하세요: tower brain boards rename {id} <이름>"
        )),
        other => other,
    }
}

fn revision_text(saved: &Value) -> String {
    saved["revision"].as_u64().map(|r| r.to_string()).unwrap_or_else(|| "?".into())
}

pub fn list_lines(list: &BoardList) -> Vec<String> {
    let mut lines = vec![format!("보드 {}개", list.boards.len())];
    if list.boards.is_empty() {
        lines.push("  보드가 없습니다.".into());
        return lines;
    }
    // id 열은 가장 긴 id에 맞춘다(최소 "id" 두 글자).
    let width = list.boards.iter().map(|b| b.id.chars().count()).max().unwrap_or(0).max(2);
    lines.push(format!("  {:<11}  {:<width$}  {:>5}  {}", "수정", "id", "카드", "이름"));
    for board in &list.boards {
        let updated = board.updated.as_deref().map(when).unwrap_or_else(|| "-".into());
        lines.push(format!("  {updated:<11}  {:<width$}  {:>5}  {}", board.id, board.cards, board.name));
    }
    lines
}

pub fn board_lines(board: &BoardView) -> Vec<String> {
    let mut lines = vec![
        format!("# {}", board.name),
        format!("id: {} · revision {}", board.id, board.revision),
        format!(
            "카드 {}개 · 연결선 {}개 · 화면 x {} y {} 확대 {}",
            board.cards.len(),
            board.edges.len(),
            board.view.x,
            board.view.y,
            board.view.zoom
        ),
        String::new(),
    ];
    if board.cards.is_empty() {
        lines.push("카드: 없음".into());
    } else {
        lines.push("카드 (노트 id로 내용은 tower brain show <id>):".into());
        for card in &board.cards {
            lines.push(format!("  {}  위치 ({}, {})  폭 {}", card.note_id, card.x, card.y, card.w));
        }
    }
    if !board.edges.is_empty() {
        lines.push(String::new());
        lines.push("연결선:".into());
        for edge in &board.edges {
            lines.push(format!("  {} → {}", edge.from, edge.to));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_board() -> Value {
        json!({
            "id": "plan", "name": "기획", "revision": 3,
            "cards": [{"noteId": "bm90ZQ", "x": 40, "y": 60, "w": 240}],
            "edges": [{"from": "bm90ZQ", "to": "bm90ZQ", "kind": "manual"}],
            "view": {"x": 10, "y": 20, "zoom": 1.5}
        })
    }

    #[test]
    fn a_name_is_trimmed_and_limited_to_eighty_characters() {
        assert_eq!(check_name("  새 이름  ").unwrap(), "새 이름");
        assert!(check_name(&"가".repeat(80)).is_ok(), "80 characters is fine");
        assert!(check_name(&"가".repeat(81)).is_err(), "characters, not bytes");
        assert!(check_name("   ").is_err(), "blank after trimming");
        let error = check_name("").unwrap_err().to_string();
        assert!(error.contains("1~80자"), "{error}");
    }

    #[test]
    fn the_save_body_has_only_the_board_fields_and_keeps_the_content() {
        let body = renamed_board(&current_board(), "plan", "새 이름").unwrap();
        let mut keys: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["cards", "edges", "id", "name", "revision", "view"]);
        assert_eq!(body["name"], "새 이름");
        assert_eq!(body["revision"], 3, "the revision read is the one sent back");
        assert_eq!(body["cards"], current_board()["cards"]);
        assert_eq!(body["edges"], current_board()["edges"]);
        assert_eq!(body["view"], current_board()["view"]);
    }

    #[test]
    fn a_board_that_does_not_look_like_a_board_is_never_saved_over() {
        let mut no_cards = current_board();
        no_cards.as_object_mut().unwrap().remove("cards");
        assert!(renamed_board(&no_cards, "plan", "x").is_err(), "a missing card list is not an empty board");
        let mut no_revision = current_board();
        no_revision.as_object_mut().unwrap().remove("revision");
        assert!(renamed_board(&no_revision, "plan", "x").is_err());
    }

    #[test]
    fn a_new_board_is_sent_as_revision_zero_with_an_empty_canvas() {
        let body = new_board("plan", "기획");
        let mut keys: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["cards", "edges", "id", "name", "revision", "view"]);
        assert_eq!(body, json!({
            "id": "plan", "name": "기획", "revision": 0, "cards": [], "edges": [],
            "view": {"x": 0, "y": 0, "zoom": 1}
        }));
    }

    #[test]
    fn a_taken_id_says_to_use_show_or_rename_and_nothing_else() {
        let taken = create_error(AppError::Api { status: 409, message: "stale".into() }, "plan").to_string();
        assert!(taken.contains("이미 있습니다"), "{taken}");
        assert!(taken.contains("boards show plan") && taken.contains("boards rename plan"), "{taken}");
        let other = create_error(AppError::Api { status: 500, message: "x".into() }, "plan").to_string();
        assert!(other.contains("서버 오류(500)"), "{other}");
    }

    #[test]
    fn the_list_shows_a_count_rows_and_a_line_when_empty() {
        let empty = BoardList::default();
        assert_eq!(list_lines(&empty), vec!["보드 0개".to_string(), "  보드가 없습니다.".to_string()]);

        let list: BoardList = serde_json::from_value(json!({"boards": [
            {"id": "plan", "name": "기획", "updated": "2026-11-08T03:00:00Z", "cards": 2},
            {"id": "archive", "name": "보관함", "updated": null, "cards": 0}
        ]}))
        .unwrap();
        let lines = list_lines(&list);
        assert_eq!(lines[0], "보드 2개");
        assert!(lines[2].starts_with("  ") && lines[2].contains("plan") && lines[2].ends_with("기획"), "{lines:?}");
        assert!(lines[3].contains("archive") && lines[3].contains("보관함") && lines[3].contains('-'), "{lines:?}");
    }

    #[test]
    fn a_board_shows_its_cards_edges_and_view_with_no_token_or_raw_json() {
        let board: BoardView = serde_json::from_value(current_board()).unwrap();
        let text = board_lines(&board).join("\n");
        assert!(text.starts_with("# 기획\nid: plan · revision 3\n"), "{text}");
        assert!(text.contains("카드 1개 · 연결선 1개 · 화면 x 10 y 20 확대 1.5"), "{text}");
        assert!(text.contains("  bm90ZQ  위치 (40, 60)  폭 240"), "{text}");
        assert!(text.contains("  bm90ZQ → bm90ZQ"), "{text}");

        let bare: BoardView = serde_json::from_value(json!({"id": "x", "name": "빈", "revision": 1, "cards": [], "edges": [], "view": {}})).unwrap();
        assert!(board_lines(&bare).contains(&"카드: 없음".to_string()));
    }

    #[test]
    fn a_stale_save_says_to_try_again_and_a_missing_board_says_where_to_look() {
        let stale = save_error(AppError::Api { status: 409, message: "stale".into() }, "plan").to_string();
        assert!(stale.contains("다시 시도하세요") && stale.contains("tower brain boards rename plan"), "{stale}");
        let missing = not_found(AppError::Api { status: 404, message: "찾지 못했습니다".into() }, "nope").to_string();
        assert!(missing.contains("보드를 찾지 못했습니다: nope") && missing.contains("boards list"), "{missing}");
        // 다른 오류는 그대로 둔다.
        let other = not_found(AppError::Api { status: 500, message: "x".into() }, "nope").to_string();
        assert!(other.contains("서버 오류(500)"), "{other}");
    }

    #[test]
    fn delete_and_rename_refuse_bad_input_before_any_request() {
        // 주소는 아무도 듣지 않는 곳이다. 요청이 나갔다면 "실행 중이 아닙니다" 같은 다른 오류가 난다.
        let tower = Tower::with_token("http://127.0.0.1:1", Some("token")).unwrap();
        let bad_create = create(&tower, "Plan", "이름", false).unwrap_err().to_string();
        assert!(bad_create.contains("보드 id"), "{bad_create}");
        let bad_create_name = create(&tower, "plan", " ", false).unwrap_err().to_string();
        assert!(bad_create_name.contains("1~80자"), "{bad_create_name}");
        let no_yes = delete(&tower, "plan", false, false).unwrap_err().to_string();
        assert!(no_yes.contains("--yes"), "{no_yes}");
        let bad_id = delete(&tower, "../plan", true, false).unwrap_err().to_string();
        assert!(bad_id.contains("보드 id"), "{bad_id}");
        let bad_name = rename(&tower, "plan", "   ", false).unwrap_err().to_string();
        assert!(bad_name.contains("1~80자"), "{bad_name}");
    }
}
