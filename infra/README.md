

ansible-playbook playbooks/make-linode-hosts.yml

scripts/generate-zone

when the hosts come up:

ansible-playbook playbooks/setup-linode-hosts.yml

