# Guia de modelos locais

O GameFlow funciona sem IA (geradores embutidos). Para textos, histórias, mecânicas e arte mais livres, ligue modelos locais em **⚙ Configurações → Agentes**. Cada tarefa (texto, código, imagem, som) pode usar um modelo diferente.

Rode `gameflow doctor` a qualquer momento para ver quais programas o GameFlow encontrou.

## Texto e código

### Opção A — llama.cpp com modelos GGUF do Hugging Face (recomendada)

1. Instale o `llama-server`:
   - macOS / Linux (Homebrew): `brew install llama.cpp`
   - Windows e outros: baixe os binários em <https://github.com/ggml-org/llama.cpp/releases> e coloque a pasta no `PATH` — ou informe o caminho em **Configurações → Avançado**.
2. Em **Configurações → Modelos (Hugging Face)** baixe um modelo GGUF. Sugestões (a lista "Recomendados" já traz estas):
   | Uso | Modelo | Tamanho aprox. |
   |---|---|---|
   | começar / PCs modestos | `Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF` (arquivo `q4_k_m`) | ~1 GB |
   | equilíbrio | `Qwen/Qwen2.5-Coder-3B-Instruct-GGUF` | ~2 GB |
   | melhor código | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` | ~5 GB (16 GB de RAM) |
   | roteiros/diálogos | `bartowski/Llama-3.2-3B-Instruct-GGUF`, `Qwen/Qwen2.5-7B-Instruct-GGUF` | 2–5 GB |
3. Clique em **Usar p/ código** (ou texto) ao lado do modelo baixado, e em **Agentes → Testar**.
4. Com GPU, adicione em "Argumentos extras": `-ngl 99`.

O GameFlow inicia o `llama-server` sozinho na primeira vez que um agente precisar dele e o encerra ao sair.

### Opção B — Ollama, LM Studio ou qualquer servidor compatível com OpenAI

Escolha **Servidor local compatível com OpenAI** e informe:

| Programa | Endereço | Exemplo de modelo |
|---|---|---|
| Ollama | `http://127.0.0.1:11434/v1` | `ollama pull qwen2.5-coder:7b` → nome `qwen2.5-coder:7b` |
| LM Studio | `http://127.0.0.1:1234/v1` | o nome mostrado na aba "Local Server" |

## Imagens

### stable-diffusion.cpp

1. Instale o [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) (o executável se chama `sd-cli` nas versões novas ou `sd` nas antigas — o GameFlow procura os dois).
2. Baixe um modelo de imagem (ex.: `second-state/stable-diffusion-v1-5-GGUF`) na aba Modelos.
3. Escolha **stable-diffusion.cpp** no agente de imagem e selecione o arquivo. "Argumentos extras" aceita flags como `--steps 20`.

Se você anexou imagens de referência, a primeira é enviada como imagem inicial (`-i`) para guiar o estilo. Sprites gerados por modelo têm o fundo liso removido automaticamente.

### Outros geradores

- **Servidor compatível** (`/v1/images/generations`): informe endereço e modelo.
- **Comando próprio**: use `{prompt}`, `{out}`, `{model}`, `{w}`, `{h}`. Exemplo: `python gerar.py "{prompt}" {out} --size {w}x{h}`.

## Som

- Padrão: sintetizador embutido (efeitos e trilha chiptune coerentes com o tema).
- **Comando próprio**, por exemplo [Piper](https://github.com/rhasspy/piper) para vozes (modelos `.onnx` em `rhasspy/piper-voices`):

  ```
  piper --model {model} --output_file {out}
  ```

  Quando o comando **não** contém `{prompt}`, o texto é enviado pela entrada padrão (como o Piper espera). Placeholders disponíveis: `{prompt}`, `{out}`, `{model}`, `{kind}` (`sfx`/`music`), `{duration}`.

## Dicas

- Falha de um modelo nunca derruba a geração: o agente avisa e usa o gerador embutido.
- Modelos grandes em CPU podem demorar — aumente o tempo limite em **Avançado**.
- Modelos "gated" (que exigem aceitar a licença) pedem um token do Hugging Face (Configurações → Modelos).
- Os arquivos ficam em `gameflow paths` → "dados e modelos"; apague pela interface ou direto na pasta.
