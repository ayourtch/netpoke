
.PHONY: all install

all:

install:
	curl -LsSf https://astral.sh/uv/install.sh | sh
	bash scripts/create-env

create-blue:
	. .venv/bin/activate
	ansible-playbook playbooks/make-linode-hosts.yml
