# bugthesystem/tap

```sh
brew install --cask bugthesystem/tap/warped
```

**Warped** is a temporary macOS build of [bugthesystem/warp](https://github.com/bugthesystem/warp), a fork of [Warp](https://github.com/warpdotdev/warp) with an embedded browser pane that agents (Warp's own and Claude Code) can drive. The same work is proposed upstream to Warp. Once Warp ships it, uninstall Warped and use Warp.

Warped is not made or supported by Warp. It is Warp's open-source build with the browser pane turned on. It signs in to Warp's servers like Warp does, keeps its data in `~/.warp-oss`, and installs next to an official Warp without touching it.

It is not signed with an Apple Developer ID, so the cask clears macOS's quarantine flag after installing. macOS can't verify who built it; build it yourself from the fork with `script/macos/release_warped` if you'd rather not trust a downloaded binary.

The cask always installs the latest release. Update with `brew upgrade --cask --greedy warped`.

Apple silicon only.
