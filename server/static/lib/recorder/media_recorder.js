const recorders = new Map();
let nextId = 0;

export function createMediaRecorder(stream) {
    const id = `recorder_${nextId++}`;
    const chunks = [];

    // Codec detection with audio support
    // Include audio codecs for proper audio recording
    const codecs = [
        'video/webm;codecs=vp9,opus',     // VP9 with Opus audio (best quality)
        'video/webm;codecs=vp8,opus',     // VP8 with Opus audio
        'video/webm',                      // WebM default (browser chooses)
        'video/mp4;codecs=avc1.42E01E,mp4a.40.2',  // H.264 Baseline + AAC
        'video/mp4;codecs=avc1.4D401E,mp4a.40.2',  // H.264 Main + AAC
        'video/mp4',                       // MP4 default
    ];

    const supported = codecs.filter(codec =>
        MediaRecorder.isTypeSupported(codec)
    );

    console.log('Supported codecs:', supported);

    let selectedMimeType = '';
    let recorderOptions = {
        videoBitsPerSecond: 4000000
    };

    if (supported.length > 0) {
        // Prefer WebM with Opus on all platforms for best audio compatibility
        const webmWithOpus = supported.find(c => c.includes('webm') && c.includes('opus'));
        const webmDefault = supported.find(c => c === 'video/webm');
        const mp4WithAac = supported.find(c => c.includes('mp4') && c.includes('mp4a'));
        const mp4Default = supported.find(c => c === 'video/mp4');

        selectedMimeType = webmWithOpus || webmDefault || mp4WithAac || mp4Default || supported[0];
        recorderOptions.mimeType = selectedMimeType;
        console.log('Selected codec:', selectedMimeType);
    } else {
        // Let browser choose - don't specify mimeType
        console.log('No explicitly supported codecs, letting browser choose');
    }

    const recorder = new MediaRecorder(stream, recorderOptions);

    // Get actual mimeType from recorder
    const actualMimeType = recorder.mimeType || selectedMimeType || 'video/webm';
    console.log('Actual recorder mimeType:', actualMimeType);

    recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
            chunks.push(event.data);
        }
    };

    recorders.set(id, { recorder, chunks, mimeType: actualMimeType });

    return { id, mimeType: actualMimeType };
}

export function startRecorder(id) {
    const entry = recorders.get(id);
    if (!entry) throw new Error(`Recorder ${id} not found`);
    entry.recorder.start(1000);
}

export function stopRecorder(id) {
    const entry = recorders.get(id);
    if (!entry) throw new Error(`Recorder ${id} not found`);

    return new Promise((resolve) => {
        entry.recorder.onstop = () => {
            const blob = new Blob(entry.chunks, { type: entry.mimeType });
            resolve(blob);
            recorders.delete(id);
        };
        entry.recorder.stop();
    });
}

export function getRecorderState(id) {
    const entry = recorders.get(id);
    return entry ? entry.recorder.state : 'inactive';
}

export function getChunksSize(id) {
    const entry = recorders.get(id);
    if (!entry) return 0;
    return entry.chunks.reduce((sum, chunk) => sum + chunk.size, 0);
}
