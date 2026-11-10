const { invoke } = window.__TAURI__.core;
const $ = id => document.getElementById(id);
let region = null, busy = false, recording = false;
let saveFolder = "Videos / RecMi", savedFile = null;
function error(e) { $('message').textContent = String(e); $('message').classList.add('error'); $('message').hidden = false; }
async function config() { await invoke('configure', { region, audio: $('audio').value }); }
function renderArea() {
  $('full').classList.toggle('selected', !region);
  $('region').classList.toggle('selected', !!region);
  $('edit').hidden = !region;
  $('full').setAttribute('aria-pressed', String(!region));
  $('region').setAttribute('aria-pressed', String(!!region));
  $('area').textContent = region ? `${region.width} × ${region.height} px · (${region.x}, ${region.y})` : 'Primary monitor';
}
$('full').onclick = async () => { try { region = null; await config(); renderArea(); } catch (e) { error(e); } };
async function selectArea() {
  busy = true; renderControls();
  try { await config(); await invoke('begin_selection'); } catch (e) { error(e); }
  finally { busy = false; renderControls(); }
}
$('region').onclick = $('edit').onclick = selectArea;
$('audio').onchange = async () => {
  const hints = {system:'Default speakers or headphones.',microphone:'Default Windows microphone.',both:'System sound with your default microphone.',silent:'Video only. No audio track.'};
  $('audio-hint').textContent = hints[$('audio').value];
  try { await config(); } catch (e) { error(e); }
};
function renderControls() {
  $('record').disabled = busy;
  for (const id of ['full', 'region', 'edit', 'audio']) $(id).disabled = recording || busy;
  const label = busy ? (recording ? 'Saving recording…' : 'Starting recording…') : (recording ? 'Stop recording' : 'Start recording');
  $('record-label').textContent = label;
  $('record').setAttribute('aria-label', label);
  $('record').title = label + ' (F9)';
  document.body.classList.toggle('is-recording', recording);
  document.body.classList.toggle('is-busy', busy);
}
$('record').onclick = async () => {
  busy = true; renderControls(); $('message').classList.remove('error'); $('message').hidden = true;
  try { await invoke('toggle_recording'); } catch (e) { error(e); }
  finally { busy = false; await refresh(false); }
};
function renderLocation() {
  $('folder-label').textContent = savedFile ? 'Saved · ' + savedFile.split(/[\\/]/).pop() : saveFolder;
  $('folder').title = savedFile ? 'Show saved recording: ' + savedFile : 'Open recordings folder: ' + saveFolder;
  $('folder').setAttribute('aria-label', savedFile ? 'Show saved recording in folder' : 'Open recordings folder');
}
$('folder').onclick = () => invoke(savedFile ? 'open_recording_location' : 'open_folder').catch(error);
invoke('recording_folder').then(path => { saveFolder = path; renderLocation(); }).catch(error);
const appWindow = window.__TAURI__.window.getCurrentWindow();
$('minimize').onclick = () => appWindow.minimize().catch(error);
$('close').onclick = () => appWindow.close().catch(error);
async function refresh(schedule = true) {
  try {
    const s = await invoke('status'); recording = s.recording; renderControls();
    if (s.error) error(s.error);
    else { $('message').hidden = true; $('message').classList.remove('error'); }
    savedFile = recording ? null : s.saved;
    renderLocation();
  } catch (e) { error(e); }
  if (schedule) setTimeout(refresh, 500);
}
window.__TAURI__.event.listen('recorder-error', e => error(e.payload));
window.__TAURI__.event.listen('selection-changed', e => { region = e.payload; renderArea(); });
refresh();
