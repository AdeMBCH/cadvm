//! Self-contained HTML/WebGL viewer for a geometric diff.
//!
//! Produces a single standalone `.html` file (no network, no CDN, no external
//! JS) embedding the mesh data and a small hand-written WebGL renderer.
//!
//! Layers: the full input shapes A and B are drawn as a translucent context, and
//! the three boolean pieces are drawn opaque on top — common = grey,
//! added = green, removed = red. Each layer can be toggled.

/// Build the standalone viewer HTML, embedding `mesh_json` (the raw JSON emitted
/// by `cadvm-geom mesh`).
pub fn render(title: &str, mesh_json: &str) -> String {
    TEMPLATE
        .replace("__TITLE__", &html_escape(title))
        .replace("/*__DATA__*/null", mesh_json)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>cadvm geom-diff — __TITLE__</title>
<style>
  html, body { margin: 0; height: 100%; background: #1e2024; color: #e8e8e8;
    font: 13px/1.5 system-ui, sans-serif; overflow: hidden; }
  #c { display: block; width: 100vw; height: 100vh; touch-action: none; }
  #panel { position: fixed; top: 12px; left: 12px; background: rgba(20,22,26,.82);
    border: 1px solid #3a3d44; border-radius: 8px; padding: 12px 14px; max-width: 340px; }
  #panel h1 { font-size: 14px; margin: 0 0 6px; }
  #panel .file { color: #9aa0aa; word-break: break-all; margin-bottom: 8px; }
  .row { display: flex; align-items: center; gap: 8px; margin: 4px 0; }
  .sw { width: 14px; height: 14px; border-radius: 3px; flex: none; border: 1px solid #555; }
  .count { color: #9aa0aa; margin-left: auto; font-variant-numeric: tabular-nums; }
  #hint { color: #777c85; margin-top: 8px; font-size: 12px; }
  #empty { position: fixed; inset: 0; display: none; align-items: center; justify-content: center;
    color: #9aa0aa; font-size: 15px; pointer-events: none; }
</style>
</head>
<body>
<canvas id="c"></canvas>
<div id="panel">
  <h1>Geometric diff</h1>
  <div class="file">__TITLE__</div>
  <div id="layers"></div>
  <div id="hint">drag to rotate · scroll to zoom</div>
</div>
<div id="empty">No geometry to display.</div>
<script>
const DATA = /*__DATA__*/null;

// key, label, rgb color, alpha, default-on. Faces are classified per face.
const LAYERS = [
  {key:'unchanged', label:'unchanged faces', color:[0.60,0.62,0.66], alpha:1.0, on:true},
  {key:'added',     label:'added faces',     color:[0.22,0.78,0.35], alpha:1.0, on:true},
  {key:'removed',   label:'removed faces',   color:[0.88,0.28,0.23], alpha:1.0, on:true},
];

const canvas = document.getElementById('c');
const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
function fail(msg){ const e=document.getElementById('empty'); e.textContent=msg; e.style.display='flex'; }
if (!gl) fail('WebGL not available in this browser.');

// ---- minimal column-major mat4 ----
function ident(){ return [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]; }
function mul(a,b){ const o=new Array(16); for(let c=0;c<4;c++)for(let r=0;r<4;r++){let s=0;for(let k=0;k<4;k++)s+=a[k*4+r]*b[c*4+k];o[c*4+r]=s;} return o; }
function translate(x,y,z){ const m=ident(); m[12]=x;m[13]=y;m[14]=z; return m; }
function rotX(a){ const c=Math.cos(a),s=Math.sin(a); return [1,0,0,0, 0,c,s,0, 0,-s,c,0, 0,0,0,1]; }
function rotY(a){ const c=Math.cos(a),s=Math.sin(a); return [c,0,-s,0, 0,1,0,0, s,0,c,0, 0,0,0,1]; }
function perspective(fovy,aspect,near,far){ const f=1/Math.tan(fovy/2),nf=1/(near-far);
  return [f/aspect,0,0,0, 0,f,0,0, 0,0,(far+near)*nf,-1, 0,0,2*far*near*nf,0]; }

function compile(type,src){ const s=gl.createShader(type); gl.shaderSource(s,src); gl.compileShader(s);
  if(!gl.getShaderParameter(s,gl.COMPILE_STATUS)) throw gl.getShaderInfoLog(s); return s; }

let prog, uMVP, uNormal, uColor, uAlpha, aPos, aNormal;
let center=[0,0,0], radius=1, yaw=0.6, pitch=-0.45, dist=4;
const buffers = {}; // key -> {pb,nb,count}

function setup(){
  const vs = compile(gl.VERTEX_SHADER,
    'attribute vec3 aPos; attribute vec3 aNormal; uniform mat4 uMVP; uniform mat4 uNormal;' +
    'varying vec3 vN; void main(){ vN = mat3(uNormal)*aNormal; gl_Position = uMVP*vec4(aPos,1.0); }');
  const fs = compile(gl.FRAGMENT_SHADER,
    'precision mediump float; varying vec3 vN; uniform vec3 uColor; uniform float uAlpha;' +
    'void main(){ vec3 n=normalize(vN); vec3 L=normalize(vec3(0.35,0.5,0.8));' +
    'float d=abs(dot(n,L)); float lit=0.35+0.65*d; gl_FragColor=vec4(uColor*lit,uAlpha); }');
  prog = gl.createProgram(); gl.attachShader(prog,vs); gl.attachShader(prog,fs); gl.linkProgram(prog);
  gl.useProgram(prog);
  uMVP=gl.getUniformLocation(prog,'uMVP'); uNormal=gl.getUniformLocation(prog,'uNormal');
  uColor=gl.getUniformLocation(prog,'uColor'); uAlpha=gl.getUniformLocation(prog,'uAlpha');
  aPos=gl.getAttribLocation(prog,'aPos'); aNormal=gl.getAttribLocation(prog,'aNormal');
  gl.enable(gl.DEPTH_TEST);

  const L = (DATA && DATA.layers) || {};
  const panel = document.getElementById('layers');
  let total = 0;
  for (const spec of LAYERS) {
    const m = L[spec.key];
    const tris = m && m.positions ? (m.positions.length/9)|0 : 0;
    total += tris;
    // Build the toggle row.
    const label = document.createElement('label'); label.className='row';
    const cb = document.createElement('input'); cb.type='checkbox'; cb.checked = spec.on && tris>0; cb.disabled = tris===0;
    cb.addEventListener('change', draw); spec.cb = cb;
    const sw = document.createElement('span'); sw.className='sw';
    sw.style.background = 'rgb('+spec.color.map(x=>Math.round(x*255)).join(',')+')';
    const txt = document.createElement('span'); txt.textContent = spec.label;
    const cnt = document.createElement('span'); cnt.className='count'; cnt.textContent = tris+' tris';
    label.append(cb, sw, txt, cnt); panel.append(label);

    if (tris>0){
      const pb=gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER,pb); gl.bufferData(gl.ARRAY_BUFFER,new Float32Array(m.positions),gl.STATIC_DRAW);
      const nb=gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER,nb); gl.bufferData(gl.ARRAY_BUFFER,new Float32Array(m.normals),gl.STATIC_DRAW);
      buffers[spec.key]={pb,nb,count:m.positions.length/3};
    }
  }
  if (total===0) fail('No geometry to display (meshes are empty).');

  const bb = DATA && DATA.bbox;
  if (bb){ center=[(bb.min[0]+bb.max[0])/2,(bb.min[1]+bb.max[1])/2,(bb.min[2]+bb.max[2])/2];
    const dx=bb.max[0]-bb.min[0],dy=bb.max[1]-bb.min[1],dz=bb.max[2]-bb.min[2];
    radius=0.5*Math.sqrt(dx*dx+dy*dy+dz*dz)||1; }
  dist = radius*3;
}

function drawLayer(spec, mvp, rot){
  const b = buffers[spec.key];
  if (!b || !spec.cb || !spec.cb.checked) return;
  gl.bindBuffer(gl.ARRAY_BUFFER,b.pb); gl.enableVertexAttribArray(aPos); gl.vertexAttribPointer(aPos,3,gl.FLOAT,false,0,0);
  gl.bindBuffer(gl.ARRAY_BUFFER,b.nb); gl.enableVertexAttribArray(aNormal); gl.vertexAttribPointer(aNormal,3,gl.FLOAT,false,0,0);
  gl.uniform3fv(uColor, spec.color); gl.uniform1f(uAlpha, spec.alpha);
  // Push 'removed' (old) faces slightly back so that where they are coplanar
  // with unchanged/added (new) faces, the new ones win — avoids z-fighting.
  if (spec.key === 'removed') { gl.enable(gl.POLYGON_OFFSET_FILL); gl.polygonOffset(1.0, 1.0); }
  gl.drawArrays(gl.TRIANGLES,0,b.count);
  if (spec.key === 'removed') gl.disable(gl.POLYGON_OFFSET_FILL);
}

function draw(){
  const w=canvas.clientWidth, h=canvas.clientHeight;
  if (canvas.width!==w||canvas.height!==h){ canvas.width=w; canvas.height=h; }
  gl.viewport(0,0,canvas.width,canvas.height);
  gl.clearColor(0.118,0.125,0.14,1); gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT);

  const aspect = canvas.width/Math.max(1,canvas.height);
  const proj = perspective(45*Math.PI/180, aspect, Math.max(radius*0.01,0.01), dist+radius*10);
  const rot = mul(rotY(yaw), rotX(pitch));
  const model = mul(rot, translate(-center[0],-center[1],-center[2]));
  const mvp = mul(proj, mul(translate(0,0,-dist), model));
  gl.uniformMatrix4fv(uMVP,false,new Float32Array(mvp));
  gl.uniformMatrix4fv(uNormal,false,new Float32Array(rot));

  // Opaque pass first, then translucent context with blending and no depth write.
  gl.disable(gl.BLEND); gl.depthMask(true);
  for (const s of LAYERS) if (s.alpha>=1) drawLayer(s, mvp, rot);
  gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA); gl.depthMask(false);
  for (const s of LAYERS) if (s.alpha<1) drawLayer(s, mvp, rot);
  gl.depthMask(true); gl.disable(gl.BLEND);
}

if (gl) {
  setup();
  let dragging=false, lx=0, ly=0;
  canvas.addEventListener('pointerdown',e=>{dragging=true;lx=e.clientX;ly=e.clientY;canvas.setPointerCapture(e.pointerId);});
  canvas.addEventListener('pointerup',()=>{dragging=false;});
  canvas.addEventListener('pointermove',e=>{ if(!dragging)return; yaw+=(e.clientX-lx)*0.01; pitch+=(e.clientY-ly)*0.01;
    pitch=Math.max(-1.55,Math.min(1.55,pitch)); lx=e.clientX; ly=e.clientY; draw(); });
  canvas.addEventListener('wheel',e=>{ e.preventDefault(); dist*=(e.deltaY>0?1.1:0.9); dist=Math.max(radius*0.2,dist); draw(); },{passive:false});
  window.addEventListener('resize',draw);
  draw();
}
</script>
</body>
</html>
"##;
