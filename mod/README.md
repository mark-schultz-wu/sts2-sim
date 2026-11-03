# The mods

Each mod has one job. A run loads the ones its job needs.

| Mod | Its job |
|---|---|
| `sts2_determinism` | Makes the game reproducible: same seed, same run. |
| `sts2_headless` | The game without its presentation layer. It stands in for the screens: it reports what they show, and it does what a click does. It speaks the protocol. |
| `sts2_setup` | Puts a run into a position without playing to it, as the developer console does. It adds commands to the protocol. |
| `sts2_fixes` | Patches game bugs the simulator models as fixed. Each patch changes the game. |
| `sts2_net` | Co-op over the game's ENet transport on one machine, with no Steam and no lobby screen. A game with its screens can load it too. |
| `sts2_observe` | Reports what the game does inside, when a run asks. Off unless asked. |
| `sts2_dump` | Dumps the game's content for code generation. |
| `sts2_headless_test`, `sts2_determinism_test` | Check the others inside the game. |

A mod reports facts and does what the game's own screens do. Deciding what the
facts mean is the Rust client's job.

`build.sh` builds them in dependency order, and `deploy.sh` installs them.
`identity.sh` hashes the source of every mod the oracle loads, for the greeting.

## The game

The mods compile against the game's assemblies, and the checks run the game.
This repository holds neither. Each supported build has a lock under
`games/`, which holds the hash of each assembly. `game.sh` names each
build's Steam manifest. `game.sh` moves the game into place.

With a Steam account that owns the game, get each build from Steam:

```sh
STS2_STEAM_USERNAME=<your account> just acquire-game v0.107.1
just install-game v0.107.1
```

`acquire-game` needs
[DepotDownloader](https://github.com/SteamRE/DepotDownloader). It asks for
your password and Steam Guard code itself, and `game.sh` stores neither.
`install-game` checks the copy against the lock, and refuses one that does
not match. It puts the game in `$XDG_DATA_HOME/sts2/games/<build>`, where
`.envrc` points `STS2_GAME_DIR`. To install a copy from somewhere else, give
its directory: `just install-game v0.107.1 /path/to/copy`.

Mark's machines fetch the same builds from a private package on his forge
instead, with `just vendor-game`. That needs a token for the forge.

The mods and the checks need the .NET SDK too, and run on Linux.
