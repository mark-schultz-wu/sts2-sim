# Co-op scenarios

The scenarios of [`../scenarios`](../scenarios/README.md), for a run of
several players on v0.111.0. The rules for writing one are the same.

A file starts with a `start` that names the players. Each line after it
carries the seat whose step it is, as `"seat"`. A decision is played by
that seat. A setup goes to every seat's game at once. Each game makes the
position for itself, and a game entering a fight waits for the others. A setup that names a player names them by net id: 1 for seat 0,
and 1000 + k for seat k.

After each step, every seat's position is noted, in seat order. A failure
names the step and the seat:

    an-attacker-who-dies-mid-attack-stops: step 5, seat 0 differs in run

What the games answered lives in `../coop-scenarios.bin`. To write it,
deploy the mod for v0.111.0 and run:

    STS2_GAME_VERSION=v0.111.0 STS2_GAME_DIR=<the v0.111.0 game> \
        STS2_UPDATE=1 cargo test -p sts2-golden --test coop_scenarios

Without `STS2_UPDATE` the test reads the file and needs no game.
