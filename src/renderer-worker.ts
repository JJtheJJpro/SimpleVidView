// renderer-worker.ts
// --- Worker module to receive OffscreenCanvas and binary RGBA frames via postMessage
export type InitMessage = { type: "init"; canvas: OffscreenCanvas; videoWidth: number; videoHeight: number; };
export type FrameMessage = { type: "frame"; buffer: ArrayBuffer };

let gl: WebGL2RenderingContext | null = null;
let videoW = 0;
let videoH = 0;
let texture: WebGLTexture | null = null;
let program: WebGLProgram | null = null;
let vao: WebGLVertexArrayObject | null = null;

const vsSrc = `#version 300 es
in vec2 a_position;
in vec2 a_tex;
out vec2 v_tex;
void main() {
  v_tex = a_tex;
  gl_Position = vec4(a_position, 0.0, 1.0);
}`;
const fsSrc = `#version 300 es
precision mediump float;
in vec2 v_tex;
uniform sampler2D u_tex;
out vec4 outColor;
void main() {
  outColor = texture(u_tex, v_tex);
}`;

function compileShader(gl: WebGL2RenderingContext, src: string, type: number) {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
        const log = gl.getShaderInfoLog(s);
        gl.deleteShader(s);
        throw new Error("Shader compile error: " + log);
    }
    return s;
}

function createProgram(gl: WebGL2RenderingContext, vs: string, fs: string) {
    const vsS = compileShader(gl, vs, gl.VERTEX_SHADER);
    const fsS = compileShader(gl, fs, gl.FRAGMENT_SHADER);
    const p = gl.createProgram()!;
    gl.attachShader(p, vsS);
    gl.attachShader(p, fsS);
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
        const log = gl.getProgramInfoLog(p);
        gl.deleteProgram(p);
        throw new Error("Program link error: " + log);
    }
    return p;
}

function initGL(offscreen: OffscreenCanvas, w: number, h: number) {
    const ctx = offscreen.getContext("webgl2", { antialias: false, preserveDrawingBuffer: false }) as WebGL2RenderingContext | null;
    if (!ctx) throw new Error("WebGL2 not supported in worker.");
    gl = ctx;
    videoW = w;
    videoH = h;

    program = createProgram(gl, vsSrc, fsSrc);
    gl.useProgram(program);

    // Setup VAO + buffer: full screen quad
    vao = gl.createVertexArray();
    gl.bindVertexArray(vao);

    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);

    // x,y, u,v for four verts (triangle strip)
    const verts = new Float32Array([
        -1, -1, 0, 0,
        1, -1, 1, 0,
        -1, 1, 0, 1,
        1, 1, 1, 1,
    ]);
    gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);

    const posLoc = gl.getAttribLocation(program, "a_position");
    const texLoc = gl.getAttribLocation(program, "a_tex");
    const stride = 4 * Float32Array.BYTES_PER_ELEMENT;

    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, stride, 0);
    gl.enableVertexAttribArray(texLoc);
    gl.vertexAttribPointer(texLoc, 2, gl.FLOAT, false, stride, 2 * Float32Array.BYTES_PER_ELEMENT);

    // Create texture and allocate storage
    texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);
    // Allocate with null data first
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, videoW, videoH, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);

    // viewport
    gl.viewport(0, 0, offscreen.width, offscreen.height);
    gl.clearColor(0, 0, 0, 1);
}

function renderFrameFromBuffer(ab: ArrayBuffer) {
    if (!gl || !texture || !program) return;
    // Expect exactly videoW * videoH * 4 bytes
    const expected = videoW * videoH * 4;
    if (ab.byteLength !== expected) {
        // Optionally handle framing metadata (PTS) or mismatched frame sizes
        // Drop frame silently:
        // console.warn("frame size mismatch", ab.byteLength, expected);
        return;
    }

    const u8 = new Uint8Array(ab);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1); // important for non-4-multiples
    // Upload entire texture (fast path)
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, videoW, videoH, gl.RGBA, gl.UNSIGNED_BYTE, u8);

    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

// Message handling
self.onmessage = (ev: MessageEvent) => {
    const data = ev.data as InitMessage | FrameMessage;
    if (data.type === "init") {
        try {
            initGL(data.canvas, data.videoWidth, data.videoHeight);
        } catch (err) {
            // forward error to main thread
            self.postMessage({ type: "error", message: String(err) });
        }
    } else if (data.type === "frame") {
        // data.buffer is the transferred ArrayBuffer
        renderFrameFromBuffer(data.buffer);
        // We don't post anything back. Ownership of the buffer was transferred.
    }
};
