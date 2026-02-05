# Issue 045: Add Upload UI to nettest.html

## Summary
Add upload button, progress bar, and edit notes UI elements to the recordings list in nettest.html.

## Location
- File: `server/static/nettest.html`

## Current Behavior
Recording items show play, download, and delete buttons but no upload functionality.

## Expected Behavior
Each recording item should display:
1. Upload button (green, changes state based on upload progress)
2. Progress bar (hidden initially, shows during upload)
3. Edit notes button
4. Upload status indicator

## Impact
Enables surveyors to upload recordings to the server with visual feedback.

## Suggested Implementation

### Step 1: Add CSS for upload UI

Add to the `<style>` section:

```css
.upload-progress {
    margin-top: 8px;
    width: 100%;
}

.progress-bar {
    width: 100%;
    height: 20px;
    background-color: #e0e0e0;
    border-radius: 10px;
    overflow: hidden;
}

.progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #2196F3, #4CAF50);
    transition: width 0.3s ease;
}

.progress-text {
    font-size: 12px;
    color: #666;
    margin-top: 4px;
    display: block;
}

.upload-btn.uploading {
    opacity: 0.6;
    cursor: not-allowed;
}

.upload-btn.success {
    background-color: #4CAF50;
    color: white;
}

.upload-btn.error {
    background-color: #f44336;
    color: white;
}

.recording-notes {
    font-size: 12px;
    color: #666;
    margin-top: 4px;
    font-style: italic;
}

.recording-notes:empty::before {
    content: "No notes";
    color: #999;
}
```

### Step 2: Update recording item template

Find the function that creates recording items (likely `renderRecordingsList` or similar) and update to include:

```html
<div class="recording-item" data-recording-id="${recording.id}">
  <div class="recording-item-info">
    <strong>Recording ${index + 1}</strong>
    <span>${formatDate(recording.timestamp)}</span>
    <span>Video: ${formatBytes(recording.videoSize)}, Sensors: ${formatBytes(recording.sensorSize)}</span>
    <div class="recording-notes" id="notes-${recording.id}">${recording.notes || ''}</div>
  </div>

  <div class="recording-item-actions">
    <button onclick="editNotes('${recording.id}')" class="capture-btn blue">
      ✏️ Notes
    </button>
    <button onclick="startUpload('${recording.id}')"
            id="upload-btn-${recording.id}"
            class="capture-btn green upload-btn"
            data-recording-id="${recording.id}">
      📤 Upload
    </button>
    <button onclick="playRecording('${recording.id}')" class="capture-btn purple">
      ▶️ Play
    </button>
    <button onclick="downloadRecording('${recording.id}')" class="capture-btn">
      💾 Save
    </button>
    <button onclick="deleteRecording('${recording.id}')" class="capture-btn red">
      🗑️ Delete
    </button>
  </div>

  <!-- Progress bar (hidden initially) -->
  <div class="upload-progress" id="progress-${recording.id}" style="display:none;">
    <div class="progress-bar">
      <div class="progress-fill" style="width: 0%"></div>
    </div>
    <span class="progress-text">Preparing upload...</span>
  </div>
</div>
```

### Step 3: Add editNotes function

```javascript
async function editNotes(recordingId) {
    const notesDiv = document.getElementById(`notes-${recordingId}`);
    const currentNotes = notesDiv.textContent || '';
    const newNotes = prompt('Enter notes for this recording:', currentNotes);
    
    if (newNotes !== null) {
        // Update in IndexedDB
        const recording = await getRecordingFromIndexedDB(recordingId);
        if (recording) {
            recording.notes = newNotes;
            await saveRecordingToIndexedDB(recording);
            notesDiv.textContent = newNotes;
        }
    }
}
```

## Button States
- **Default:** "📤 Upload" (green)
- **Uploading:** Button disabled, opacity reduced
- **Complete:** "✓ Uploaded" (green with success class)
- **Failed:** "⚠️ Retry Upload" (red with error class)

## Testing
- Recording list displays with upload buttons
- CSS styling appears correctly
- Edit notes button opens prompt dialog
- Progress bar is hidden by default

## Dependencies
- None (UI only, no backend dependency)

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 13 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - User Interface Design section.

---
*Created: 2026-02-05*
