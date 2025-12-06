let ws = null;

function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/dashboard/ws`;

    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        console.log('Dashboard WebSocket connected');
        document.getElementById('status').textContent = 'Connected';
        document.getElementById('status').style.color = 'green';
    };

    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        updateClientsTable(data.clients);
    };

    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        document.getElementById('status').textContent = 'Error';
        document.getElementById('status').style.color = 'red';
    };

    ws.onclose = () => {
        console.log('Dashboard WebSocket closed');
        document.getElementById('status').textContent = 'Disconnected - Reconnecting...';
        document.getElementById('status').style.color = 'orange';
        setTimeout(connect, 1000);
    };
}

function updateClientsTable(clients) {
    const tbody = document.getElementById('clients-body');

    if (clients.length === 0) {
        tbody.innerHTML = '<tr><td colspan="5">No clients connected</td></tr>';
        return;
    }

    tbody.innerHTML = '';

    for (const client of clients) {
        const row = document.createElement('tr');

        const formatMetric = (values) => {
            return values.map(v => v.toFixed(2)).join(' / ');
        };

        const formatBytes = (values) => {
            return values.map(v => (v / 1024).toFixed(2) + ' KB/s').join(' / ');
        };

        row.innerHTML = `
            <td>${client.id}</td>
            <td>${formatBytes(client.metrics.c2s_throughput)}</td>
            <td>${formatBytes(client.metrics.s2c_throughput)}</td>
            <td>${formatMetric(client.metrics.c2s_delay_avg)} ms</td>
            <td>${formatMetric(client.metrics.s2c_delay_avg)} ms</td>
        `;

        tbody.appendChild(row);
    }
}

// Connect when page loads
connect();
