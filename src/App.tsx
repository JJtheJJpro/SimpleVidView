import { MouseEvent, useEffect, useRef, useState } from "react";
import { FaPause, FaPlay } from "react-icons/fa6";
import "./App.css";
import { listen } from "@tauri-apps/api/event";

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

export default function App() {
    const [playing, setPlaying] = useState(false);
    const [progress, setProgress] = useState(0);
    const vidRef = useRef<HTMLVideoElement | null>(null);

    useEffect(() => {
        const unlisten = listen<{ paths: string[] }>('tauri://drag-drop', (event) => {
            
        });
        return () => { unlisten.then(u => u()); };
    }, []);

    const handleSeek = async (val: number) => {

    };

    return (
        <>
            <div className="vid">
                <video ref={vidRef} itemType='video/mp4' src="" controls autoPlay />
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
        </>
    );
}

//<Canvas camera={{ position: [0, 0, 5] }}>
//                    <ambientLight intensity={0.5} />
//                    {meta ? (
//                        <VideoScreen playing={playing} meta={meta} />
//                    ) : (
//                        <Text color="white">Waiting for image stream...</Text>
//                    )}
//
//                </Canvas>