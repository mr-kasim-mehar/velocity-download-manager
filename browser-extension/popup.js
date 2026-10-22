const api = globalThis.browser || globalThis.chrome;
const button = document.querySelector("#download");
const status = document.querySelector("#status");
const mode = document.querySelector("#mode");
const resolution = document.querySelector("#resolution");
const audioFormat = document.querySelector("#audio-format");

mode.addEventListener("change", () => {
  document.querySelector("#resolution-row").hidden = mode.value === "audio";
  document.querySelector("#audio-row").hidden = mode.value !== "audio";
  button.textContent = mode.value === "audio" ? "Download audio" : "Download current video";
});

function setStatus(message, type = "") {
  status.textContent = message;
  status.className = type;
}

button.addEventListener("click", () => {
  api.tabs.query({ active: true, currentWindow: true }, (tabs) => {
    const error = globalThis.chrome?.runtime?.lastError;
    if (error) {
      setStatus(error.message, "error");
      return;
    }
    const tab = tabs && tabs[0];
    if (!tab || !/^https?:\/\//i.test(tab.url || "")) {
      setStatus("Open a normal video webpage first.", "error");
      return;
    }
    const handoff = `velocity://download?url=${encodeURIComponent(tab.url)}&type=media&title=${encodeURIComponent(tab.title || "")}&mode=${mode.value}&resolution=${resolution.value}&audio_format=${audioFormat.value}`;
    button.disabled = true;
    api.tabs.update(tab.id, { url: handoff }, () => {
      const updateError = globalThis.chrome?.runtime?.lastError;
      if (updateError) setStatus(updateError.message, "error");
      else setStatus("Sent to Velocity.", "success");
      setTimeout(() => { button.disabled = false; }, 1000);
    });
  });
});
