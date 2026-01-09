
.PHONY: all install

all:

install:
	bash create-env

create-blue:
	. .venv/bin/activate
	ansible-playbook playbooks/make-linode-hosts.yml
