import { MouseEvent, useEffect, useRef, useState } from "react";
import "./App.css";
import { FaPlay } from "react-icons/fa6";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import VideoCanvas from "./VideoCanvas";

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

function App() {
    return (
        <>
            <VideoCanvas videoWidth={0} videoHeight={0} />
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
