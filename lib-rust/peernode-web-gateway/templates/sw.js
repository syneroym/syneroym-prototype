// Service Worker Logic
const SIGNALING_SERVER_URL = "{{ signaling_server_url }}"; 
const TARGET_PEER_ID = "{{ target_peer_id }}";
const MY_ID = "gateway-" + Math.random().toString(36).substr(2, 9);

let peerConnection;
let dataChannel;
let ws;
let isConnected = false;
let connectionPromise = null;

// Queue for pending requests if channel isn't ready
const pendingRequests = [];

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

    // Create Data Channel
    dataChannel = peerConnection.createDataChannel("syneroym"); 
    setupDataChannel(dataChannel, resolve);

    peerConnection.onicecandidate = (event) => {
        if (event.candidate) {
            // Send candidate to peer logic here
        }
    };

    peerConnection.onconnectionstatechange = () => {
        console.log("[SW] Connection State:", peerConnection.connectionState); 
    };

    // Create Offer
    const offer = await peerConnection.createOffer();
    await peerConnection.setLocalDescription(offer);

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

function setupDataChannel(dc, resolve) {
    dc.onopen = () => {
        console.log("[SW] DataChannel Open!"); 
        isConnected = true;
        resolve();
    };
    dc.onmessage = handleDataChannelMessage;
    dc.onerror = (e) => console.error("[SW] DataChannel Error:", e);
}

// --------------------------------------------------------------------------
// Request/Response Handling
// --------------------------------------------------------------------------

let activeRequestResolve = null; 
let activeResponseStreamController = null; 
let responseBuffer = []; 

function handleDataChannelMessage(event) {
    const chunk = new Uint8Array(event.data);
    
    if (activeResponseStreamController) {
        activeResponseStreamController.enqueue(chunk);
    } else if (activeRequestResolve) {
        responseBuffer.push(...chunk);
        
        const endpoint = findDoubleCRLF(responseBuffer);
        if (endpoint !== -1) {
            const headerBytes = new Uint8Array(responseBuffer.slice(0, endpoint));
            const bodyBytes = new Uint8Array(responseBuffer.slice(endpoint + 4));
            
            const headerStr = new TextDecoder().decode(headerBytes);
            const { status, headers } = parseHeaders(headerStr);
            
            const stream = new ReadableStream({
                start(controller) {
                    activeResponseStreamController = controller;
                    if (bodyBytes.length > 0) {
                        controller.enqueue(bodyBytes);
                    }
                }
            });
            
            activeRequestResolve(new Response(stream, { status, headers }));
            responseBuffer = [];
        }
    }
}

function findDoubleCRLF(buffer) {
    for (let i = 0; i < buffer.length - 3; i++) {
        if (buffer[i]===13 && buffer[i+1]===10 && buffer[i+2]===13 && buffer[i+3]===10) {
            return i;
        }
    }
    return -1;
}

function parseHeaders(headerStr) {
    const lines = headerStr.split('\r\n');
    const statusLine = lines[0];
    const statusMatch = statusLine.match(/^HTTP\/1\.1 (\d+) (.+)$/);
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

    // Do not intercept if it is the SW itself (handled by 'sw' check above, but for safety)
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
    const method = request.method;
    const path = new URL(request.url).pathname + new URL(request.url).search;
    const headers = [];
    for (const [k, v] of request.headers) {
        headers.push(`${k}: ${v}`);
    }
    
    let reqStr = `${method} ${path} HTTP/1.1\r\n`;
    reqStr += headers.join('\r\n') + '\r\n\r\n';
    
    const reqBytes = new TextEncoder().encode(reqStr);
    const serviceBytes = new TextEncoder().encode(serviceName);
    
    let bodyBytes = new Uint8Array(0);
    if (method !== 'GET' && method !== 'HEAD') {
         const buf = await request.arrayBuffer();
         bodyBytes = new Uint8Array(buf);
    }
    
    const totalLen = 1 + serviceBytes.length + reqBytes.length + bodyBytes.length;
    const buffer = new Uint8Array(totalLen);
    
    buffer[0] = serviceBytes.length;
    buffer.set(serviceBytes, 1);
    buffer.set(reqBytes, 1 + serviceBytes.length);
    buffer.set(bodyBytes, 1 + serviceBytes.length + reqBytes.length);
    
    while (activeRequestResolve) {
        await new Promise(r => setTimeout(r, 100));
    }
    
    return new Promise((resolve, reject) => {
        activeRequestResolve = resolve;
        activeResponseStreamController = null;
        responseBuffer = [];
        
        try {
            dataChannel.send(buffer);
        } catch(e) {
            activeRequestResolve = null;
            reject(e);
        }
    });
}
