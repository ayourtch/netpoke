sandbox	60	A	45.79.160.152
sandbox	60	AAAA	2600:3c03::2000:45ff:fe63:19d6
www1	60	A	45.79.160.171
www1	60	AAAA	2600:3c03::2000:4aff:fe2d:e1cc
www	60	CNAME	www1.netpoke.com
@ 60	CNAME	www1.netpoke.com
