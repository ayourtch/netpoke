sandbox	60	A	66.175.209.100
sandbox	60	AAAA	2600:3c03::2000:51ff:fe78:496a
www1	60	A	66.175.209.104
www1	60	AAAA	2600:3c03::2000:64ff:fee9:16b1
www	60	CNAME	www1.netpoke.com
@ 60	CNAME	www1.netpoke.com
