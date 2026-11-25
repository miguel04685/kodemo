// Kodemo live preview demo.
// Edit this file and save: the embedded preview reloads automatically.

const counter = document.getElementById("counter");
const button = document.getElementById("counter-button");
const greeting = document.getElementById("greeting");
const demoButton = document.getElementById("demo-button");
const terminal = document.getElementById("terminal");

const messages = [
  "Hello from Kodemo.",
  "Saved, reloaded, still yours.",
  "Everything is local. No telemetry.",
  "Rendered by WebKitGTK, wrapped by Rust.",
  "Try editing styles.css next.",
];

let count = Number(localStorage.getItem("kodemo-wins") || 0) || 0;

function paint() {
  counter.textContent = count;
  localStorage.setItem("kodemo-wins", String(count));
  greeting.textContent = messages[count % messages.length];
}

button?.addEventListener("click", () => {
  count += 1;
  paint();
});

demoButton?.addEventListener("click", () => {
  count += 1;
  paint();
  terminal?.classList.add("running");
  window.setTimeout(() => terminal?.classList.remove("running"), 900);
});

paint();

console.log("Kodemo preview ready");
