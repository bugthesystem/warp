Warped is a temporary build of [this Warp fork](https://github.com/bugthesystem/warp) for people who want the embedded browser pane now. The same work is proposed upstream to [Warp](https://github.com/warpdotdev/warp). Once Warp ships it, use Warp instead.

Warped is not made or supported by Warp. It is Warp's open-source build with the browser pane turned on, and it signs in to Warp's servers like Warp does.

## Install

```sh
brew install --cask bugthesystem/tap/warped
```

Or download `Warped.zip`, unzip it into `/Applications`, then run this once:

```sh
xattr -dr com.apple.quarantine /Applications/Warped.app
```

Warped is not signed with an Apple Developer ID, so macOS can't verify who built it and blocks it until the quarantine flag is cleared. Build it yourself with `script/macos/release_warped` if you'd rather not trust a downloaded binary.

Apple silicon only.
