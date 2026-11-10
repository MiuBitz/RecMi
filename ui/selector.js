const {invoke}=window.__TAURI__.core;
const $=id=>document.getElementById(id);
let data=null,rect=null,drag=null,busy=false;
// Preserve the screenshot aspect ratio, including during DPI/fullscreen changes.
function viewport(){
  const scale=Math.min(innerWidth/data.snapshot.width,innerHeight/data.snapshot.height);
  return {scale,x:(innerWidth-data.snapshot.width*scale)/2,y:(innerHeight-data.snapshot.height*scale)/2};
}
function render(){
  $('rectangle').hidden=!rect;$('confirm').disabled=!rect||busy;
  if(!rect||!data)return;
  const view=viewport();
  Object.assign($('rectangle').style,{left:view.x+rect.x*view.scale+'px',top:view.y+rect.y*view.scale+'px',width:rect.width*view.scale+'px',height:rect.height*view.scale+'px'});
  $('dimensions').textContent=`${rect.width} × ${rect.height} px`;
  for(const key of ['x','y','width','height'])$(key).value=rect[key];
}
function point(e){const view=viewport();return {x:Math.max(0,Math.min(data.snapshot.width,(e.clientX-view.x)/view.scale)),y:Math.max(0,Math.min(data.snapshot.height,(e.clientY-view.y)/view.scale))}}
$('canvas').onpointerdown=e=>{
  if(!data||busy||e.button!==0)return;
  const handle=e.target.dataset.handle;
  const action=handle?'resize':e.target.closest('#rectangle')?'move':'draw';
  drag={action,handle,start:point(e),original:rect?{...rect}:null};
  $('canvas').setPointerCapture(e.pointerId);$('canvas').focus();
  if(action==='draw'){rect=RectGeometry.draw(drag.start,drag.start,data.snapshot.width,data.snapshot.height);render()}
};
$('canvas').onpointermove=e=>{
  if(!drag)return;const p=point(e),dx=p.x-drag.start.x,dy=p.y-drag.start.y,w=data.snapshot.width,h=data.snapshot.height;
  rect=drag.action==='draw'?RectGeometry.draw(drag.start,p,w,h):drag.action==='move'?RectGeometry.move(drag.original,dx,dy,w,h):RectGeometry.resize(drag.original,drag.handle,dx,dy,w,h);render();
};
$('canvas').onpointerup=$('canvas').onpointercancel=()=>{drag=null};
$('reset').onclick=()=>{rect=null;drag=null;render();$('canvas').focus()};
async function finish(commit){if(busy)return;busy=true;render();try{await invoke('finish_selection',{region:rect,commit})}catch(e){$('error').textContent=String(e);busy=false;render()}}
$('cancel').onclick=()=>finish(false);$('confirm').onclick=()=>finish(true);
for(const key of ['x','y','width','height'])$(key).onchange=()=>{if(!rect||!data)return;const value=Number($(key).value);if(!Number.isFinite(value)){render();return}rect=RectGeometry.normalize({...rect,[key]:value},data.snapshot.width,data.snapshot.height);render()};
document.onkeydown=e=>{
  if(e.key==='Escape'){e.preventDefault();finish(false);return}
  if(e.target.tagName==='BUTTON')return;
  if(e.key==='Enter'&&rect){e.preventDefault();e.target.blur();finish(true);return}
  if(e.target.tagName==='INPUT')return;
  if(!rect||!data)return;
  const step=e.shiftKey?10:1,delta={ArrowLeft:[-step,0],ArrowRight:[step,0],ArrowUp:[0,-step],ArrowDown:[0,step]}[e.key];
  if(delta){e.preventDefault();rect=RectGeometry.move(rect,...delta,data.snapshot.width,data.snapshot.height);render()}
};
window.onresize=render;
invoke('selection_data').then(value=>{data=value;$('desktop').src=data.snapshot.image;rect=data.region?RectGeometry.normalize(data.region,data.snapshot.width,data.snapshot.height):null;$('error').textContent='';render();$('canvas').focus()}).catch(e=>{$('error').textContent=String(e)});
let toolbarDrag=null;
$('toolbar-grip').onpointerdown=e=>{const bounds=$('toolbar').getBoundingClientRect();toolbarDrag={x:e.clientX-bounds.left,y:e.clientY-bounds.top};$('toolbar-grip').setPointerCapture(e.pointerId)};
$('toolbar-grip').onpointermove=e=>{if(!toolbarDrag)return;const bar=$('toolbar'),bounds=bar.getBoundingClientRect();Object.assign(bar.style,{transform:'none',bottom:'auto',left:Math.max(0,Math.min(innerWidth-bounds.width,e.clientX-toolbarDrag.x))+'px',top:Math.max(0,Math.min(innerHeight-bounds.height,e.clientY-toolbarDrag.y))+'px'})};
$('toolbar-grip').onpointerup=$('toolbar-grip').onpointercancel=()=>{toolbarDrag=null};

