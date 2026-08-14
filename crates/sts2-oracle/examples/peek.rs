use sts2_oracle::{Command, Config, Oracle, Response};

fn main() -> anyhow::Result<()> {
    let dir = sts2_oracle::game_dir_from_env().expect("STS2_GAME_DIR");
    let mut o = Oracle::launch(Config::new(dir))?;
    let start = Command::Start {
        seed: Some("ORACLE1".into()),
        character: Some("IRONCLAD".into()),
        ascension: 0,
        modifiers: vec![],
    };
    let resp: Response = o.send(&start)?;
    println!("{resp}");

    let resp = o.send(&Command::EnterFight)?;
    println!("{resp}");

    Ok(())
}
