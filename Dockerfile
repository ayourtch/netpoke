FROM ghcr.io/linuxcontainers/alpine:latest AS build

RUN apk add git make curl libgcc clang libressl-dev libpcap-dev sudo libcap-setcap bash
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
WORKDIR /netpoke
COPY ./Makefile.in-docker /netpoke/

RUN . "${HOME}/.cargo/env" && make -f Makefile.in-docker in-docker-prep
COPY . /netpoke
RUN . "${HOME}/.cargo/env" && make build
RUN mkdir result && cp target/release/netpoke-server result/
RUN . "${HOME}/.cargo/env" && cargo clean


