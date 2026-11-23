# 0042. egui 0.37 with fastframe from its main branch

- Status: Accepted
- Date: 2026-11-24
- Amends: 0033 (the egui version; the fastframe crates stay)

## Context
ADR-0033 lowered egui to 0.36.2 because fastframe's egui crates needed it,
and left "move to the egui release fastframe adopts" as the exit. The owner
asked to update every dependency to its latest version and fastframe to its
latest commit (2026-11-24). Research, 2026-11-24:

- VERIFIED: fastframe's newest tag is still v0.17.1 (egui 0.36). Its `main`
  branch, at `a7dc9bdb12319515c2bbb194054974b304deb36d` (2026-11-22), pins
  egui and eframe 0.37.0 ("Build against egui 0.37").
- VERIFIED: the newest eframe and egui_kittest are 0.37.0.
- VERIFIED by building: the 0.36 forms ADR-0033 introduced are deprecated
  or gone in 0.37, and their 0.37 forms behave the same (`Id::new` calls
  `Id::unique`, `Ui::id` returns `scope_id`, `WidgetType` is an alias of
  `Role`). `State::offset` is private; `clamped_offset` is what the pager
  arrows compare with `0..max_offset`.
- VERIFIED: eframe 0.37 builds the clipboard only with its `clipboard`
  feature, and without it `arboard` leaves the tree. 0.36, which Cineo
  ships, always had it.
- VERIFIED: fastframe `main` still uses zbus 5, and the newest zbus is 6.5.0.
- UNKNOWN: how egui 0.37's own emoji, system fonts and kinetic scrolling
  combine with `fastframe-emoji`, `fastframe-fonts` and `fastframe-scroll`
  on a real desktop.

## Options considered
1. Stay on fastframe v0.17.1 and egui 0.36.2 until fastframe tags a release
   on 0.37.
2. Take fastframe `main` by commit and move to egui 0.37.0.

## Decision
- The fastframe crates come from git at commit
  `a7dc9bdb12319515c2bbb194054974b304deb36d` (`rev`), not a tag.
- `eframe` and `egui_kittest` are 0.37.0. `eframe` adds the `clipboard`
  feature, so copy and paste stay as shipped.
- The fastframe crates ADR-0033 adopted stay, with the same roles.
- `zbus` stays on 5.x, the major version fastframe and `accesskit_unix`
  use, so only one zbus builds.

## Consequences
- The TECHNICAL_DEBT.md entry "egui is held at 0.36.2" is closed.
- A commit is not a release: updating fastframe means choosing a new
  commit, and `cargo update` does not move it.
- The Windows clipboard crates stay, under the same BSL-1.0 exceptions
  (LEGAL.md).

## Rejected alternatives
- Option 1: the owner asked for fastframe's latest commit, and it is the
  one that moves to egui 0.37.
- zbus 6: two zbus versions would build until fastframe moves.
