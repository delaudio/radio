import { FormEvent, useEffect, useRef, useState } from "react";
import { speak, speechRecognitionConstructor, speechSynthesisAvailable, stopSpeaking, type SpeechRecognitionLike } from "./speech";

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
  const [muted, setMuted] = useState(false);
  const [diff, setDiff] = useState("");
  const [showDiff, setShowDiff] = useState(false);
  const socket = useRef<WebSocket>();
  const recognition = useRef<SpeechRecognitionLike>();
  const responseBuffer = useRef<string[]>([]);
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
        if (message.type === "output") { setOutput(lines => [...lines, message.text]); responseBuffer.current.push(message.text); }
        if (message.type === "error") { setOutput(lines => [...lines, `error: ${message.message}`]); if (!muted) speak(`Radio error. ${message.message}`); }
        if (message.type === "done") {
          setAgentStatus("done");
          void refreshDiff();
          if (!muted) speak(completionSpeech(responseBuffer.current));
          responseBuffer.current = [];
        }
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
    stopSpeaking();
    responseBuffer.current = [];
    socket.current.send(JSON.stringify({ type: "prompt", text }));
    setPrompt("");
  }

  function stop() { stopSpeaking(); socket.current?.send(JSON.stringify({ type: "stop" })); }

  async function refreshDiff() {
    try {
      const response = await fetch(`${daemonHttpUrl()}/diff`);
      if (!response.ok) throw new Error(`diff failed (${response.status})`);
      setDiff((await response.json()).diff);
    } catch (error) {
      setOutput(lines => [...lines, `diff error: ${error instanceof Error ? error.message : String(error)}`]);
    }
  }

  function toggleMute() {
    setMuted(value => {
      const next = !value;
      if (next) stopSpeaking();
      return next;
    });
  }

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
    <header><div><span className={`dot ${connection}`} /> radio</div><div className="status"><small>{connection} · {agentStatus}</small>{speechSynthesisAvailable() && <button className="mute" type="button" onClick={toggleMute}>{muted ? "Unmute" : "Mute"}</button>}</div></header>
    <section className="terminal" aria-live="polite">{showDiff ? <DiffView diff={diff} /> : output.length ? output.map((line, i) => <div key={i}>{line}</div>) : <span className="muted">Codex output will appear here.</span>}</section>
    {connection === "connected" ? <form onSubmit={send}>
      <textarea value={prompt} onChange={e => setPrompt(e.target.value)} placeholder="Tell Codex what to do…" rows={3} />
      {voiceAvailable && <button className={`talk ${listening ? "listening" : ""}`} type="button" onPointerDown={startListening} onPointerUp={stopListening} onPointerCancel={stopListening}>{listening ? "Listening… release to stop" : "Hold to talk"}</button>}
      {voiceError && <small className="voice-error">Voice input: {voiceError}. You can keep typing.</small>}
      <div className="view-actions"><button type="button" onClick={() => setShowDiff(false)}>Output</button><button type="button" onClick={() => { void refreshDiff(); setShowDiff(true); }}>Diff</button></div>
      <div className="actions"><button className="send" type="submit">Send transcript</button><button className="stop" type="button" onClick={stop}>Stop agent</button></div>
    </form> : <button className="connect" onClick={() => connect(sessionId)}>{sessionId ? "Reconnect" : "Start session"}</button>}
  </main>;
}

function completionSpeech(lines: string[]) {
  const useful = lines.map(line => line.trim()).filter(line => line.length > 0 && !line.startsWith("$") && !line.startsWith("Running ") && !line.startsWith("Reading "));
  const text = useful.slice(-6).join(" ").replace(/[`*_#]/g, " ").replace(/\s+/g, " ").trim();
  if (!text) return "Codex finished."; 
  return text.length > 900 ? `${text.slice(0, 900)}. Response truncated.` : text;
}

function DiffView({ diff }: { diff: string }) {
  if (!diff) return <span className="muted">No working-tree changes.</span>;
  return <div className="diff">{diff.split("\n").map((line, i) => <div key={i} className={line.startsWith("+") && !line.startsWith("+++") ? "added" : line.startsWith("-") && !line.startsWith("---") ? "removed" : line.startsWith("@@") ? "hunk" : ""}>{line || " "}</div>)}</div>;
}
