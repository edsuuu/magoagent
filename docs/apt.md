# MagoAgent pelo apt (Debian, Ubuntu e derivados)

## Instalar

```bash
curl -fsSL https://edsuuu.github.io/magoagent/apt/magoagent.gpg | sudo tee /usr/share/keyrings/magoagent.gpg > /dev/null
echo "deb [arch=amd64 signed-by=/usr/share/keyrings/magoagent.gpg] https://edsuuu.github.io/magoagent/apt ./" \
  | sudo tee /etc/apt/sources.list.d/magoagent.list
sudo apt update && sudo apt install magoagent
```

Atualizar: `sudo apt update && sudo apt upgrade magoagent` (o app instalado pelo apt avisa isso em vez de se
atualizar sozinho). Remover: `sudo apt remove magoagent` (os dados ficam em `~/.local/share/magoagent`).

Sem o repositório, o `.deb` de uma release também instala direto: `sudo apt install ./magoagent_<versão>_amd64.deb`.

## Como sai (para o dono do repositório)

1. A tag `v<versão>` roda o `release.yml`: o build do Linux no Ubuntu 22.04 gera o `.tar.gz` e o `.deb`
   (`installer/build-deb.sh`) e os põe na release em **rascunho**, junto com o MSI.
2. Ao **publicar** a release, o `pages.yml` baixa o `.deb` dela, monta o repositório apt (Packages, Release,
   InRelease e Release.gpg assinados) e o publica no ramo `gh-pages`. O mesmo fluxo publica o site (`site/`) e roda à mão em Actions › Pages; sem release ou sem segredo, sai só o site, com um aviso.

### Configurar uma vez (a chave e o segredo `APT_GPG_PRIVATE_KEY` já foram criados; o passo 1 é só para refazer)

1. **Chave de assinatura** (o apt recusa repositório sem assinatura). Num Linux:
   ```bash
   gpg --quick-gen-key "MagoAgent apt <111332935+edsuuu@users.noreply.github.com>" rsa4096 sign never
   gpg --armor --export-secret-keys "MagoAgent apt" > magoagent-apt-privada.asc
   ```
   No GitHub: Settings › Secrets and variables › Actions › New repository secret
   `APT_GPG_PRIVATE_KEY` com o conteúdo do `.asc` (e `APT_GPG_PASSPHRASE` se a chave tiver senha). Guarde o
   `.asc` fora do repositório e apague a cópia local depois.
2. **GitHub Pages**: Settings › Pages › Source: *Deploy from a branch*, ramo `gh-pages`, pasta `/ (root)`. O
   repositório precisa ser público (ou um plano com Pages em repositório privado).
3. Publique uma release e confira em Actions › Pages; o site `https://edsuuu.github.io/magoagent/` mostra os comandos e os downloads.

Só a última versão fica no repositório: o `gh-pages` é refeito do zero a cada release.
