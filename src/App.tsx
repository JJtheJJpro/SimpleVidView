import { useState, useEffect, useRef, useMemo, MouseEvent, ChangeEventHandler } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import "./App.css";
import { Text } from '@react-three/drei';
import { FaPause, FaPlay } from 'react-icons/fa6';

function ProgressBar(props: { progress: number, onChange: (n: number) => void }) {
    const [isDragging, setIsDragging] = useState(false);
    const barRef = useRef<HTMLDivElement | null>(null);

    const updateProgressFromEvent = (e: any) => {
        if (!barRef.current) {
            return;
        }

        const rect = barRef.current.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const fraction = Math.min(Math.max(x / rect.width, 0), 1);
        //setProgress(fraction);
        props.onChange(fraction);
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
            <div className="pbardrag" style={{ width: `${props.progress * 100}%`, transition: isDragging ? "none" : "width 0.1s" }} />
        </div>
    );
}

// --- Types ---
interface VideoMeta {
    width: number;
    height: number;
    duration: number;
}

// --- R3F Component: Handles Video Texture ---
const VideoScreen = ({
    playing,
    meta
}: {
    playing: boolean;
    meta: VideoMeta
}) => {
    const meshRef = useRef<THREE.Mesh>(null);
    const textureRef = useRef<THREE.DataTexture | null>(null);
    const isFetching = useRef(false);

    // 1. Initialize DataTexture when meta changes
    // We use DataTexture because we are pushing raw bytes, not an Image element
    useMemo(() => {
        if (!meta) return;

        const size = meta.width * meta.height * 4; // RGBA
        const data = new Uint8Array(size);
        const texture = new THREE.DataTexture(
            data,
            meta.width,
            meta.height,
            THREE.RGBAFormat,
            THREE.UnsignedByteType
        );

        texture.needsUpdate = true;
        // Flip Y because WebGL coordinates are inverted relative to images
        texture.flipY = true;
        textureRef.current = texture;

    }, [meta]);

    // 2. The Render Loop (Runs 60fps+)
    useFrame(() => {
        if (!playing || !meta || !textureRef.current || isFetching.current) return;

        isFetching.current = true;

        // Fetch frame from Rust
        invoke<ArrayBuffer>('get_frame')
            .then((buffer) => {
                if (textureRef.current) {
                    // Direct update of texture memory
                    const data = new Uint8Array(buffer);

                    // Safety check for buffer size mismatch
                    if (textureRef.current.image.data && data.length === textureRef.current.image.data.length) {
                        (textureRef.current.image.data as Uint8Array).set(data);
                        textureRef.current.needsUpdate = true;
                    }
                }
            })
            .catch((e) => {
                console.warn("Frame drop/End:", e);
            })
            .finally(() => {
                isFetching.current = false;
            });
    });

    if (!meta || !textureRef.current) return null;

    // Calculate aspect ratio to scale the plane correctly
    const aspectRatio = meta.width / meta.height;

    return (
        <mesh ref={meshRef} scale={[aspectRatio * 5, 5, 1]}>
            <planeGeometry />
            <meshBasicMaterial map={textureRef.current} side={THREE.DoubleSide} />
        </mesh>
    );
};

// --- Main App Component ---
export default function App() {
    const [meta, setMeta] = useState<VideoMeta | null>(null);
    const [playing, setPlaying] = useState(false);
    const [progress, setProgress] = useState(0);

    // File Drop Listener
    useEffect(() => {
        const unlisten = listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
            if (event.payload.paths.length > 0) {
                loadVideo(event.payload.paths[0]);
            }
        });
        return () => { unlisten.then(u => u()); };
    }, []);

    const loadVideo = async (path: string) => {
        try {
            setPlaying(false);
            const metaStr = await invoke<string>('load_video', { path });
            setMeta(JSON.parse(metaStr));
            setPlaying(true);
        } catch (e) {
            console.error(e);
        }
    };

    const handleSeek = async (val: number) => {
        if (!meta) return;
        setProgress(val);
        // Basic debounce could be added here
        await invoke('seek_video', { timeSec: val * meta.duration });
    };

    return (
        <>
            <div className="vid">
                <Canvas camera={{ position: [0, 0, 5] }}>
                    <ambientLight intensity={0.5} />
                    {meta ? (
                        <VideoScreen playing={playing} meta={meta} />
                    ) : (
                        <Text color="white">Waiting for image stream...</Text>
                    )}

                </Canvas>
            </div>

            <div className="options">
                <div className="playpause" onClick={() => setPlaying(!playing)}>
                    {playing ? (
                        <FaPause className="playpause" size="100%" />
                    ) : (
                        <FaPlay className="playpause" size="100%" />
                    )}
                </div>
                <ProgressBar progress={progress} onChange={(n) => handleSeek(n)} />
            </div>
            <div className="absolute inset-0 z-10 flex flex-col justify-between pointer-events-none">
                {meta && (
                    <div className="bg-zinc-900/90 p-6 backdrop-blur-xl border-t border-zinc-800 pointer-events-auto pb-8">
                        <div className="flex flex-col gap-3 max-w-4xl mx-auto">
                            {/* Progress Bar */}
                            <input
                                title='Range'
                                type="range"
                                min="0" max="1" step="0.001"
                                value={progress}
                                onChange={(e) => handleSeek(parseFloat(e.target.value))}
                                className="w-full h-1.5 bg-zinc-700 rounded-lg appearance-none cursor-pointer accent-blue-500 hover:h-2 transition-all"
                            />
                        </div>
                    </div>
                )}
            </div>
        </>
    );
}