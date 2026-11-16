#!/bin/sh
# O MSI do Windows: compila o app em release e gera target/release/MagoAgent-<versão>.msi com o
# WiX 3 (candle e light). WIX_BIN aponta para a pasta dos binários do WiX (wix314-binaries.zip);
# não é WIX, que o runner do GitHub define para um WiX que ele não tem mais.
set -e
cd "$(dirname "$0")/.."
WIX_BIN="${WIX_BIN:-$HOME/tools/wix314/bin}"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build --release
out=target/release
obj=target/msi
mkdir -p "$obj"
printf '%s' "$version" > "$obj/versao"
"$WIX_BIN/candle.exe" -nologo -arch x64 -ext WixUtilExtension \
  -dVersion="$version" -dExe="$out/magoagent.exe" -dIcon=assets/icon.ico -dVersionFile="$obj/versao" -dNotices=THIRD-PARTY-NOTICES.md -dData=data \
  -out "$obj/magoagent.wixobj" installer/magoagent.wxs
# ICE61 (REINSTALLMODE com "a") e ICE91 (atalho por usuário numa instalação por máquina) são de propósito
"$WIX_BIN/light.exe" -nologo -ext WixUtilExtension -cultures:pt-BR -sice:ICE61 -sice:ICE91 -sice:ICE38 -sice:ICE64 -sice:ICE57 \
  -out "$out/MagoAgent-$version.msi" "$obj/magoagent.wixobj"
echo "$out/MagoAgent-$version.msi"
