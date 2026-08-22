//! The wire format is a contract, and a diff on it should be loud.
//!
//! Every line in `golden/` is parsed, rendered again, and compared byte for
//! byte. A record that carries an identity has that identity re-derived from
//! what this build would write, so a change to any field, to the field order
//! or to the escaping shows up here as a failing test rather than as a
//! repository whose annotations two builds disagree about.

use review::{Event, Record, Result, derive_id};

const RECORDS: &str = include_str!("golden/records.jsonl");
const EVENTS: &str = include_str!("golden/events.jsonl");

#[test]
fn every_golden_record_renders_back_to_the_bytes_it_came_from() -> Result<()> {
    for line in RECORDS.lines().filter(|line| !line.is_empty()) {
        let record = Record::parse_line(line)?;
        assert_eq!(record.to_line(), line, "kind {}", record.kind());
    }
    Ok(())
}

#[test]
fn every_golden_record_carries_the_identity_this_build_derives() -> Result<()> {
    for line in RECORDS.lines().filter(|line| !line.is_empty()) {
        let record = Record::parse_line(line)?;
        let Some(id) = record.id() else { continue };
        let blank = line.replacen(&format!(r#""id":"{id}""#), r#""id":"""#, 1);
        assert_eq!(&derive_id(&blank), id, "kind {}", record.kind());
    }
    Ok(())
}

#[test]
fn the_golden_file_carries_one_line_of_every_kind() -> Result<()> {
    let kinds: Vec<String> = RECORDS
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| Ok(Record::parse_line(line)?.kind().as_str().to_owned()))
        .collect::<Result<Vec<String>>>()?;
    assert_eq!(
        kinds,
        vec![
            "comment",
            "rationale",
            "resolve",
            "reply",
            "check",
            "chunk",
            "chunk",
            "work",
            "work",
            "dispatch"
        ]
    );
    Ok(())
}

#[test]
fn every_golden_event_renders_back_to_the_bytes_it_came_from() -> Result<()> {
    for line in EVENTS.lines().filter(|line| !line.is_empty()) {
        let event = Event::parse_line(line)?;
        assert_eq!(event.to_line(), line, "kind {}", event.kind());
    }
    Ok(())
}

#[test]
fn the_golden_file_carries_one_line_of_every_event_kind() -> Result<()> {
    let kinds: Vec<String> = EVENTS
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| Ok(Event::parse_line(line)?.kind().as_str().to_owned()))
        .collect::<Result<Vec<String>>>()?;
    assert_eq!(kinds, vec!["opened", "landed", "abandoned", "retargeted"]);
    Ok(())
}

/// A record keeps the id it was written with, even when this build would
/// derive another one for the same content: that id is what every resolution
/// and reply in the log points at.
#[test]
fn a_record_keeps_the_id_it_was_written_with() -> Result<()> {
    let foreign = r#"{"v":1,"kind":"comment","id":"ffffffffffff","rev":"9f6c1e2a3b4d5e6f708192a3b4c5d6e7f8091a2b","file":"a.txt","side":"new","start":1,"end":1,"body":"if a < b","author":"go","at":"2026-08-21T18:04:05Z"}"#;
    let record = Record::parse_line(foreign)?;
    assert_eq!(
        record.id().map(ToString::to_string),
        Some("ffffffffffff".to_owned())
    );
    assert_eq!(record.to_line(), foreign);
    Ok(())
}
