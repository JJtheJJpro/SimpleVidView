// VideoCanvas.tsx
import { useEffect, useRef, useState } from "react";

type VideoCanvasProps = {
    wsUrl?: string;          // e.g. "ws://127.0.0.1:9001"
    videoWidth: number;      // width of the incoming frames
    videoHeight: number;     // height of the incoming frames
};

export default function VideoCanvas({ wsUrl = "wss://127.0.0.1:9001", videoWidth, videoHeight }: VideoCanvasProps) {
    const canvasRef = useRef<HTMLCanvasElement | null>(null);
    const workerRef = useRef<Worker | null>(null);
    const wsRef = useRef<WebSocket | null>(null);
    const [status, setStatus] = useState("idle");

    useEffect(() => {
        const canvas = canvasRef.current!;
        if (!canvas) return;

        let worker: Worker;
        try {
            // create worker (Vite-friendly pattern)
            worker = new Worker(new URL("./renderer-worker.ts", import.meta.url), { type: "module" });
            workerRef.current = worker;
        } catch (err) {
            console.error("Failed to create worker:", err);
            setStatus("no-worker");
            return;
        }

        // Transfer the OffscreenCanvas to the worker
        if (!("transferControlToOffscreen" in HTMLCanvasElement.prototype)) {
            console.error("OffscreenCanvas not supported");
            setStatus("no-offscreen");
            return;
        }

        const offscreen = (canvas as HTMLCanvasElement).transferControlToOffscreen();
        worker.postMessage({ type: "init", canvas: offscreen, videoWidth, videoHeight }, [offscreen]);

        worker.onmessage = (ev) => {
            const d = ev.data;
            if (d?.type === "error") {
                console.error("Worker error:", d.message);
            }
        };

        // WebSocket connect
        const ws = new WebSocket(wsUrl);
        ws.binaryType = "arraybuffer";
        ws.onopen = () => {
            console.log("WS open");
            setStatus("connected");
        };
        ws.onmessage = (ev) => {
            // ev.data is ArrayBuffer - transfer to worker to avoid copy
            const ab = ev.data as ArrayBuffer;
            // Basic protection: if frame size is wrong, drop it in worker
            try {
                worker.postMessage({ type: "frame", buffer: ab }, [ab]);
            } catch (err) {
                // Transfer failed (maybe buffer already neutered). Fallback: copy
                worker.postMessage({ type: "frame", buffer: ab.slice(0) }, []);
            }
        };
        ws.onerror = (e) => {
            console.error("WS error", e);
            setStatus("ws-error");
        };
        ws.onclose = () => {
            console.log("WS closed");
            setStatus("closed");
        };

        wsRef.current = ws;

        // Clean up on unmount
        return () => {
            ws.close();
            worker.terminate();
            workerRef.current = null;
            wsRef.current = null;
        };
    }, [wsUrl, videoWidth, videoHeight]);

    return (
        <div style={{ width: "100%", height: "100%", position: "relative" }}>
            <canvas
                ref={canvasRef}
                width={videoWidth}
                height={videoHeight}
                style={{ width: "100%", height: "100%", background: "black", display: "block" }}
            />
            <div style={{ position: "absolute", left: 8, top: 8, color: "white", background: "rgba(0,0,0,0.4)", padding: 6, borderRadius: 4 }}>
                {status}
            </div>
        </div>
    );
}
