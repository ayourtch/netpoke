.PHONY: all build build-wasm build-server install-service restart pull update-server

all: build restart

# Build WASM client first, then the server (so WASM is embedded)
build: build-wasm build-server
	@echo "Build complete. Single executable at target/release/netpoke-server"

# Build the WASM client to be embedded in the server
build-wasm:
	(cd client && ./build.sh)

# Build the server (includes embedded static files and WASM)
build-server:
	cargo build --release -p netpoke-server
	-sudo /usr/sbin/setcap cap_net_raw=+ep target/release/netpoke-server

update-server: pull build restart
	echo "Server updated!"

pull:
	git pull


build-everything: netpoke-server

netpoke-server: docker
	docker run --rm -v $$(pwd):/out netpoke-builder:latest cp /netpoke/target/release/netpoke /out/

docker:
	docker build -t netpoke-build:latest . --progress=plain

install-service:
	sudo cp misc/netpoke.service /etc/systemd/system/
	sudo systemctl daemon-reload
	sudo systemctl enable netpoke
	sudo systemctl restart netpoke

restart:
	sudo systemctl stop netpoke
	sudo systemctl start netpoke

