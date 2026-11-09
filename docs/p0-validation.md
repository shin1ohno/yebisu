# P0 validation

The Mac mini verified GPUI 0.3.12 using herdr-uikit revision a573fb3d90cec084e6211bc84473f2f0906f1731 (platform PR #6, unmerged). Both iOS targets pass Clippy. Host input-boundary tests and the copied iOS tooling tests pass. The physical-device step is pending the owner's interactive keychain unlock.

The smoke screen uses GPUI text, emoji, rounded bordered quads, underline and a checker image. Its field registers ElementInputHandler during paint, displays committed/marked text and logs input events. A background timer notifies the UI once per second. Candidate geometry is intentionally unavailable in P0; this is not a complete terminal/IME editor.

Screenshots under screenshots/ show input, background return, landscape rotation, a landscape frame with the keyboard dismissed, and input after at least 60 seconds idle. UI automation has separate sources in scripts/ios/smoke-qa; XcodeGen can generate that test project, then xcodebuild test can run it against the installed simulator app. Launch the app with simctl --stdout/--stderr first; the test activates the running app so its input log stays captured. Simulator checks do not prove physical IME or hardware keyboard behavior.

Dependency graphs are in dependency-graphs/{device,simulator,host}.txt. The iOS graphs include normal/build edges and the host graph includes dev edges. GPUI resolves to 0.3.12. Cargo.lock was seeded from Sheprd 398c132a06723a0cc1494842a34666d173ec0200, pruned, then updated for GPUI and its new ICU/benchmark dependencies and the registry embed-resource resolution. No blanket cargo update was run. The new P0 app has no connection dependencies yet.

The initial device-target cold development build took 38.88 s; a one-line label change rebuilt in 0.91 s. Development debug symbols and incremental compilation are disabled to bound disk use. The release device binary was also prebuilt without developer signing; just ipad uses incremental release builds and performs provisioning, signing, install and launch after the keychain is unlocked.

The 6 GiB guard samples free space before the command and every five seconds, stopping only its process group. Tests simulate both refusal before launch and an in-flight disk drop, checking that the child no longer exists. The worker never deletes old app targets.
