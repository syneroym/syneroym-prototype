// Service Worker Logic
const SIGNALING_SERVER_URL = "{{ signaling_server_url }}";
const TARGET_PEER_ID = "{{ target_peer_id }}";
const HTTP_VERSION = "{{ http_version }}";
const MY_ID = "gateway-" + Math.random().toString(36).substr(2, 9);

let peerConnection;
let ws;
let isConnected = false;
let connectionPromise = null;

async function bootstrap() {
    if (isConnected) return;
    if (connectionPromise) return connectionPromise;

    connectionPromise = new Promise((resolve, reject) => {
        console.log("[SW] Connecting to Signaling Server:", SIGNALING_SERVER_URL);
        ws = new WebSocket(SIGNALING_SERVER_URL);

        ws.onopen = () => {
            console.log("[SW] WS Open. Registering as:", MY_ID);
            ws.send(JSON.stringify({ type: "register", id: MY_ID }));
            startWebRTC(resolve, reject);
        };

        ws.onmessage = async (event) => {
            const msg = JSON.parse(event.data);
            handleSignalingMessage(msg);
        };

        ws.onerror = (e) => {
            console.error("[SW] WS Error:", e);
            reject(e);
        };
    });

    return connectionPromise;
}

async function startWebRTC(resolve, reject) {
    console.log("[SW] Starting WebRTC...");
    const config = {
        iceServers: [{ urls: "stun:stun.l.google.com:19302" }]
    };

    peerConnection = new RTCPeerConnection(config);

    peerConnection.onicecandidate = (event) => {
        if (event.candidate) {
            // TODO: We use Vanilla ICE (wait for all candidates), so we don't send them individually.
            // They will be included in the final SDP.
            // Can implement Trickle ICE later if needed.
        }
    };

    peerConnection.onconnectionstatechange = () => {
        console.log("[SW] Connection State:", peerConnection.connectionState);
        if (peerConnection.connectionState === 'connected') {
            isConnected = true;
            resolve();
        }
    };

    // Create Offer
    const offer = await peerConnection.createOffer();
    await peerConnection.setLocalDescription(offer);

    // Wait for ICE Gathering to complete (Vanilla ICE)
    // This ensures all candidates are included in the SDP
    if (peerConnection.iceGatheringState !== 'complete') {
        await new Promise(resolve => {
            const checkState = () => {
                if (peerConnection.iceGatheringState === 'complete') {
                    peerConnection.removeEventListener('icegatheringstatechange', checkState);
                    resolve();
                }
            };
            peerConnection.addEventListener('icegatheringstatechange', checkState);
        });
    }

    console.log("[SW] Sending Offer to:", TARGET_PEER_ID);
    ws.send(JSON.stringify({
        type: "offer",
        target: TARGET_PEER_ID,
        sender: MY_ID,
        sdp: peerConnection.localDescription.sdp
    }));
}

async function handleSignalingMessage(msg) {
    console.log("[SW] Signaling Msg:", msg.type);
    switch (msg.type) {
        case "answer":
            await peerConnection.setRemoteDescription(new RTCSessionDescription({
                type: "answer",
                sdp: msg.sdp
            }));
            break;
        case "candidate":
            if (msg.candidate) {
                await peerConnection.addIceCandidate(msg.candidate);
            }
            break;
    }
}

// --------------------------------------------------------------------------
// Request/Response Handling
// --------------------------------------------------------------------------

self.addEventListener('install', (event) => {
    console.log('[SW] Installing');
    self.skipWaiting();
    event.waitUntil(bootstrap().catch(err => console.error("[SW] Bootstrap failed:", err)));
});

