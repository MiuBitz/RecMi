const { invoke } = window.__TAURI__.core;
const elapsed = document.getElementById('elapsed');
const stop = document.getElementById('stop');
let stopping = false;
stop.onclick = async () => {
  if (stopping) return;
  stopping = true; stop.disabled = true;
  try { await invoke('stop_recording'); }
  catch (error) { stop.title = String(error); }
  finally { stopping = false; stop.disabled = false; }
};
async function refresh() {
  try {
    const status = await invoke('status');
    elapsed.textContent = `${String(Math.floor(status.elapsed / 60)).padStart(2, '0')}:${String(status.elapsed % 60).padStart(2, '0')}`;
    stop.disabled = stopping || !status.recording;
  } catch (error) { stop.title = String(error); }
  setTimeout(refresh, 250);
}
refresh();
