// Service Worker Logic

self.addEventListener('install', (event) => {
    console.log('[SW] Installing');
    self.skipWaiting();
});

self.addEventListener('activate', (event) => {
    console.log('[SW] Activating');
    event.waitUntil(clients.claim());
});

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    if (url.origin !== self.location.origin) return;
    if (url.searchParams.has('sw')) return;
    if (url.pathname === '/sw.js') return;
    console.log("[SW] ----- Starting overridden Fetch for", url)

    event.respondWith(
        (async () => {
            // Always serve App Shell for navigation to keep the proxy logic alive
            if (event.request.mode === 'navigate') {
                console.log("[SW] Navigation request detected. Serving App Shell.");
                return fetch(event.request);
            }

            try {
                // Find a client (window) to handle the WebRTC request
                const clientsList = await self.clients.matchAll({ includeUncontrolled: true, type: 'window' });
                const client = clientsList[0];

                if (!client) {
                    return new Response("<h1>Gateway Not Connected</h1><p>Please open the gateway page.</p>", {
                        status: 503,
                        headers: { 'Content-Type': 'text/html' }
                    });
                }

                return await proxyRequestToClient(client, event.request);

            } catch (err) {
                console.error("[SW] Proxy logic failed:", err);
                return new Response("<h1>Peer Proxy Error</h1><p>" + err.toString() + "</p>", {
                    status: 502,
                    headers: { 'Content-Type': 'text/html' }
                });
            }
        })()
    );
});

async function proxyRequestToClient(client, request) {
    return new Promise(async (resolve, reject) => {
        const channel = new MessageChannel();

        const headers = [];
        for (const [k, v] of request.headers) {
            headers.push([k, v]);
        }

        let body = null;
        if (request.body) {
            try {
                body = await request.arrayBuffer();
            } catch (e) {
                console.warn("[SW] Failed to read body", e);
            }
        }

        const msg = {
            type: 'FETCH_REQUEST',
            url: request.url,
            method: request.method,
            headers: headers,
            body: body
        };

        channel.port1.onmessage = (event) => {
            const data = event.data;
            console.log("[SW] Received DC response from client:", data);
            if (data.type === 'RESPONSE_HEAD') {
                const stream = new ReadableStream({
                    start(controller) {
                        channel.port1.onmessage = (evt) => {
                            console.log("[SW] Controller Received DC response from client:", evt);
                            if (evt.data.type === 'RESPONSE_CHUNK') {
                                controller.enqueue(evt.data.chunk);
                            } else if (evt.data.type === 'RESPONSE_END') {
                                controller.close();
                                channel.port1.close();
                            }
                        };
                    }
                });
                resolve(new Response(stream, { status: data.status, headers: new Headers(data.headers) }));
            } else if (data.type === 'ERROR') {
                resolve(new Response(data.message || "Unknown Error", { status: 502 }));
            }
        };

        client.postMessage(msg, [channel.port2]);
    });
}
