import { MouseEvent, useEffect, useRef, useState } from "react";
import { FaPause, FaPlay } from "react-icons/fa6";
import "./App.css";
import { convertFileSrc } from "@tauri-apps/api/core";

function useVideoFrame(
    videoRef: React.RefObject<HTMLVideoElement | null>,
    onFrame: (currentTime: number, metadata?: VideoFrameCallbackMetadata) => void
) {
    //const rafId = useRef<number | null>(null);
    const vfId = useRef<number | null>(null);

    useEffect(() => {
        const video = videoRef.current;
        if (!video) return;

        let running = true;

        // --- Option 3: requestVideoFrameCallback ----------------------------
        if ("requestVideoFrameCallback" in video) {
            const handleFrame = (
                _now: number,
                metadata: VideoFrameCallbackMetadata
            ) => {
                if (!running) return;
                onFrame(metadata.mediaTime, metadata);
                vfId.current = video.requestVideoFrameCallback(handleFrame);
            };

            vfId.current = video.requestVideoFrameCallback(handleFrame);

            return () => {
                running = false;
                if (vfId.current !== null && "cancelVideoFrameCallback" in video) {
                    (video as any).cancelVideoFrameCallback(vfId.current);
                }
            };
        }
        //else {
        //    // --- Option 1 fallback: requestAnimationFrame ------------------------
        //    const loop = () => {
        //        if (!running) return;
        //        onFrame(video.currentTime); // Error will always be here.  Deal with it.
        //        rafId.current = requestAnimationFrame(loop);
        //    };
        //    rafId.current = requestAnimationFrame(loop);
        //    return () => {
        //        running = false;
        //        if (rafId.current !== null) cancelAnimationFrame(rafId.current);
        //    };
        //}
    }, [videoRef, onFrame]);
}

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

    useVideoFrame(vidRef, (curTime) => {
        setProgress(curTime);
    });

    useEffect(() => {
        if (vidRef.current) {
            vidRef.current.play();
        }
    }, [vidRef]);

    //useEffect(() => {
    //    const unlisten = listen<{ paths: string[] }>('tauri://drag-drop', (event) => {
    //        if (vidRef.current) {
    //            vidRef.current.src = convertFileSrc('v.mp4', 'stream');
    //        }
    //    });
    //    return () => { unlisten.then(u => u()); };
    //}, [vidRef]);

    const handleSeek = async (val: number) => {
        if (vidRef.current) {
            const upProg = vidRef.current.duration * val;
            vidRef.current.currentTime = upProg;
            setProgress(upProg);
        }
    };

    return (
        <>
            <div className="vid">
                <video loop ref={vidRef} itemType='video/mp4' src={convertFileSrc('v.mp4', 'stream')} />
            </div>

            <div className="options">
                <div className="playpause" onClick={() => {
                    setPlaying(!playing);
                    if (vidRef.current) {
                        if (playing) {
                            vidRef.current.play();
                        } else {
                            vidRef.current.pause();
                        }
                    }
                }}>
                    {playing ? (
                        <FaPause className="playpause" size="100%" />
                    ) : (
                        <FaPlay className="playpause" size="100%" />
                    )}
                </div>
                <ProgressBar progress={vidRef.current ? progress / vidRef.current.duration : 0} onChange={(n) => handleSeek(n)} />
            </div>
        </>
    );
}
