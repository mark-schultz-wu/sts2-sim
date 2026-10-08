# CI runner image and host setup

CI jobs run on this machine. They run inside a Docker image that this
directory builds. No registry holds the image, so the host is the only copy.

## Runners

| Runner | Registered to | Label | Runs |
|---|---|---|---|
| `forgejo-runner-sts2` | `mark/sts2` | `sts2` | Every job in `.forgejo/workflows/`, in `sts2-ci:<digest>`. |
| `forgejo-runner-sts2-builder` | `mark/sts2-infra` only | `sts2-image-builder` | The image build, on the host. |

Both are systemd user services, and their configs are in
`~/.config/forgejo-runner/`. The `sts2` runner's drop-in sets
`TimeoutStopSec=1800`. So a restart lets the jobs in progress finish.

The builder runs on the host with Docker access and the game's package token.
A job in this repository cannot select it. Never give a workflow here the
`sts2-image-builder` label: no runner takes the job, and every job that needs
it waits forever.

## Files

| File | What it does |
|---|---|
| `versions.env` | The versions the image bakes. Rust comes from `rust-toolchain.toml`. |
| `Dockerfile` | The image, which checks its own contents as it builds. |
| `digest.sh` | A hash of every image input. The build bakes it into the image and uses it as the tag. |
| `build.sh` | Builds `sts2-ci:<digest>`. |
| `promote.sh` | Points the `sts2` label at that image and restarts the runner. |
| `checkout.sh` | Baked into the image as `sts2-checkout`. It replaces `actions/checkout`. |

Each job compares `/etc/sts2-ci.digest` with `digest.sh` on its checkout. A
stale image fails the run.

## Change the image

Do this after a change to a file that `digest.sh` hashes: this directory's
build files, `rust-toolchain.toml`, or a lock in `mod/games/`.

1. Merge the change to main.
2. Run the `sts2-infra` workflow "Build and promote the STS2 runner image" with
   `ref` set to `main`. It calls `~/.local/bin/sts2-image-build`, which builds
   only a commit that main contains. Then it promotes the image.

Until the new image is live, every run fails the digest check.

## When the host loses the image

A monthly cron job runs `docker system prune -af`, which deletes the image
when no job uses it. Docker then cannot start a job, and the log says
`pull access denied for sts2-ci`.

The `sts2-image-check.timer` user timer runs every five minutes. It reads the
image that the `sts2` label names. If Docker does not have that image, it runs
`sts2-image-build main`. A cold build takes a few minutes. The script is
`~/.local/bin/sts2-image-check`.

## DNS

The router at 192.168.0.1 sometimes drops DNS queries. When it drops the IPv4
query for `code.schultz-wu.com`, the client has only the IPv6 address. This
host has no IPv6 route, so git, `tea` and curl fail with `network is unreachable`
or `Couldn't connect to server`.

So both of these use Cloudflare's resolvers:

- Job containers, through `--dns 1.1.1.1 --dns 1.0.0.1` in the `sts2`
  runner's `container.options`.
- The host, through the Wi-Fi connection's `ipv4.dns` setting. This covers
  the runner daemons, the builder and the image check.
