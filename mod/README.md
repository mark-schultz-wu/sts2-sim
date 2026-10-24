# The mods

Each mod has one job. A run loads the ones its job needs.

| Mod | Its job |
|---|---|
| `sts2_determinism` | Makes the game reproducible: same seed, same run. |
| `sts2_headless` | The game without its presentation layer. It stands in for the screens: it reports what they show, and it does what a click does. It speaks the protocol. |
| `sts2_setup` | Puts a run into a position without playing to it, as the developer console does. It adds commands to the protocol. |
| `sts2_fixes` | Patches game bugs the simulator models as fixed. Each patch changes the game. |
| `sts2_observe` | Reports what the game does inside, when a run asks. Off unless asked. |
| `sts2_dump` | Dumps the game's content for code generation. |
| `sts2_headless_test`, `sts2_determinism_test` | Check the others inside the game. |

A mod reports facts and does what the game's own screens do. Deciding what the
facts mean is the Rust client's job.

`build.sh` builds them in dependency order, and `deploy.sh` installs them.
`identity.sh` hashes the source of every mod the oracle loads, for the greeting.
