.PHONY: all build build-wasm build-server install-service restart pull update-server deploy-host

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

clean-out:
	rm -rf ./out || true
	mkdir ./out || true


build-everything: clean-out netpoke-server

netpoke-server-copy:
	docker run --rm -v $$(pwd)/out:/out netpoke-build:latest cp /netpoke/result/netpoke-server /out/
	cp out/netpoke-server out/netpoke-server-symbols
	strip out/netpoke-server

deploy-sandbox:
	cd infra && \
	UV_VENV_CLEAR=1 make install && \
	. .venv/bin/activate && \
	ansible-playbook playbooks/deploy-netpoke-on-sandbox.yml

setup-linode-all:
	cd infra && \
	UV_VENV_CLEAR=1 make install && \
	ansible-playbook playbooks/setup-linode-hosts.yml

netpoke-server: docker netpoke-server-copy
	echo "Netpoke server done!"

docker:
	docker build -t netpoke-build:latest . --progress=plain

install-bin:
	strip out/netpoke-server
	sudo cp out/netpoke-server /usr/local/bin/
	sudo /usr/sbin/setcap cap_net_raw=+ep /usr/local/bin/netpoke-server

install-service:
	sudo cp misc/netpoke.service /etc/systemd/system/
	sudo systemctl daemon-reload
	sudo systemctl enable netpoke
	sudo systemctl restart netpoke

restart:
	sudo systemctl stop netpoke
	sudo systemctl start netpoke

# Deploy to a specific host by FQDN
# Usage: make deploy-host HOST=www1.netpoke.com
deploy-host:
ifndef HOST
	$(error HOST is required. Usage: make deploy-host HOST=www1.netpoke.com)
endif
	cd infra && \
	UV_VENV_CLEAR=1 make install && \
	. .venv/bin/activate && \
	ansible-playbook playbooks/deploy-netpoke.yml -e "target_fqdn=$(HOST)"

