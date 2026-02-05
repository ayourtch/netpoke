# Issue 046: Implement Client Upload Logic

## Summary
Implement JavaScript functions for chunked upload with SHA-256 verification and resume capability.

## Location
- File: `server/static/nettest.html`

## Current Behavior
Upload button exists but does nothing when clicked.

## Expected Behavior
Client-side JavaScript that:
1. Calculates SHA-256 checksums using Web Crypto API
2. Calls prepare endpoint to get existing chunk info
3. Uploads only missing/changed chunks
4. Shows progress during upload
5. Calls finalize endpoint to verify completion
6. Updates UI on success or failure

## Impact
Enables reliable, resumable uploads from the browser to the server.

## Suggested Implementation

### Step 1: Add SHA-256 helper function

```javascript
const CHUNK_SIZE = 1048576; // 1 MB

async function calculateSHA256(data) {
    // data can be ArrayBuffer or Uint8Array
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}
```

### Step 2: Implement startUpload function

```javascript
async function startUpload(recordingId) {
    const recording = await getRecordingFromIndexedDB(recordingId);
    if (!recording) {
        alert('Recording not found');
        return;
    }

    const button = document.getElementById(`upload-btn-${recordingId}`);
    const progressDiv = document.getElementById(`progress-${recordingId}`);
    const progressFill = progressDiv.querySelector('.progress-fill');
    const progressText = progressDiv.querySelector('.progress-text');

    // Disable button and show progress
    button.disabled = true;
    button.classList.add('uploading');
    progressDiv.style.display = 'block';
    progressText.textContent = 'Preparing upload...';

    try {
        // Step 1: Prepare upload
        const sensorBlob = new Blob([JSON.stringify(recording.sensors)], 
                                     { type: 'application/json' });
        
        const deviceInfo = {
            browser: navigator.userAgent,
            os: navigator.platform,
            screen_width: window.screen.width,
            screen_height: window.screen.height
        };

        const prepareReq = {
            session_id: surveySessionId,
            recording_id: recordingId,
            video_size_bytes: recording.video.size,
            sensor_size_bytes: sensorBlob.size,
            device_info: deviceInfo,
            user_notes: recording.notes || null
        };

        const prepareResp = await fetch('/api/upload/prepare', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(prepareReq)
        });

        if (!prepareResp.ok) {
            throw new Error(`Prepare failed: ${prepareResp.status}`);
        }
        const prepareData = await prepareResp.json();

        // Step 2: Upload video chunks
        progressText.textContent = 'Uploading video...';
        await uploadFile(
            recording.video,
            recordingId,
            'video',
            prepareData.video_chunks,
            (progress) => {
                progressFill.style.width = `${progress}%`;
                progressText.textContent = `Uploading video: ${progress}%`;
            }
        );

        // Step 3: Upload sensor data
        progressText.textContent = 'Uploading sensors...';
        await uploadFile(
            sensorBlob,
            recordingId,
            'sensor',
            prepareData.sensor_chunks,
            (progress) => {
                progressFill.style.width = `${progress}%`;
                progressText.textContent = `Uploading sensors: ${progress}%`;
            }
        );

        // Step 4: Finalize
        progressText.textContent = 'Verifying upload...';
        const videoChecksums = await calculateAllChunkChecksums(recording.video);
        const sensorChecksums = await calculateAllChunkChecksums(sensorBlob);

        const videoFinalChecksum = await calculateSHA256(
            new TextEncoder().encode(videoChecksums.join(''))
        );
        const sensorFinalChecksum = await calculateSHA256(
            new TextEncoder().encode(sensorChecksums.join(''))
        );

        const finalizeResp = await fetch('/api/upload/finalize', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                recording_id: recordingId,
                video_final_checksum: videoFinalChecksum,
                sensor_final_checksum: sensorFinalChecksum
            })
        });

        if (!finalizeResp.ok) {
            throw new Error(`Finalize failed: ${finalizeResp.status}`);
        }

        const finalizeData = await finalizeResp.json();
        if (finalizeData.status !== 'complete') {
            throw new Error('Verification failed');
        }

        // Success
        progressFill.style.width = '100%';
        progressText.textContent = '✓ Upload complete';
        button.textContent = '✓ Uploaded';
        button.classList.remove('uploading');
        button.classList.add('success');

    } catch (error) {
        console.error('Upload failed:', error);
        progressText.textContent = `⚠️ Upload failed: ${error.message}`;
        button.disabled = false;
        button.classList.remove('uploading');
        button.classList.add('error');
        button.textContent = '⚠️ Retry Upload';
    }
}
```

### Step 3: Implement uploadFile function

```javascript
async function uploadFile(blob, recordingId, fileType, existingChunks, onProgress) {
    const totalChunks = Math.ceil(blob.size / CHUNK_SIZE);

    for (let i = 0; i < totalChunks; i++) {
        const start = i * CHUNK_SIZE;
        const end = Math.min(start + CHUNK_SIZE, blob.size);
        const chunk = blob.slice(start, end);
        const chunkData = await chunk.arrayBuffer();
        const checksum = await calculateSHA256(chunkData);

        // Skip if server already has this chunk with matching checksum
        if (existingChunks && existingChunks[i] && 
            existingChunks[i].checksum === checksum) {
            onProgress(Math.round(((i + 1) / totalChunks) * 100));
            continue;
        }

        // Upload chunk with retry
        let retries = 3;
        while (retries > 0) {
            try {
                const response = await fetch('/api/upload/chunk', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/octet-stream',
                        'X-Recording-Id': recordingId,
                        'X-File-Type': fileType,
                        'X-Chunk-Index': i.toString(),
                        'X-Chunk-Checksum': checksum
                    },
                    body: chunkData
                });

                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}`);
                }
                break; // Success, exit retry loop
            } catch (err) {
                retries--;
                if (retries === 0) throw err;
                await new Promise(r => setTimeout(r, 1000)); // Wait 1s before retry
            }
        }

        onProgress(Math.round(((i + 1) / totalChunks) * 100));
    }
}
```

### Step 4: Implement calculateAllChunkChecksums function

```javascript
async function calculateAllChunkChecksums(blob) {
    const checksums = [];
    const totalChunks = Math.ceil(blob.size / CHUNK_SIZE);

    for (let i = 0; i < totalChunks; i++) {
        const start = i * CHUNK_SIZE;
        const end = Math.min(start + CHUNK_SIZE, blob.size);
        const chunk = blob.slice(start, end);
        const chunkData = await chunk.arrayBuffer();
        const checksum = await calculateSHA256(chunkData);
        checksums.push(checksum);
    }

    return checksums;
}
```

## Testing
- Upload button triggers upload flow
- Progress bar updates during upload
- Upload completes successfully for small test files
- Upload resumes if interrupted and retried
- Error states display correctly

## Dependencies
- Issue 044: Add upload API routes
- Issue 045: Add upload UI to nettest.html

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 14 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Upload Protocol section.

---
*Created: 2026-02-05*
