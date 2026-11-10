import { useCallback, useEffect, useRef, useState } from "react";

export const canRecord = () => typeof window !== "undefined" && typeof MediaRecorder !== "undefined" && !!navigator.mediaDevices?.getUserMedia;

/** Records the microphone until stopped, then hands over the recording as a file. */
export function useRecorder(onRecorded: (file: File) => void, onRefused: () => void) {
  const [recording, setRecording] = useState(false);
  const recorder = useRef<MediaRecorder | null>(null);
  const done = useRef(onRecorded);
  const refused = useRef(onRefused);
  useEffect(() => {
    done.current = onRecorded;
    refused.current = onRefused;
  });

  const start = useCallback(async () => {
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      refused.current();
      return;
    }
    const r = new MediaRecorder(stream);
    const chunks: Blob[] = [];
    r.ondataavailable = (e) => {
      if (e.data.size > 0) chunks.push(e.data);
    };
    r.onstop = () => {
      for (const track of stream.getTracks()) track.stop();
      recorder.current = null;
      setRecording(false);
      const type = (r.mimeType || chunks[0]?.type || "audio/webm").replace(/;.*$/, "");
      const extension = type.includes("mp4") ? "m4a" : type.includes("ogg") ? "ogg" : "webm";
      if (chunks.length) done.current(new File(chunks, `recording-${new Date().toISOString().slice(11, 19).replaceAll(":", "")}.${extension}`, { type }));
    };
    recorder.current = r;
    r.start();
    setRecording(true);
  }, []);

  const stop = useCallback(() => recorder.current?.stop(), []);

  useEffect(() => () => recorder.current?.stop(), []);

  return { recording, start, stop };
}
