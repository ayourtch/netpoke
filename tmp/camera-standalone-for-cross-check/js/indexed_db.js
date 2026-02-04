let db = null;

export async function openDb() {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open('CameraTrackingDB', 2);

        request.onerror = () => reject(request.error);
        request.onsuccess = () => {
            db = request.result;
            resolve();
        };

        request.onupgradeneeded = (event) => {
            const db = event.target.result;
            if (!db.objectStoreNames.contains('recordings')) {
                db.createObjectStore('recordings', { keyPath: 'id' });
            }
        };
    });
}

export async function saveRecording(id, videoBlob, metadata, motionData = []) {
    if (!db) throw new Error('Database not initialized');

    const recording = {
        id,
        videoBlob,
        metadata,
        motionData,
        timestamp: Date.now()
    };

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readwrite');
        const store = transaction.objectStore('recordings');
        const request = store.put(recording);

        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
    });
}

export async function getAllRecordings() {
    if (!db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readonly');
        const store = transaction.objectStore('recordings');
        const request = store.getAll();

        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}

export async function deleteRecording(id) {
    if (!db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readwrite');
        const store = transaction.objectStore('recordings');
        const request = store.delete(id);

        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
    });
}

export async function getRecording(id) {
    if (!db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readonly');
        const store = transaction.objectStore('recordings');
        const request = store.get(id);

        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}
