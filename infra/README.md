

ansible-playbook playbooks/make-linode-hosts.yml

scripts/generate-zone

when the hosts come up:

ansible-playbook playbooks/setup-linode-hosts.yml





certbot certonly --standalone -d sandbox.netpoke.com -m ayourtch@gmail.com --agree-tos


uccessfully received certificate.
Certificate is saved at: /etc/letsencrypt/live/sandbox.netpoke.com/fullchain.pem
Key is saved at:         /etc/letsencrypt/live/sandbox.netpoke.com/privkey.pem
This certificate expires on 2026-04-11.
These files will be updated when the certificate renews.



localhost:/etc/netpoke/certs# ln -s /etc/letsencrypt/live/sandbox.netpoke.com/fullchain.pem server.
crt
localhost:/etc/netpoke/certs# ln -s /etc/letsencrypt/live/sandbox.netpoke.com/privkey.pem server.ke


echo Y | certbot certonly --standalone -d netpoke.com -d www.netpoke.com -d www1.netpo
ke.com -m ayourtch@gmail.com --agree-tos

localhost:~# rm /etc/netpoke/certs/server.crt
localhost:~# rm /etc/netpoke/certs/server.key
localhost:~# ln -s /etc/letsencrypt/live/netpoke.com/fullchain.pem /etc/netpoke/certs/server.crt
localhost:~# ln -s /etc/letsencrypt/live/netpoke.com/privkey.pem /etc/netpoke/certs/server.key



