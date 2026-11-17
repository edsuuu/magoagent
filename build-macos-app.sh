#!/bin/sh
set -eu

cd "$(dirname "$0")"
cargo build --release
mkdir -p MagoAgent.app/Contents/MacOS
mkdir -p MagoAgent.app/Contents/Resources
# a fresh file, not an overwrite: macOS refuses to launch a binary replaced in place
rm -f MagoAgent.app/Contents/MacOS/magoagent
cp target/release/magoagent MagoAgent.app/Contents/MacOS/magoagent
# assinatura ad-hoc (sem ela as permissões se perdem a cada build)
codesign --force --deep --sign - MagoAgent.app
# ícone
cp assets/icon.icns MagoAgent.app/Contents/Resources/icon.icns
cat > MagoAgent.app/Contents/Info.plist <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>magoagent</string>
  <key>CFBundleIdentifier</key><string>com.edsuuu.magoagent</string>
  <key>CFBundleName</key><string>MagoAgent</string>
  <key>CFBundleDisplayName</key><string>MagoAgent</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>O MagoAgent usa o microfone para a chamada de voz e o ditado.</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>NSAppleEventsUsageDescription</key><string>O MagoAgent mostra na ilha a música que toca no Spotify e a pausa ou pula a pedido.</string>
</dict>
</plist>
PLIST
open MagoAgent.app
