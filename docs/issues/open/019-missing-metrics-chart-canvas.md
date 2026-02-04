# Issue 019: Missing Metrics Chart Canvas Element

## Summary
The `initChart()` function tries to get a canvas element with id `metrics-chart`, but this element doesn't exist in the HTML. This causes a TypeError when Chart.js tries to initialize.

## Location
- File: `server/static/nettest.html`
- Function: `initChart()` (line 1190)
- Call site: Page initialization (line 1425)

## Current Behavior
Browser console error:
```
[Error] TypeError: null is not an object (evaluating 'document.getElementById('metrics-chart').getContext')
	initChart (nettest.html:1191)
	(anonymous function) (nettest.html:1425)
```

The code tries to initialize a chart:
```javascript
function initChart() {
    const ctx = document.getElementById('metrics-chart').getContext('2d');
    // ...
}
```

But there's no `<canvas id="metrics-chart">` element in the HTML. There is only a `<select>` dropdown option referencing this chart:
```html
<select id="chart-type">
    <option value="metrics-chart">Metrics Chart</option>
    <option value="latency-chart">Latency Chart</option>
    <option value="throughput-chart">Throughput Chart</option>
</select>
```

## Expected Behavior
The HTML should contain canvas elements for each chart type that can be overlaid on recordings:
```html
<canvas id="metrics-chart" style="display:none"></canvas>
<canvas id="latency-chart" style="display:none"></canvas>
<canvas id="throughput-chart" style="display:none"></canvas>
```

These canvases are used by:
1. Chart.js to render the actual charts
2. The recorder's `render_chart_overlay()` function to capture chart images for overlaying on video

## Impact
- **Priority: Medium**
- Chart initialization fails on page load
- Chart overlay feature in recordings will not work
- No visible charts available for compositing into recordings
- Feature is incomplete despite UI controls being present

## Suggested Implementation

1. **Add canvas elements to HTML** (after the chart controls):
```html
<!-- Chart Overlay Controls -->
<div id="chart-controls" class="control-group">
    <!-- existing controls -->
</div>

<!-- Hidden chart canvases for Chart.js and recording overlay -->
<div style="display:none">
    <canvas id="metrics-chart" width="400" height="300"></canvas>
    <canvas id="latency-chart" width="400" height="300"></canvas>
    <canvas id="throughput-chart" width="400" height="300"></canvas>
</div>
```

2. **Update initChart() to handle multiple charts**:
```javascript
function initChart() {
    // Initialize all charts
    initMetricsChart();
    initLatencyChart();
    initThroughputChart();
}

function initMetricsChart() {
    const ctx = document.getElementById('metrics-chart');
    if (!ctx) {
        console.warn('metrics-chart canvas not found');
        return;
    }
    
    window.metricsChart = new Chart(ctx.getContext('2d'), {
        type: 'line',
        data: {
            labels: [],
            datasets: [{
                label: 'Network Metrics',
                data: [],
                borderColor: 'rgb(75, 192, 192)',
                tension: 0.1
            }]
        },
        options: {
            responsive: false,
            maintainAspectRatio: true,
            animation: false
        }
    });
}

// Similar for initLatencyChart() and initThroughputChart()
```

3. **Add null checks in recorder code**:
The recorder's `render_chart_overlay()` should verify the chart element exists before trying to capture it.

## Related
- The chart selection dropdown exists (line 942)
- The chart overlay rendering code exists in `client/src/recorder/canvas_renderer.rs`
- Issue 016 addresses chart dimension calculations

---
*Created: 2026-02-04*
