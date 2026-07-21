# rayflex task runner.  `just` with no target lists everything.
#
# The important one is `just check`: it runs exactly what CI runs. In
# particular it builds the WHOLE workspace -- plain `cargo build` only builds
# the root package and silently skips the `xtask` member, which let a
# RenderConfig field change compile green here and break `cargo xtask`.
# That shipped twice. Run `just check` before pushing.

default:
    @just --list

# Everything CI enforces: workspace build + tests + clippy + rustfmt.
check: build test clippy fmt-check

build:
    cargo build --workspace --release

test:
    cargo test --workspace --release

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Render a scene to /tmp. e.g. `just render cornell-box 400 400 200`
# (spp of 0 or 1 = classic ray tracing; >1 = path tracing).
render scene x="480" y="300" spp="64":
    cargo run --release -- -l scenes/{{scene}}.json -x {{x}} -y {{y}} \
        -p {{spp}} -g --reflection-max-depth 8 --img-file /tmp/rayflex-{{scene}}.png
    @echo "wrote /tmp/rayflex-{{scene}}.png"

# Open the interactive UI.
ui:
    cargo run --release -- -u

# Regenerate committed assets/ images (deterministic: xtask pins a seed, so
# an unchanged renderer reproduces them byte for byte and `git diff` is a
# real "did the output change?" signal). Takes ~30 min for all of them.
#   just assets            -> all
#   just assets sponza     -> one
#   just assets "" --fast  -> quick low-res preview of all
assets scene="" flags="":
    cargo xtask -- {{scene}} {{flags}}

# List the scenes you can render.
scenes:
    @ls scenes/*.json | xargs -n1 basename | sed 's/\.json$//'
