const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const elements=new Map(),calls=[];
function element(id){if(!elements.has(id))elements.set(id,{dataset:{},style:{},value:'',tagName:'DIV',focus(){},blur(){},setPointerCapture(){},closest(){return null},getBoundingClientRect(){return {left:0,top:0,width:600,height:160}}});return elements.get(id)}
const context={console,innerWidth:960,innerHeight:540,RectGeometry:require('../ui/geometry.js'),document:{getElementById:element},window:{__TAURI__:{core:{invoke:async(name,args)=>{calls.push({name,args});if(name==='selection_data')return {snapshot:{image:'data:image/png;base64,',width:1920,height:1080},region:{x:200,y:150,width:900,height:500}}}}}}};
vm.createContext(context);vm.runInContext(fs.readFileSync('ui/selector.js','utf8'),context);
(async()=>{
 await new Promise(setImmediate);
 assert.equal(element('rectangle').style.left,'100px');assert.equal(element('rectangle').style.width,'450px');
 const inside={dataset:{},closest(){return element('rectangle')}};
 element('canvas').onpointerdown({button:0,target:inside,clientX:200,clientY:200,pointerId:1});
 element('canvas').onpointermove({clientX:300,clientY:250});element('canvas').onpointerup();
 assert.equal(element('x').value,400);assert.equal(element('y').value,250);
 element('canvas').onpointerdown({button:0,target:{dataset:{handle:'e'}},clientX:650,clientY:250,pointerId:1});
 element('canvas').onpointermove({clientX:750,clientY:250});element('canvas').onpointerup();
 assert.equal(element('width').value,1100);
 context.document.onkeydown({key:'ArrowRight',shiftKey:true,target:element('canvas'),preventDefault(){}});
 assert.equal(element('x').value,410);
 element('width').value=199;element('width').onchange();assert.equal(element('width').value,198);
 // A viewport with a different aspect ratio must letterbox, not stretch,
 // and pointer coordinates must use the same image offset as rendering.
 context.innerWidth=1000;context.window.onresize();
 assert.equal(element('rectangle').style.left,'225px');
 assert.equal(element('rectangle').style.width,'99px');
 element('canvas').onpointerdown({button:0,target:inside,clientX:225,clientY:125,pointerId:2});
 element('canvas').onpointermove({clientX:235,clientY:125});element('canvas').onpointerup();
 assert.equal(element('x').value,430);
 context.innerWidth=960;context.innerHeight=600;context.window.onresize();
 assert.equal(element('rectangle').style.top,'155px');
 element('canvas').onpointerdown({button:0,target:inside,clientX:215,clientY:155,pointerId:3});
 element('canvas').onpointermove({clientX:215,clientY:165});element('canvas').onpointerup();
 assert.equal(element('y').value,270);
 await element('confirm').onclick();
 const result=calls.find(c=>c.name==='finish_selection').args;
 assert.deepEqual(JSON.parse(JSON.stringify(result)),{region:{x:430,y:270,width:198,height:500},commit:true});
 console.log('PASS: scaled and letterboxed overlay rendering, pointer alignment, move/resize, keyboard editing and confirmation');
})().catch(e=>{console.error(e);process.exitCode=1});
