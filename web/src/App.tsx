import { FormEvent, useEffect, useRef, useState } from "react";
import { speechRecognitionConstructor, type SpeechRecognitionLike } from "./speech";

type Connection = "idle" | "connecting" | "connected" | "disconnected" | "error";
type ServerMessage = { type: "status"; status: string } | { type: "output"; text: string } | { type: "error"; message: string } | { type: "done" };

function daemonHttpUrl() { return import.meta.env.VITE_RADIO_URL ?? window.location.origin; }
function daemonWsUrl(id: string) { const url = new URL(daemonHttpUrl()); url.protocol = url.protocol === "https:" ? "wss:" : "ws:"; url.pathname = `/sessions/${id}/ws`; return url.toString(); }

export function App() {
  const [connection, setConnection] = useState<Connection>("idle");
  const [sessionId, setSessionId] = useState<string>();
  const [agentStatus, setAgentStatus] = useState("not started");
  const [prompt, setPrompt] = useState("");
  const [output, setOutput] = useState<string[]>([]);
  const [listening, setListening] = useState(false);
  const [voiceError, setVoiceError] = useState<string>();
  const socket = useRef<WebSocket>();
  const recognition = useRef<SpeechRecognitionLike>();
  const voiceAvailable = Boolean(speechRecognitionConstructor());

  async function connect(id?: string) {
    setConnection("connecting");
    try {
      let target = id;
      if (!target) {
        const response = await fetch(`${daemonHttpUrl()}/sessions`, { method: "POST" });
        if (!response.ok) throw new Error(`session creation failed (${response.status})`);
        target = (await response.json()).id;
        setSessionId(target);
      }
      const ws = new WebSocket(daemonWsUrl(target!));
      socket.current = ws;
      ws.onopen = () => setConnection("connected");
      ws.onclose = () => setConnection("disconnected");
      ws.onerror = () => setConnection("error");
      ws.onmessage = ({ data }) => {
        const message = JSON.parse(data) as ServerMessage;
        if (message.type === "status") setAgentStatus(message.status);
        if (message.type === "output") setOutput(lines => [...lines, message.text]);
        if (message.type === "error") setOutput(lines => [...lines, `error: ${message.message}`]);
        if (message.type === "done") setAgentStatus("done");
      };
    } catch (error) {
      setConnection("error");
      setOutput(lines => [...lines, error instanceof Error ? error.message : String(error)]);
    }
  }

  useEffect(() => () => {
    socket.current?.close();
    recognition.current?.stop();
  }, []);

  function send(event: FormEvent) {
    event.preventDefault();
    const text = prompt.trim();
    if (!text || socket.current?.readyState !== WebSocket.OPEN) return;
    socket.current.send(JSON.stringify({ type: "prompt", text }));
    setPrompt("");
  }

  function stop() { socket.current?.send(JSON.stringify({ type: "stop" })); }

  function startListening() {
    const Recognition = speechRecognitionConstructor();
    if (!Recognition || listening) return;
    setVoiceError(undefined);
    const instance = new Recognition();
    instance.continuous = true;
    instance.interimResults = true;
    instance.lang = navigator.language || "en-US";
    let committed = prompt.trim();
    instance.onresult = event => {
      let interim = "";
      for (let i = event.resultIndex; i < event.results.length; i++) {
        const text = event.results[i][0]?.transcript ?? "";
        if (event.results[i].isFinal) committed = `${committed} ${text}`.trim();
        else interim += text;
      }
      setPrompt(`${committed} ${interim}`.trim());
    };
    instance.onerror = event => setVoiceError(event.message || event.error);
    instance.onend = () => { setListening(false); recognition.current = undefined; };
    recognition.current = instance;
    instance.start();
    setListening(true);
  }

  function stopListening() { recognition.current?.stop(); }

  return <main>
    <header><div><span className={`dot ${connection}`} /> radio</div><small>{connection} · {agentStatus}</small></header>
    <section className="terminal" aria-live="polite">{output.length ? output.map((line, i) => <div key={i}>{line}</div>) : <span className="muted">Codex output will appear here.</span>}</section>
    {connection === "connected" ? <form onSubmit={send}>
      <textarea value={prompt} onChange={e => setPrompt(e.target.value)} placeholder="Tell Codex what to do…" rows={3} />
      {voiceAvailable && <button className={`talk ${listening ? "listening" : ""}`} type="button" onPointerDown={startListening} onPointerUp={stopListening} onPointerCancel={stopListening}>{listening ? "Listening… release to stop" : "Hold to talk"}</button>}
      {voiceError && <small className="voice-error">Voice input: {voiceError}. You can keep typing.</small>}
      <div className="actions"><button className="send" type="submit">Send transcript</button><button className="stop" type="button" onClick={stop}>Stop agent</button></div>
    </form> : <button className="connect" onClick={() => connect(sessionId)}>{sessionId ? "Reconnect" : "Start session"}</button>}
  </main>;
}
