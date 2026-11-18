#!/bin/sh
# O pacote .deb do Linux (Debian, Ubuntu e derivados): sh installer/build-deb.sh, num Linux com dpkg-dev.
# Sai target/release/magoagent_<versão>_amd64.deb, com o programa em /usr/bin, o atalho no menu de aplicativos, o
# ícone e as licenças das fontes. As dependências vêm do próprio binário (dpkg-shlibdeps), mais as bibliotecas que ele
# abre ao rodar (abaixo), então o pacote feito no Ubuntu mais antigo do CI instala nos mais novos. Os pacotes -dev
# (libgtk-3-dev…) são só para compilar: quem instala pelo apt não precisa deles. Os dados de cada pessoa ficam em ~/.local/share/magoagent.
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
arch=$(dpkg --print-architecture)
[ -f target/release/magoagent ] || cargo build --release
root=target/deb/magoagent_${version}_${arch}
rm -rf target/deb && mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/applications" \
  "$root/usr/share/icons/hicolor/512x512/apps" "$root/usr/share/doc/magoagent"
install -m 755 target/release/magoagent "$root/usr/bin/magoagent"
strip "$root/usr/bin/magoagent" 2>/dev/null || true
install -m 644 assets/icon.png "$root/usr/share/icons/hicolor/512x512/apps/magoagent.png"
cat > "$root/usr/share/applications/magoagent.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=MagoAgent AlphaVersion
Comment=Quadro branco com agentes de IA
Exec=magoagent
Icon=magoagent
Terminal=false
Categories=Development;Graphics;
StartupWMClass=magoagent
EOF
for license in assets/*-license.txt; do install -m 644 "$license" "$root/usr/share/doc/magoagent/"; done
printf 'MagoAgent, de edsuuu: https://github.com/edsuuu/magoagent\nAs fontes levam as licenças OFL ao lado.\n' > "$root/usr/share/doc/magoagent/copyright"

# dpkg-shlibdeps quer um debian/control de fonte: um de mentira, numa pasta à parte
work=target/deb/shlibs && mkdir -p "$work/debian"
printf 'Source: magoagent\n\nPackage: magoagent\nArchitecture: any\n' > "$work/debian/control"
depends=$(cd "$work" && dpkg-shlibdeps -O "../../../$root/usr/bin/magoagent" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
# o que o programa abre só ao rodar (dlopen: a janela pelo winit, o OpenGL pelo glutin, as fontes do sistema) não aparece
# para o dpkg-shlibdeps: vai à mão, para o apt instalar tudo junto numa máquina sem esses pacotes
# O navegador no quadro é o WebKitGTK: o binário já liga nele (o dpkg-shlibdeps o acha), mas ele vai à mão também, para
# o pacote nunca sair sem ele; o libwebkit2gtk-4.1-0 traz o resto (JavaScriptCore, GStreamer, bubblewrap)
runtime="libegl1, libgl1, libfontconfig1, libwayland-client0, libwayland-egl1, libx11-6, libx11-xcb1, libxcb1, libxcursor1, libxi6, libxrender1, libxkbcommon0, libxkbcommon-x11-0, libwebkit2gtk-4.1-0"
depends="${depends:+$depends, }$runtime"
size=$(du -sk "$root/usr" | cut -f1)
cat > "$root/DEBIAN/control" <<EOF
Package: magoagent
Version: $version
Architecture: $arch
Maintainer: edsuuu <111332935+edsuuu@users.noreply.github.com>
Installed-Size: $size
Depends: $depends
Recommends: xdg-desktop-portal, git, libayatana-appindicator3-1, gstreamer1.0-libav
Section: devel
Priority: optional
Homepage: https://github.com/edsuuu/magoagent
Description: Quadro branco nativo com terminais, chats e agentes de IA
 MagoAgent é um quadro branco em Rust (egui) com terminais, chats do Claude Code e do
 Codex, árvore de arquivos com editor, a ilha dos agentes no topo da tela e um servidor MCP.
EOF
dpkg-deb --build --root-owner-group "$root" "target/release/magoagent_${version}_${arch}.deb"
echo "target/release/magoagent_${version}_${arch}.deb"
