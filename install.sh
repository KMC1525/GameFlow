#!/usr/bin/env sh
# Instalador do GameFlow para Linux e macOS.
#   curl -fsSL https://raw.githubusercontent.com/KMC1525/GameFlow/main/install.sh | sh
#
# Variáveis opcionais:
#   GAMEFLOW_VERSION      versão específica (ex.: v0.1.0). Padrão: a mais recente.
#   GAMEFLOW_INSTALL_DIR  pasta de instalação. Padrão: ~/.local/bin
set -eu

REPO="KMC1525/GameFlow"
INSTALL_DIR="${GAMEFLOW_INSTALL_DIR:-$HOME/.local/bin}"

say()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33maviso:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merro:\033[0m %s\n' "$*" >&2; exit 1; }

# --- plataforma -------------------------------------------------------------
os="$(uname -s)"; arch="$(uname -m)"
case "$os" in
  Linux)  os_part="unknown-linux-gnu" ;;
  Darwin) os_part="apple-darwin" ;;
  *) die "sistema não suportado: $os (no Windows use: irm https://raw.githubusercontent.com/$REPO/main/install.ps1 | iex)" ;;
esac
case "$arch" in
  x86_64|amd64)  arch_part="x86_64" ;;
  aarch64|arm64) arch_part="aarch64" ;;
  *) die "arquitetura não suportada: $arch" ;;
esac
target="$arch_part-$os_part"
asset="gameflow-$target.tar.gz"

if [ -n "${GAMEFLOW_VERSION:-}" ]; then
  base="https://github.com/$REPO/releases/download/$GAMEFLOW_VERSION"
else
  base="https://github.com/$REPO/releases/latest/download"
fi

# --- download ---------------------------------------------------------------
if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q "$1" -O "$2"; }
else
  die "preciso de curl ou wget instalado"
fi

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT INT TERM

install_from_source() {
  if command -v cargo >/dev/null 2>&1; then
    say "Compilando a partir do código-fonte com cargo (pode levar alguns minutos)…"
    cargo install --git "https://github.com/$REPO" --locked --force
    say "Pronto! Rode:  gameflow"
    exit 0
  fi
  die "não encontrei uma versão publicada para $target. Instale o Rust (https://rustup.rs) e rode: cargo install --git https://github.com/$REPO --locked"
}

say "Baixando GameFlow para $target…"
if ! fetch "$base/$asset" "$tmp/$asset" 2>/dev/null; then
  warn "download indisponível ($base/$asset)"
  install_from_source
fi

# --- integridade ------------------------------------------------------------
if fetch "$base/$asset.sha256" "$tmp/$asset.sha256" 2>/dev/null; then
  want="$(cut -d' ' -f1 "$tmp/$asset.sha256")"
  if command -v sha256sum >/dev/null 2>&1; then got="$(sha256sum "$tmp/$asset" | cut -d' ' -f1)";
  elif command -v shasum >/dev/null 2>&1; then got="$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1)";
  else got=""; warn "sem sha256sum/shasum: pulando a verificação de integridade"; fi
  if [ -n "$got" ] && [ "$got" != "$want" ]; then die "verificação de integridade falhou (esperado $want, obtido $got)"; fi
  [ -n "$got" ] && say "Integridade verificada (SHA-256)."
else
  warn "esta versão não publicou checksum; pulando a verificação"
fi

# --- instalação -------------------------------------------------------------
tar -xzf "$tmp/$asset" -C "$tmp"
bin="$(find "$tmp" -type f -name gameflow | head -n 1)"
[ -n "$bin" ] || die "o pacote baixado não contém o executável"
mkdir -p "$INSTALL_DIR"
cp "$bin" "$INSTALL_DIR/gameflow"
chmod +x "$INSTALL_DIR/gameflow"
say "Instalado em $INSTALL_DIR/gameflow"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) warn "$INSTALL_DIR não está no seu PATH. Adicione esta linha ao ~/.bashrc, ~/.zshrc ou equivalente:"
     printf '    export PATH="%s:$PATH"\n' "$INSTALL_DIR" ;;
esac

say "Tudo pronto! Para começar, rode:  gameflow"
say "Para atualizar no futuro:        gameflow update"
