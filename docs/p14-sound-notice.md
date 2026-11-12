# P14 sound attribution

`assets/sounds/done.mp3` and `request.mp3` are unmodified copies from
shin1ohno/herdr-gpui 398c132a06723a0cc1494842a34666d173ec0200, whose
`crates/sheprd-gpui/SOUND-NOTICE.md` identifies their original source as
herdrdev/herdr 8ac9542757292f7a8d42a2d532bc6a8a33c7ffce under Apache-2.0.
The repository root LICENSE includes that license. No remote or custom audio
paths are accepted. The bounded worker is modeled on that reference's
sound/playback.rs; its local-file reader and external configuration are omitted.

Rodio 0.22.2 is MIT OR Apache-2.0. CPAL and Symphonia resolved versions and
license texts must be checked from the final lockfile before release. The
reference uses CPAL 0.17.3 (Apache-2.0) and Symphonia 0.5.5 MP3 components
(MPL-2.0); this is reference provenance, not proof of this consumer's resolution.
