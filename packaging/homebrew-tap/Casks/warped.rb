cask "warped" do
  version :latest
  sha256 :no_check

  url "https://github.com/bugthesystem/warp/releases/latest/download/Warped.zip"
  name "Warped"
  desc "Temporary build of a Warp fork with an embedded browser pane"
  homepage "https://github.com/bugthesystem/warp"

  depends_on arch: :arm64

  app "Warped.app"

  # Warped is ad-hoc signed, not signed with an Apple Developer ID, so Gatekeeper would refuse to
  # open it while it is quarantined.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Warped.app"]
  end

  zap trash: [
    "~/.warp-oss",
    "~/Library/Application Support/dev.bugthesystem.Warped",
    "~/Library/Preferences/dev.bugthesystem.Warped.plist",
  ]

  caveats <<~EOS
    Warped is temporary: its browser pane is proposed upstream to Warp
    (https://github.com/warpdotdev/warp). Once Warp ships it, switch back.
    Warped is not made or supported by Warp.

    Update with: brew upgrade --cask --greedy warped
  EOS
end
