FROM ghcr.io/linuxcontainers/alpine:latest AS build

RUN apk add git make curl libgcc clang libressl-dev libpcap-dev sudo
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN <<EOF
  . "$HOME/.cargo/env"
  cargo install wasm-pack

  MACH="$(uname -m)"
  RTARG="${MACH}-unknown-linux-gnu"
  rustup target add ${RTARG}
  # set libc 
  #echo "[build]" > ~/.cargo/config.toml
  # echo "target = \"${RTARG}\"" >>~/.cargo/config.toml
EOF

WORKDIR /netpoke

COPY . /netpoke

RUN <<EOF
  . "$HOME/.cargo/env"
  make build
EOF

