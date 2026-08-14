use sts2_oracle::{Command, Config, Oracle};

fn main() -> anyhow::Result<()> {
    let dir = sts2_oracle::game_dir_from_env().expect("STS2_GAME_DIR");
    let mut o = Oracle::launch(Config::new(dir))?;
    let start = Command::Start {
        seed: Some("ORACLE1".into()),
        character: Some("IRONCLAD".into()),
        ascension: 0,
        modifiers: vec![],
    };
    let _ = o.send(&start)??;
    let _ = o.send(&Command::ChooseOption { index: 0 })??;
    let _ = o.send(&Command::Travel { col: 1, row: 1 })??;
    let _ = o.send(&Command::PlayCard {
        index: 2,
        target: Some(1),
    })??;
    let _ = o.send(&Command::PlayCard {
        index: 0,
        target: None,
    })??;
    let _ = o.send(&Command::PlayCard {
        index: 0,
        target: None,
    })??;
    let resp = o.send(&Command::EndTurn)??;
    println!("{resp}");

    Ok(())
}
