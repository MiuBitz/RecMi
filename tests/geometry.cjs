const assert = require('node:assert/strict');
const G = require('../ui/geometry.js');
const r = {x:100,y:100,width:200,height:100};
assert.deepEqual(G.draw({x:300,y:200},{x:100,y:100},1920,1080),r);
assert.deepEqual(G.move(r,-500,2000,1920,1080),{x:0,y:980,width:200,height:100});
assert.deepEqual(G.resize(r,'e',50,0,1920,1080),{...r,width:250});
assert.deepEqual(G.resize(r,'nw',-50,-50,1920,1080),{x:50,y:50,width:250,height:150});
for(const handle of ['nw','n','ne','e','se','s','sw','w']) {
  for(const delta of [-2000,-1,1,2000]) {
    const result=G.resize(r,handle,delta,delta,1920,1080);
    assert(result.x>=0&&result.y>=0&&result.x+result.width<=1920&&result.y+result.height<=1080);
    assert(result.width>=20&&result.height>=20);
    assert.equal(result.width%2,0);assert.equal(result.height%2,0);
  }
}
assert.deepEqual(G.normalize({x:1919,y:1079,width:99,height:99},1920,1080),{x:1822,y:982,width:98,height:98});
console.log('PASS: reverse drawing, moving, eight resize handles, boundaries and even dimensions');
