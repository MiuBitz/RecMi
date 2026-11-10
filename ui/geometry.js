(function(root){
  const clamp=(v,min,max)=>Math.max(min,Math.min(max,v));
  function normalize(r,w,h,min=20){
    const width=clamp(Math.round(r.width),Math.min(min,w),w)&~1;
    const height=clamp(Math.round(r.height),Math.min(min,h),h)&~1;
    return {x:clamp(Math.round(r.x),0,w-width),y:clamp(Math.round(r.y),0,h-height),width,height};
  }
  function draw(a,b,w,h){return normalize({x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),width:Math.abs(a.x-b.x),height:Math.abs(a.y-b.y)},w,h)}
  function move(r,dx,dy,w,h){return normalize({...r,x:r.x+dx,y:r.y+dy},w,h)}
  function resize(r,handle,dx,dy,w,h){
    let l=r.x,t=r.y,right=r.x+r.width,b=r.y+r.height;
    if(handle.includes('w'))l=clamp(l+dx,0,right-20);
    if(handle.includes('e'))right=clamp(right+dx,l+20,w);
    if(handle.includes('n'))t=clamp(t+dy,0,b-20);
    if(handle.includes('s'))b=clamp(b+dy,t+20,h);
    return normalize({x:l,y:t,width:right-l,height:b-t},w,h);
  }
  const api={normalize,draw,move,resize};
  if(typeof module!=='undefined')module.exports=api;else root.RectGeometry=api;
})(globalThis);
