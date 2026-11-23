# Agent icons

The sidebar's AGENTS rows, space rows and the rail draw an agent-kind mark
before each agent name. Each `agent-<label>.svg` is an unchanged copy from
shin1ohno/herdr-gpui at 9327f4482faeb0b9ccfb086c912a300a98ebad61
(`assets/icons/`), named after the Herdr daemon's canonical `agent_label`
identity. `crates/yebisu/src/shell/sidebar/agent_icon.rs` maps identities to
files with the fork's table (`crates/sheprd-gpui/src/icons.rs:34`), compared
without regard to case; unknown, empty and missing identities use
`agent-generic.svg`. The files are embedded in the binary (`include_bytes!`),
and `PROVENANCE.tsv` records each copy.

The fork's `omo` mark is a PNG (`agent-omo.png`) with no license record, so it
is not copied and Omo shows the generic icon.

| Label | Source (in herdr-gpui's assets/icons/README.md) |
| --- | --- |
| `pi` | Lobe Icons `pi.svg` (Pi Agent, pi.dev) |
| `claude` | Lobe Icons `claude.svg` |
| `codex` | Lobe Icons `openai.svg` |
| `gemini` | Lobe Icons `gemini.svg` |
| `cursor` | Lobe Icons `cursor.svg` |
| `devin` | Lobe Icons `devin.svg` |
| `agy` | Lobe Icons `antigravity.svg` |
| `cline` | Lobe Icons `cline.svg` |
| `mastracode` | Lobe Icons `mastra.svg` |
| `opencode` | Lobe Icons `opencode.svg` |
| `copilot` | Lobe Icons `githubcopilot.svg` |
| `kimi` | Lobe Icons `kimi.svg` |
| `kiro` | Lobe Icons `kiro.svg` |
| `amp` | Lobe Icons `amp.svg` |
| `grok` | Lobe Icons `grok.svg` |
| `hermes` | Lobe Icons `hermesagent.svg` |
| `kilo` | Lobe Icons `kilocode.svg` |
| `qodercli` | Lobe Icons `qoder.svg` |
| `qwen` | Lobe Icons `qwen.svg` |
| `muse` | Lobe Icons `meta.svg` (Meta's mark; Muse Code has no redistributable mark) |
| `omp`, `droid`, `letta`, `maki` | Original lettermarks (O, D, L, M) |
| generic | Original terminal artwork |

Lobe Icons marks (https://github.com/lobehub/lobe-icons,
`packages/static-svg/icons`) are MIT licensed, Copyright (c) 2023 LobeHub; the
license is in [NOTICE](../../NOTICE) and `assets/licenses/LobeIcons-MIT.txt`,
which `scripts/ios/bundle.sh` ships in the app's `Licenses/`. Their SVG
wrappers were reduced to 24px `currentColor` masks in herdr-gpui; paths are
unchanged. Brand marks identify their respective products and remain their
owners' trademarks. The lettermarks and the generic mark are original
herdr-gpui artwork under Apache-2.0 (`LICENSE`).
