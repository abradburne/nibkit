# Contributing to nibkit

Issues and pull requests are welcome: bugs, a system nibkit gets wrong, a module an app on a
handheld needs.

## Before you start

- **Open an issue first** for a new module or feature, a new dependency, or a change to a public
  API, so it can be agreed before the work is done.
- One logical change per pull request: no unrelated refactors, reformatting or renames mixed in.
  A few hundred changed lines at most, unless agreed first.

## The checks

What CI runs, and what a pull request must pass:

```sh
cargo fmt --all --check
cargo clippy --all-targets --features dslot,menu-raster,net-control -- -D warnings      # nibOS
cargo clippy --all-targets --features desktop,dslot,menu-raster -- -D warnings          # a desktop
cargo clippy --all-targets --no-default-features --features stock,dslot,menu-raster -- -D warnings
cargo test --features desktop,dslot,menu-raster
cargo test -p nibkit-menu-c
```

Every test runs on a desktop, Linux or macOS: none needs the handheld. The example needs SDL2.
Rust 1.87 is the oldest supported; don't use anything newer without raising `rust-version`.

## Every pull request says

What it changes; why (the problem, with evidence); how; the alternatives considered; the risk;
and how it was tested: the checks above, and for anything that touches the device (brightness,
the battery, the radios, the panels, touch), the board, the system (nibOS or the stock OS), the
commands and the results on it. New behaviour comes with tests where it can be tested; a bug fix
with a regression test where possible. Docs (the README, the module docs) and
[CHANGELOG.md](CHANGELOG.md) change in the same pull request.

## Style

Match the surrounding code. Comments and docs say why, in plain words; the README's tables say
what each module is for. No dead or commented-out code, debug leftovers, personal paths, host
names or secrets.

## Sign-off (DCO)

Every commit carries a `Signed-off-by:` line under the
[Developer Certificate of Origin](https://developercertificate.org/): you wrote the change, or
have the right to submit it under the project's licence. `git commit -s` adds it. CI checks it on
pull requests.

## AI-assisted contributions

**Accepted**, under the same rules as any other change. Much of nibkit was written by AI agents
(Claude), steered by a person who tested the changes on the device.

- **A human submits and is accountable**: they have read the change, run it, and stand behind
  it. Disclose AI help with a `Co-Authored-By:` trailer on the commits.
- **Never add `Signed-off-by:` for someone else.** The human adds their own when they submit.
- Never claim a test or a device run that didn't happen; say plainly what wasn't checked.
- Maintainers may close without a detailed review pull requests that ignore these rules: mass
  rewrites, unverified hardware claims, speculative "improvements", invented APIs.

## Licence

By contributing you agree that your contribution is under the MIT licence
([LICENSE-MIT](LICENSE-MIT)). Fonts and other assets keep their own licences
([assets/README](assets/README)).
