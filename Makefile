.PHONY: all build install-service restart pull update-server

all: build restart

build:
	cargo build --release
	(cd client && ./build.sh)
	sudo /usr/sbin/setcap cap_net_raw=+ep /home/netpoke/wifi-verify/target/release/wifi-verify-server

update-server: pull build restart
	echo "Server updated!"

pull:
	git pull


install-service:
	sudo cp misc/netpoke.service /etc/systemd/system/
	sudo systemctl daemon-reload
	sudo systemctl enable netpoke
	sudo systemctl restart netpoke

restart:
	sudo systemctl stop netpoke
	sudo systemctl start netpoke