self.addEventListener('activate', (event) => {
    console.log('[SW] Activating');
    event.waitUntil(clients.claim());
});

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    if (url.searchParams.has('sw')) return;
    if (url.pathname === '/sw.js') return;

    event.respondWith(
        (async () => {
            try {
                await bootstrap();

                const hostname = url.hostname;
                const parts = hostname.split('.');
                let serviceName = parts.length > 0 ? parts[0] : "default";

                console.log(`[SW] Proxying ${event.request.method} ${url.pathname} to ${serviceName}`);

                return await sendRequest(serviceName, event.request);

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

async function sendRequest(serviceName, request) {
    return new Promise((resolve, reject) => {
        // Create a dedicated DataChannel for this request
        const dcLabel = "req-" + Math.random().toString(36).substr(2, 5);
        const dc = peerConnection.createDataChannel(dcLabel);

        let responseController = null;
        let responseBuffer = [];
        let headersParsed = false;

        dc.onopen = async () => {
            console.log(`[SW] Channel ${dcLabel} open. sending request...`);

            try {
                // 1. Send Preamble
                const serviceBytes = new TextEncoder().encode(serviceName);
                const preamble = new Uint8Array(1 + serviceBytes.length);
                preamble[0] = serviceBytes.length;
                preamble.set(serviceBytes, 1);
                dc.send(preamble);

                // 2. Send Request Line & Headers
                const method = request.method;
                const path = new URL(request.url).pathname + new URL(request.url).search;
                const headers = [];
                for (const [k, v] of request.headers) {
                    headers.push(`${k}: ${v}`);
                }
                let reqStr = `${method} ${path} ${HTTP_VERSION}\r\n`;
                reqStr += headers.join('\r\n') + '\r\n\r\n';
                dc.send(new TextEncoder().encode(reqStr));

                // 3. Stream Body (Upload)
                if (request.body) {
                    const reader = request.body.getReader();
                    while (true) {
                        const { done, value } = await reader.read();
                        if (done) break;
                        dc.send(value);
                    }
                }

                // Note: We don't close the channel here because we need to receive the response.
                // The server will close it when it's done sending.

            } catch (e) {
                console.error(`[SW] Failed to send request on ${dcLabel}:`, e);
                dc.close();
                reject(e);
            }
        };

        dc.onmessage = (event) => {
            const chunk = new Uint8Array(event.data);

            if (responseController) {
                // Streaming body
                responseController.enqueue(chunk);
            } else {
                // Buffering headers
                responseBuffer.push(...chunk);
                const endpoint = findDoubleCRLF(responseBuffer);
                if (endpoint !== -1) {
                    const headerBytes = new Uint8Array(responseBuffer.slice(0, endpoint));
                    const bodyBytes = new Uint8Array(responseBuffer.slice(endpoint + 4));

                    const headerStr = new TextDecoder().decode(headerBytes);
                    const { status, headers } = parseHeaders(headerStr);

                    const stream = new ReadableStream({
                        start(controller) {
                            responseController = controller;
                            if (bodyBytes.length > 0) {
                                controller.enqueue(bodyBytes);
                            }
                        },
                        cancel() {
                            dc.close();
                        }
                    });

                    headersParsed = true;
                    resolve(new Response(stream, { status, headers }));
                    responseBuffer = []; // clear memory
                }
            }
        };

        dc.onclose = () => {
            console.log(`[SW] Channel ${dcLabel} closed.`);
            if (responseController) {
                responseController.close();
            } else if (!headersParsed) {
                reject(new Error("Connection closed before response headers received"));
            }
        };

        dc.onerror = (e) => {
            console.error(`[SW] Channel ${dcLabel} error:`, e);
            if (responseController) {
                responseController.error(e);
            } else {
                reject(e);
            }
        };
    });
}

function findDoubleCRLF(buffer) {
    for (let i = 0; i < buffer.length - 3; i++) {
        if (buffer[i] === 13 && buffer[i + 1] === 10 && buffer[i + 2] === 13 && buffer[i + 3] === 10) {
            return i;
        }
    }
    return -1;
}

function parseHeaders(headerStr) {
    const lines = headerStr.split('\r\n');
    const statusLine = lines[0];
    const statusMatch = statusLine.match(/^HTTP\/[0-9.]+\s+(\d+)\s+(.+)$/);
    const status = statusMatch ? parseInt(statusMatch[1]) : 200;

    const headers = new Headers();
    for (let i = 1; i < lines.length; i++) {
        const line = lines[i];
        if (!line) continue;
        const colon = line.indexOf(':');
        if (colon > 0) {
            headers.append(line.substring(0, colon).trim(), line.substring(colon + 1).trim());
        }
    }
    return { status, headers };
}
