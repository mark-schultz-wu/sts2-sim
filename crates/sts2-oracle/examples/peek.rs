use sts2_oracle::{Command, Config, Oracle, Response};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = sts2_oracle::game_dir_from_env().expect("STS2_GAME_DIR");
    let mut o = Oracle::launch(Config::new(dir))?;
    let start = Command::Start {
        seed: Some("ORACLE1".into()),
        character: Some("IRONCLAD".into()),
        ascension: 0,
        modifiers: vec![],
    };
    println!("Starting the game");
    let resp: Response = o.send(&start)?;
    println!("Response Recieved");
    println!("{:?}", resp);
    let observe = Command::Observe;
    let resp = o.send(&observe)?;
    println!("Current state");
    println!("{:?}", resp);
    Ok(())
}
