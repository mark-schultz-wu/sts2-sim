# Scenarios

One file each, named for the rule it pins. A file is the commands that reach a
position, one JSON object per line, with `#` for the reader. What the game
answered lives in `../scenarios.bin`, which `just scenarios` writes.

## Writing one

**Record from the game, never from the simulator.** A test that writes its own
expectation freezes whatever it does today, bug and all. These are safe to
record automatically only because the answer comes from the oracle, so a wrong
simulator still fails. Nothing here records what the simulator said.

**Watch it fail before trusting it.** Break the rule the scenario is for, run
it, and read which section it names. Then put the rule back. A scenario that
only ever passed proves nothing: make it fail for the right reason first.

**Pick numbers a wrong answer changes.** The game truncates, and truncation
hides a small error. `weak-and-shrunk` cannot tell thirty per cent from
twenty-five, because a Strike for six is three either way. That is what
`a-shrunken-monster-hits-for-less` exists for, and it was found by breaking the
constant and watching nothing happen.

**One rule per file.** A failure prints the file's name, and a name that says
what broke is most of the way to the fix. Two rules in one file means a failure
that names neither.

**The fewest steps that reach the position.** Every extra step is more surface
to diverge for an unrelated reason, and a longer file to read. Use `fight` and
`power` to arrive rather than playing there.

**A rule and the scenario that pin it land together.** A port with no scenario
is a rule nothing holds. The long recorded runs say what is *missing*, because
an unported path panics by name. They do not say what is *wrong*, because a
rule that is wrong in a way the run's own numbers do not reach still passes.

## What a failure looks like

    a-shrunken-monster-hits-for-less: step 2 differs in creatures

The section is the part of the observation that moved. `question` is the
question a parked run asks: what it offers, and how many it takes. It leaves
out who asked and every text, which are for a person. Nothing of the game's
own state is kept, so run the scenario against a game to see both sides.
