# The image every CI job runs in.
#
# What a job needs and does not produce belongs here rather than in a step. A
# workflow that installs its own toolchain pays for it on every run. It also
# says the same twelve lines three times over, once per job, each with a
# comment on how the install can fail silently. None of that is about this
# repository. It is about a machine, so it lives in one.
#
# Built by `build.sh` beside this file and reached through the runner's `sts2`
# label, not pulled: there is no registry in the loop. `force_pull` is off in
# the runner config, so the local tag is what runs.
FROM ghcr.io/catthehacker/ubuntu:act-22.04

# Passed by `build.sh` from versions.env, which is the only place they are
# written down. Defaults are absent on purpose: a build that forgets to pass
# them must fail rather than bake a surprise.
ARG RUST_VERSION
ARG DOTNET_CHANNEL

# Under /usr/local rather than a home directory. The job can run as anyone,
# and a toolchain in ~root is a toolchain the job cannot see.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    DOTNET_ROOT=/usr/local/dotnet \
    PATH=/usr/local/cargo/bin:/usr/local/dotnet:$PATH \
    DOTNET_NOLOGO=1 \
    DOTNET_CLI_TELEMETRY_OPTOUT=1 \
    DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1

# The Rust toolchain, its two lints, and the browser target the simulator is
# checked against.
RUN set -euo pipefail; \
    test -n "$RUST_VERSION"; \
    curl --proto '=https' --tlsv1.2 -fsSL \
      --retry 8 --retry-all-errors --retry-delay 2 \
      --connect-timeout 15 --max-time 300 \
      https://sh.rustup.rs \
      | sh -s -- -y --no-modify-path --profile minimal \
        --default-toolchain "$RUST_VERSION" \
        --component clippy,rustfmt --target wasm32-unknown-unknown; \
    chmod -R a+rX /usr/local/rustup /usr/local/cargo

# The .NET SDK the mods are compiled with. `-L` is not optional: dot.net answers
# 301 to builds.dotnet.microsoft.com, and a 301 is not an error to `curl -f`.
RUN set -euo pipefail; \
    test -n "$DOTNET_CHANNEL"; \
    curl --proto '=https' --tlsv1.2 -fsSL \
      --retry 8 --retry-all-errors --retry-delay 2 \
      --connect-timeout 15 --max-time 300 \
      -o /tmp/dotnet-install.sh https://dot.net/v1/dotnet-install.sh; \
    bash /tmp/dotnet-install.sh --channel "$DOTNET_CHANNEL" --install-dir "$DOTNET_ROOT"; \
    rm /tmp/dotnet-install.sh; \
    chmod -R a+rX "$DOTNET_ROOT"

# `just`, so a job can run the recipe the developer runs instead of a second
# copy of it spelled out in YAML.
RUN set -euo pipefail; \
    curl --proto '=https' --tlsv1.2 -fsSL \
      --retry 8 --retry-all-errors --retry-delay 2 \
      --connect-timeout 15 --max-time 300 \
      https://just.systems/install.sh \
      | bash -s -- --to /usr/local/bin

# The first `dotnet` of an install unpacks bundles and writes a sentinel, which
# took thirty-two seconds of a fifty-seven second job. Paying it here means no
# job pays it.
RUN dotnet --info >/dev/null

# The image asserts what it holds. A layer that installed nothing must not
# produce an image that looks fine until a job fails oddly inside it.
RUN set -euo pipefail; \
    test "$(cargo --version | cut -d' ' -f2)" = "$RUST_VERSION"; \
    rustup component list --installed | grep -q "^clippy"; \
    rustup component list --installed | grep -q "^rustfmt"; \
    rustup target list --installed | grep -qx wasm32-unknown-unknown; \
    case "$(dotnet --version)" in "$DOTNET_CHANNEL".*) ;; *) exit 1 ;; esac; \
    just --version; \
    python3 --version
