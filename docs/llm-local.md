# Modelos locais (Ollama) e o OmniRoute, para o opencode

Configuração usada no PC de desenvolvimento, para repetir em outra máquina (o Linux de teste). Nada aqui tem chave
nem endereço: as chaves vêm de variáveis de ambiente e o servidor do OmniRoute quem configura é o dono.

## Ollama

Instale pelo site oficial (Linux: `curl -fsSL https://ollama.com/install.sh | sh`, que cria o serviço `ollama`).

Variáveis do servidor (as mesmas do PC):

| Variável | Valor | Por quê |
|---|---|---|
| `OLLAMA_FLASH_ATTENTION` | `1` | atenção mais rápida e com menos memória na GPU |
| `OLLAMA_KV_CACHE_TYPE` | `q8_0` | o cache do contexto pela metade da memória (precisa do flash attention) |
| `OLLAMA_CONTEXT_LENGTH` | `32768` | contexto padrão de 32 k (o opencode manda prompts grandes) |

No Linux elas vão no serviço:

```bash
sudo systemctl edit ollama
# no editor:
[Service]
Environment="OLLAMA_FLASH_ATTENTION=1"
Environment="OLLAMA_KV_CACHE_TYPE=q8_0"
Environment="OLLAMA_CONTEXT_LENGTH=32768"
# depois:
sudo systemctl daemon-reload && sudo systemctl restart ollama
```

### A GPU inteira

- Conferir: com um modelo carregado, `ollama ps` deve mostrar `100% GPU` na coluna PROCESSOR. `xx%/yy% CPU/GPU`
  quer dizer que parte das camadas ficou na CPU (lento).
- **NVIDIA**: driver proprietário instalado (`nvidia-smi` responde). Para pôr todas as camadas na GPU num modelo que
  cabe na memória dela, crie uma variante com `PARAMETER num_gpu 999` (abaixo). Se não couber, diminua `num_ctx`
  antes de tirar camadas.
- **AMD**: o Ollama usa ROCm. Placa fora da lista oficial (muitas Radeon de consumo) só usa a GPU com o override da
  arquitetura mais próxima, no serviço: `Environment="HSA_OVERRIDE_GFX_VERSION=11.0.0"` (RDNA3, RX 7000) ou
  `10.3.0` (RDNA2, RX 6000). Conferir com `journalctl -u ollama | grep -i amdgpu` e `ollama ps`.
- Várias GPUs: `Environment="OLLAMA_SCHED_SPREAD=1"` divide o modelo entre elas.

### Modelos

| Modelo | Tamanho | Uso |
|---|---|---|
| `gpt-oss:20b` | 13 GB | o local do opencode (chamada de ferramentas) |
| `qwen3.6:35b` | 22 GB | maior; a variante de 64 k abaixo |
| `nomic-embed-text` | 274 MB | embeddings |

Variante com contexto de 64 k e tudo na GPU (`Modelfile`):

```
FROM qwen3.6:35b
PARAMETER num_ctx 65536
PARAMETER num_gpu 999
```

`ollama create qwen3.6-64k -f Modelfile`. Numa GPU de 8 GB (a do PC) ela não cabe inteira: tire o `num_gpu` e o
Ollama divide sozinho.

## opencode

`~/.config/opencode/opencode.json`, com os dois provedores (o local direto no Ollama e o OmniRoute):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "model": "omniroute/livre",
  "provider": {
    "ollama": {
      "name": "Ollama (local)",
      "npm": "@ai-sdk/openai-compatible",
      "options": { "baseURL": "http://localhost:11434/v1" },
      "models": {
        "gpt-oss:20b": { "name": "gpt-oss 20b (local)", "tool_call": true, "limit": { "context": 32768, "output": 8192 } }
      }
    },
    "omniroute": {
      "name": "OmniRoute",
      "npm": "@ai-sdk/openai-compatible",
      "options": { "baseURL": "http://localhost:20128/v1", "apiKey": "{env:OMNIROUTE_API_KEY}" },
      "models": {
        "livre": { "name": "Livre (fallback entre modelos)", "tool_call": true, "limit": { "context": 128000, "output": 16384 } },
        "auto/best-coding": { "name": "Auto Best Coding", "tool_call": true, "limit": { "context": 128000, "output": 16384 } }
      }
    }
  }
}
```

Testar: `opencode run -m ollama/gpt-oss:20b "diga oi"` e `opencode run -m omniroute/livre "diga oi"`.

## OmniRoute (na VPS do dono)

O OmniRoute roda numa VPS do dono, ouvindo **só** em `127.0.0.1:20128` lá dentro; nada fica aberto na internet. Uma
máquina usa por um túnel SSH, que deixa o mesmo `localhost:20128` do `opencode.json` acima apontando para a VPS:

```bash
ssh -N -L 20128:127.0.0.1:20128 vps
```

- `vps` é um apelido no `~/.ssh/config` que **o dono configura** (endereço, usuário e chave). Não procure nem
  invente o endereço; se o apelido não existir, peça ao dono.
- Com o túnel aberto: `curl -s http://localhost:20128/v1/models -H "Authorization: Bearer $OMNIROUTE_API_KEY"`
  lista os modelos. `OMNIROUTE_API_KEY` também vem do dono.
- Para ficar de pé: um serviço de usuário do systemd (`systemctl --user`) com `ExecStart=/usr/bin/ssh -N -o
  ServerAliveInterval=30 -o ExitOnForwardFailure=yes -L 20128:127.0.0.1:20128 vps` e `Restart=always`.
- Na VPS o dono tem um guia (`~/LEIA-OMNIROUTE.md`) com o painel e o login dos provedores.
