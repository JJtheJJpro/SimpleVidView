import { MouseEvent, useCallback, useEffect, useRef, useState } from "react";
import "./App.css";
import { FaPlay } from "react-icons/fa6";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { Plane, Text } from '@react-three/drei';
import * as THREE from 'three';
import Socket from "@tauri-apps/plugin-websocket";

export interface RawImageData {
    data: Uint8Array;
    width: number;
    height: number;
}

function ProgressBar() {
    const [progress, setProgress] = useState(0);
    const [isDragging, setIsDragging] = useState(false);
    const barRef = useRef<HTMLDivElement | null>(null);

    const updateProgressFromEvent = (e: any) => {
        if (!barRef.current) {
            return;
        }

        const rect = barRef.current.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const fraction = Math.min(Math.max(x / rect.width, 0), 1);
        setProgress(fraction);
    };

    const handleMouseDown = (e: MouseEvent<HTMLDivElement>) => {
        setIsDragging(true);
        updateProgressFromEvent(e);
    };

    const handleMouseMove = (e: globalThis.MouseEvent) => {
        if (isDragging) {
            updateProgressFromEvent(e);
        }
    };

    const handleMouseUp = () => {
        setIsDragging(false);
    };

    useEffect(() => {
        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("mouseup", handleMouseUp);
        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("mouseup", handleMouseUp);
        };
    })

    return (
        <div className="pbar" ref={barRef} onMouseDown={handleMouseDown}>
            <div className="pbardrag" style={{ width: `${progress * 100}%`, transition: isDragging ? "none" : "width 0.1s" }} />
        </div>
    );
}

/*
// --- WebSocket Simulation (Replace with your actual Tauri WebSocket) ---
// This will simulate receiving raw pixel data for a simple animated gradient.
// In your real app, this would be your `websocket.onmessage` handler.
const simulateWebSocketData = (callback: any) => {
    let frameCount = 0;
    const width = 256;
    const height = 256;
    const data = new Uint8Array(width * height * 4); // RGBA

    const animate = () => {
        frameCount++;
        for (let i = 0; i < width; i++) {
            for (let j = 0; j < height; j++) {
                const index = (j * width + i) * 4;
                const r = Math.floor(128 + 127 * Math.sin(0.01 * frameCount + i / 50));
                const g = Math.floor(128 + 127 * Math.sin(0.01 * frameCount + j / 50 + Math.PI / 2));
                const b = Math.floor(128 + 127 * Math.sin(0.01 * frameCount + (i + j) / 70 + Math.PI));

                data[index] = r;      // R
                data[index + 1] = g;  // G
                data[index + 2] = b;  // B
                data[index + 3] = 255; // A (fully opaque)
            }
        }
        // Simulate receiving this data from websocket
        callback({ data: data.slice(), width, height }); // Pass a copy to avoid mutation issues
        requestAnimationFrame(animate);
    };
    animate();
};
// --- End WebSocket Simulation ---
*/

// --- Component to display the texture on a plane ---
interface ImagePlaneProps {
    imageData: RawImageData;
}

function ImagePlane({ imageData }: ImagePlaneProps) {
    // Specify the type for the refs
    const meshRef = useRef<THREE.Mesh>(null);
    const textureRef = useRef<THREE.DataTexture | null>(null);

    // Initialize the texture once
    useEffect(() => {
        if (!imageData || textureRef.current) return;

        // Create a DataTexture, specifying the format (RGBA)
        const initialTexture = new THREE.DataTexture(
            imageData.data,
            imageData.width,
            imageData.height,
            THREE.RGBAFormat,
            THREE.UnsignedByteType // Assuming standard 8-bit per channel
        );
        initialTexture.needsUpdate = true;
        textureRef.current = initialTexture;

        // Apply the texture to the material
        if (meshRef.current) {
            // Cast the material to the correct type to access the 'map' property
            const material = meshRef.current.material as THREE.MeshBasicMaterial;
            material.map = textureRef.current;
            material.needsUpdate = true;
        }

    }, [imageData]); // Run once on initial data load

    // Update the texture data when new imageData arrives
    useEffect(() => {
        const texture = textureRef.current;
        if (imageData && texture) {
            // Update the texture's data, dimensions, and mark it for update
            texture.image.data = imageData.data;
            texture.image.width = imageData.width;
            texture.image.height = imageData.height;
            texture.needsUpdate = true; // CRITICAL: Re-upload to GPU
        }
    }, [imageData]); // Re-run effect when new data is received

    // Scale the plane to maintain aspect ratio and size it relative to the scene
    const planeWidth = imageData.width / 100;
    const planeHeight = imageData.height / 100;

    return (
        <Plane ref={meshRef} args={[planeWidth, planeHeight]}>
            <meshBasicMaterial transparent side={THREE.DoubleSide} />
        </Plane>
    );
}

// --- Main App Component ---
function ImageCanvas() {
    // Type the state with the RawImageData interface
    const [currentImageData, setCurrentImageData] = useState<RawImageData | null>(null);

    // Function to handle incoming data from the WebSocket
    const handleIncomingData = useCallback((data: ArrayBuffer) => {
        if (data.byteLength < 8) {
            console.error("Received message too short for header.");
            return;
        }

        // Assuming your Tauri backend sends 4 bytes (u32) for width, then 4 bytes for height, 
        // followed by the pixel data.
        const headerBuffer = data.slice(0, 8);
        const pixelDataBuffer = data.slice(8);

        const dataView = new DataView(headerBuffer);
        // Use little-endian byte order (true) to match common Rust/binary serialization
        const width = dataView.getUint32(0, true);
        const height = dataView.getUint32(4, true);

        const pixelData = new Uint8Array(pixelDataBuffer);

        setCurrentImageData({ data: pixelData, width, height });

    }, []);

    // Effect to initialize the WebSocket
    useEffect(() => {
        // Replace this with your actual Tauri WebSocket setup
        //const ws = new WebSocket("ws://localhost:9001/ws");
        //ws.binaryType = "arraybuffer"; // ESSENTIAL: ensures event.data is an ArrayBuffer
        //ws.onmessage = (event: MessageEvent) => {
        //    if (event.data instanceof ArrayBuffer) {
        //        handleIncomingData(event.data);
        //    } else {
        //        console.warn("Received non-binary data, ignoring.");
        //    }
        //};
        //ws.onerror = (e) => console.error("WebSocket Error:", e);
        //ws.onclose = () => console.log("WebSocket connection closed.");
        //return () => {
        //    ws.close();
        //};

        const ws = Socket.connect("ws://localhost:9001/ws");
        ws.then(v => {
            v.addListener(msg => {
                if (msg.type == "Binary") {
                    handleIncomingData(Uint8Array.from(msg.data).buffer);
                }
            });
        });
    }, [handleIncomingData]);

    return (
        <div style={{ width: '100%', height: '100%' }}>
            <Canvas camera={{ position: [0, 0, 5] }}>
                <ambientLight intensity={0.5} />
                {currentImageData ? (
                    <ImagePlane imageData={currentImageData} />
                ) : (
                    <Text color="white">Waiting for image stream...</Text> // Add Text component from Drei if needed
                )}
            </Canvas>
        </div>
    );
}

function App() {
    return (
        <>
            <ImageCanvas />
            <div className="options">
                <div className="playpause">
                    <FaPlay className="playpause" size="100%" />
                </div>
                <ProgressBar />
            </div>
        </>
    );
}

export default App;
