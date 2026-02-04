# Issue 026: Missing Recordings List Implementation

## Summary
The `refreshRecordingsList()` function in `nettest.html` is a placeholder that doesn't actually query IndexedDB or populate the recordings list UI. This means users cannot see their saved recordings after creating them.

## Location
- File: `server/static/nettest.html`
- Function: `refreshRecordingsList()` (line ~2762)
- Related: `#recordings-container` div (line ~980)

## Current Behavior
The `refreshRecordingsList()` function only logs to the console and does nothing else:
```javascript
async function refreshRecordingsList() {
    console.log('[Recorder] Refreshing recordings list');
    // This is a placeholder - the actual implementation would query IndexedDB
    // and update the #recordings-container element with the list
    // For now, just log that it was called
    const container = document.getElementById('recordings-container');
    if (container) {
        console.log('[Recorder] Recordings list refresh requested');
    }
}
```

When recordings are saved (in `client/src/recorder/state.rs` line 381-391), this function is called but doesn't update the UI.

## Expected Behavior
The function should:
1. Query IndexedDB via the existing `getRecording` and related functions
2. Retrieve all saved recordings
3. Generate HTML for each recording with proper metadata (ID, date, duration, frames, size, source type)
4. Attach onclick handlers for download and delete actions
5. Update the `#recordings-container` with the generated HTML

Reference implementation from `tmp/camera-standalone-for-cross-check/camera-tracker.html` (~lines 244-265):
```javascript
async function updateRecordingsList() {
    const recordings = await getAllRecordings();
    if (recordings.length === 0) {
        recordingsListEl.innerHTML = '<p style="color:#888;">No recordings yet</p>';
        return;
    }
    recordingsListEl.innerHTML = recordings.map(rec => {
        const date = new Date(rec.timestamp);
        const sizeInMB = (rec.videoBlob.size / (1024 * 1024)).toFixed(2);
        const sourceType = rec.metadata.sourceType || 'camera';
        const sourceClass = `source-${sourceType}`;
        const sourceLabel = sourceType.charAt(0).toUpperCase() + sourceType.slice(1);
        return `
            <div class="recording-item">
                <div class="data">ID: ${rec.id} <span class="source-label ${sourceClass}">${sourceLabel}</span></div>
                <div class="data">Date: ${date.toLocaleString()}</div>
                <div class="data">Duration: ${rec.metadata.duration.toFixed(1)}s</div>
                <div class="data">Frames: ${rec.metadata.frameCount}</div>
                <div class="data">Size: ${sizeInMB} MB</div>
                <button onclick="downloadVideo('${rec.id}')">Download Video</button>
                <button onclick="downloadMotionData('${rec.id}')">Download Motion Data</button>
                <button class="danger" onclick="deleteRecordingById('${rec.id}')">Delete</button>
            </div>
        `;
    }).join('');
}
```

## Impact
**High** - Users cannot see, manage, or download their recordings. This is a critical gap that makes the recording feature essentially unusable for end users.

## Suggested Implementation

1. **In `server/static/nettest.html`**, replace the placeholder `refreshRecordingsList()` with:

```javascript
async function refreshRecordingsList() {
    console.log('[Recorder] Refreshing recordings list');
    
    const container = document.getElementById('recordings-container');
    if (!container) {
        console.warn('[Recorder] recordings-container not found');
        return;
    }
    
    try {
        // Use existing WASM storage module to get all recordings
        const dbName = 'CameraTrackingDB';
        const storeName = 'recordings';
        
        // Open IndexedDB
        const db = await new Promise((resolve, reject) => {
            const request = indexedDB.open(dbName, 2);
            request.onerror = () => reject(request.error);
            request.onsuccess = () => resolve(request.result);
        });
        
        // Get all recordings
        const recordings = await new Promise((resolve, reject) => {
            const transaction = db.transaction([storeName], 'readonly');
            const store = transaction.objectStore(storeName);
            const request = store.getAll();
            request.onsuccess = () => resolve(request.result);
            request.onerror = () => reject(request.error);
        });
        
        // Close database
        db.close();
        
        if (recordings.length === 0) {
            container.innerHTML = '<p style="color:#888;">No recordings yet. Start a recording to see it listed here.</p>';
            return;
        }
        
        // Generate HTML for each recording
        container.innerHTML = recordings
            .sort((a, b) => b.timestamp - a.timestamp) // Most recent first
            .map(rec => {
                const date = new Date(rec.timestamp);
                const sizeInMB = (rec.videoBlob.size / (1024 * 1024)).toFixed(2);
                const sourceType = rec.metadata.sourceType || 'camera';
                const sourceClass = `source-${sourceType}`;
                const sourceLabel = sourceType.charAt(0).toUpperCase() + sourceType.slice(1);
                
                return `
                    <div class="recording-item">
                        <div class="recording-info">
                            <div class="data"><strong>ID:</strong> ${rec.id} <span class="source-label ${sourceClass}">${sourceLabel}</span></div>
                            <div class="data"><strong>Date:</strong> ${date.toLocaleString()}</div>
                            <div class="data"><strong>Duration:</strong> ${rec.metadata.duration.toFixed(1)}s</div>
                            <div class="data"><strong>Frames:</strong> ${rec.metadata.frameCount}</div>
                            <div class="data"><strong>Size:</strong> ${sizeInMB} MB</div>
                        </div>
                        <div class="recording-actions">
                            <button onclick="window.downloadVideoWrapper('${rec.id}')" class="btn-download">Download Video</button>
                            <button onclick="window.downloadMotionDataWrapper('${rec.id}')" class="btn-download">Download Motion</button>
                            <button onclick="window.deleteRecordingWrapper('${rec.id}')" class="btn-delete danger">Delete</button>
                        </div>
                    </div>
                `;
            }).join('');
            
        console.log(`[Recorder] Displayed ${recordings.length} recording(s)`);
        
    } catch (error) {
        console.error('[Recorder] Error refreshing recordings list:', error);
        container.innerHTML = `<p style="color:#f44;">Error loading recordings: ${error.message}</p>`;
    }
}
```

2. **Add wrapper functions** for the WASM exports (also in `nettest.html`):

```javascript
// Wrapper functions to bridge HTML onclick handlers to WASM functions
window.downloadVideoWrapper = async function(id) {
    try {
        await download_video(id);
    } catch (error) {
        console.error('[Recorder] Error downloading video:', error);
        alert('Error downloading video: ' + error.message);
    }
};

window.downloadMotionDataWrapper = async function(id) {
    try {
        await download_motion_data(id);
    } catch (error) {
        console.error('[Recorder] Error downloading motion data:', error);
        alert('Error downloading motion data: ' + error.message);
    }
};

window.deleteRecordingWrapper = async function(id) {
    try {
        await delete_recording_by_id(id);
        // List will be refreshed by the WASM code after successful deletion
    } catch (error) {
        console.error('[Recorder] Error deleting recording:', error);
        alert('Error deleting recording: ' + error.message);
    }
};
```

3. **Call `refreshRecordingsList()` on page load** to show any existing recordings:

```javascript
// After WASM module is loaded and initialized, refresh the list
if (window.refreshRecordingsList) {
    await refreshRecordingsList();
}
```

## Related Issues
- Issue 011 (resolved): Recordings list not refreshed after save - partially addressed but implementation was incomplete
- Issue 012 (resolved): Missing download functions - functions exist but not wired to UI

---
*Created: 2026-02-04*
