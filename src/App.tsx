import { MouseEvent, useCallback, useEffect, useRef, useState } from "react";
import { FaPause, FaPlay } from "react-icons/fa6";
import "./App.css";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { exists } from '@tauri-apps/plugin-fs';

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
    const [dragPos, setDragPos] = useState(0);
    const barRef = useRef<HTMLDivElement | null>(null);

    const updateProgressFromEvent = (e: any) => {
        if (!barRef.current) {
            return;
        }

        const rect = barRef.current.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const fraction = Math.min(Math.max(x / rect.width, 0), 1);
        //setProgress(fraction);
        setDragPos(fraction);
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
        setDragPos(0);
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
            <div className="pbardrag" style={{ width: `${isDragging ? dragPos * 100 : props.progress * 100}%`, transition: isDragging ? "none" : "width 0.1s" }} />
        </div>
    );
}

export default function App() {
    const [playing, setPlaying] = useState(false);
    const [progress, setProgress] = useState(0);
    const [loading, setLoading] = useState(0);
    const [fileExists, setFileExists] = useState(true);
    const vidRef = useRef<HTMLVideoElement | null>(null);
    const playCallback = useCallback(() => {
        setPlaying(prev => {
            const newVal = !prev;

            if (vidRef.current) {
                if (newVal) {
                    vidRef.current.play();
                } else {
                    vidRef.current.pause();
                }
            }

            return newVal;
        });
    }, [playing, vidRef]);

    useVideoFrame(vidRef, (curTime) => {
        setProgress(curTime);
    });

    useEffect(() => {
        if (vidRef.current) {
            vidRef.current.src = convertFileSrc('v.mp4', 'stream') + `?t=${Date.now()}`;
            vidRef.current.load();
            vidRef.current.play();
            setPlaying(true);
        }
    }, [vidRef]);

    function keyDown(ev: KeyboardEvent) {
        if (vidRef.current) {
            console.log(ev.code);
            switch (ev.code) {
                case "ArrowRight":
                    break;
                case "ArrowLeft":
                    break;
                case "Space":
                    playCallback();
                    break;
            }
        }
    }
    function keyUp(ev: KeyboardEvent) {
        if (vidRef.current) {
            switch (ev.code) {
                case "ArrowRight":
                    break;
                case "ArrowLeft":
                    break;
                case "Space":
                    break;
            }
        }
    }

    useEffect(() => {
        (async () => setFileExists(await exists("./v.mp4")))();

        const unlisten1 = listen('refresh-mega', () => {
            //if (vidRef.current) {
            //    vidRef.current.src = convertFileSrc('v.mp4', 'stream');
            //}
            window.location.reload();
        });
        const unlisten2 = listen<number>('c-prog', (e) => {
            setFileExists(false);
            setLoading(e.payload);
        });

        window.addEventListener("keydown", keyDown);
        window.addEventListener("keyup", keyUp);

        return () => {
            unlisten1.then(u => u());
            unlisten2.then(u => u());
            window.removeEventListener("keydown", keyDown);
            window.removeEventListener("keyup", keyUp);
        };
    }, []);

    const handleSeek = async (val: number) => {
        if (vidRef.current && !loading) {
            const upProg = vidRef.current.duration * val;
            vidRef.current.currentTime = upProg;
            setProgress(upProg);
        }
    };

    return (
        <>
            <div className="vid">
                {fileExists ? (
                    <video loop ref={vidRef} itemType='video/mp4' />
                ) : loading ? (
                    <p>{(loading * 100).toPrecision(4)}%</p>
                ) : (
                    <p>Drop video file here</p>
                )}
            </div>

            <div className="options">
                <div className="playpause" onClick={() => {
                    playCallback();
                }}>
                    {playing ? (
                        <FaPlay className="playpause" size="100%" />
                    ) : (
                        <FaPause className="playpause" size="100%" />
                    )}
                </div>
                <ProgressBar progress={vidRef.current ? progress / vidRef.current.duration : loading} onChange={handleSeek} />
            </div>
        </>
    );
}
