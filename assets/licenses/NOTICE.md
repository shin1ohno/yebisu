# Third-party notices shipped inside the yebisu app bundle

yebisu embeds notification sounds and agent icons and links an audio decoder. The licenses below
travel with every bundle (`Licenses/` in the app), as their terms require.

| Component | Version | License | Text in this folder |
|---|---|---|---|
| `done.mp3`, `request.mp3` (Herdr notification sounds) | herdrdev/herdr 8ac9542757292f7a8d42a2d532bc6a8a33c7ffce | Apache-2.0 | `Apache-2.0.txt` |
| Rodio | 0.22.2 | MIT OR Apache-2.0 | `Rodio-MIT.txt`, `Apache-2.0.txt` |
| CPAL | 0.17.3 | Apache-2.0 | `Apache-2.0.txt` |
| `agent-*.svg` brand marks (sidebar agent icons, from Lobe Icons via herdr-gpui) | shin1ohno/herdr-gpui 9327f4482faeb0b9ccfb086c912a300a98ebad61 | MIT | `LobeIcons-MIT.txt` |
| `agent-omp.svg`, `agent-droid.svg`, `agent-letta.svg`, `agent-maki.svg`, `agent-generic.svg` (original lettermarks and terminal mark from herdr-gpui) | shin1ohno/herdr-gpui 9327f4482faeb0b9ccfb086c912a300a98ebad61 | Apache-2.0 | `Apache-2.0.txt` |
| Symphonia (MP3 decoder: symphonia, symphonia-core, symphonia-bundle-mp3, symphonia-metadata) | 0.5.5 | MPL-2.0 | `Symphonia-MPL-2.0.txt` |

- The two sounds are unmodified copies; see `docs/p14-sound-notice.md` for the
  chain from herdr-gpui 398c132a06723a0cc1494842a34666d173ec0200.
- Symphonia is used unmodified from crates.io. The MPL-2.0 source for these
  exact versions is available from https://github.com/pdeljanov/Symphonia and
  https://crates.io/crates/symphonia/0.5.5; yebisu's own source stays under the
  repository `LICENSE`.
- The agent icons are unmodified copies; `assets/icons/README.md` lists each
  file's source. Brand marks identify their products and remain their owners'
  trademarks.
- `Apache-2.0.txt` is the repository `LICENSE`, copied by `scripts/ios/bundle.sh`.
