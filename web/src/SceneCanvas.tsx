import {memo,useEffect,useRef} from 'react';
import type {Scene,Asset} from './types';
import {projection,regionAspect,textureScale} from './geometry';

type TextureEntry={texture:WebGLTexture;source:HTMLImageElement|HTMLVideoElement;ready:boolean;dirty:boolean;width:number;height:number;uploaded:boolean;frameTime:number;preview?:HTMLCanvasElement;cancel?:()=>void};

/** Images upload once. Video textures update once per decoded frame, shared across surfaces. */
export const SceneCanvas=memo(function SceneCanvas({scene,assets,paused}:{scene:Scene;assets:Asset[];paused:boolean}){
  const canvas=useRef<HTMLCanvasElement>(null);
  const current=useRef({scene,assets,paused});current.current={scene,assets,paused};
  const refresh=useRef<()=>void>(()=>{});
  useEffect(()=>{
    const node=canvas.current!,gl=node.getContext('webgl',{alpha:false,antialias:false});if(!gl)return;
    let closed=false,raf=0;
    const entries=new Map<string,TextureEntry>();
    function shader(type:number,code:string){const s=gl!.createShader(type)!;gl!.shaderSource(s,code);gl!.compileShader(s);if(!gl!.getShaderParameter(s,gl!.COMPILE_STATUS))throw Error(gl!.getShaderInfoLog(s)||'Ошибка графики');return s}
    const vs=shader(gl.VERTEX_SHADER,'attribute vec2 position;uniform mat3 projection;varying vec2 uv;void main(){vec3 p=projection*vec3(position,1.0);gl_Position=vec4(p.xy,0.0,p.z);uv=position;}');
    const fs=shader(gl.FRAGMENT_SHADER,'precision mediump float;varying vec2 uv;uniform sampler2D content;uniform vec2 uvScale;uniform vec2 uvDirection;void main(){vec2 p=(uv-.5)*uvDirection/uvScale+.5;if(any(lessThan(p,vec2(0.0)))||any(greaterThan(p,vec2(1.0)))){gl_FragColor=vec4(0.,0.,0.,1.);}else{gl_FragColor=texture2D(content,p);}}');
    const program=gl.createProgram()!;gl.attachShader(program,vs);gl.attachShader(program,fs);gl.linkProgram(program);gl.deleteShader(vs);gl.deleteShader(fs);
    const gridProgram=gl.createProgram()!;const gridVs=shader(gl.VERTEX_SHADER,'attribute vec2 position;uniform mat3 projection;varying vec2 uv;void main(){vec3 p=projection*vec3(position,1.);gl_Position=vec4(p.xy,0.,p.z);uv=position;}');const gridFs=shader(gl.FRAGMENT_SHADER,'precision mediump float;varying vec2 uv;void main(){vec2 f=fract(uv*8.);float line=step(f.x,.025)+step(f.y,.025);float border=step(uv.x,.01)+step(.99,uv.x)+step(uv.y,.01)+step(.99,uv.y);float check=mod(floor(uv.x*8.)+floor(uv.y*8.),2.);vec3 c=vec3(.08+.05*check)+vec3(min(1.,line))*.7;if(border>0.)c=vec3(1.);if(abs(uv.x-.5)<.003||abs(uv.y-.5)<.003)c=vec3(1.,.2,.3);gl_FragColor=vec4(c,1.);}');gl.attachShader(gridProgram,gridVs);gl.attachShader(gridProgram,gridFs);gl.linkProgram(gridProgram);gl.deleteShader(gridVs);gl.deleteShader(gridFs);
    const buffer=gl.createBuffer()!;gl.bindBuffer(gl.ARRAY_BUFFER,buffer);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([0,0,1,0,0,1,0,1,1,0,1,1]),gl.STATIC_DRAW);
    const position=gl.getAttribLocation(program,'position'),matrix=gl.getUniformLocation(program,'projection'),uvScale=gl.getUniformLocation(program,'uvScale'),uvDirection=gl.getUniformLocation(program,'uvDirection');
    gl.useProgram(program);gl.uniform1i(gl.getUniformLocation(program,'content'),0);gl.clearColor(0,0,0,1);gl.enable(gl.BLEND);gl.blendFunc(gl.SRC_ALPHA,gl.ONE_MINUS_SRC_ALPHA);
    function request(){if(!closed&&!raf)raf=requestAnimationFrame(draw)}
    function remove(id:string){const entry=entries.get(id)!;entry.cancel?.();entry.source.onload=null;if(entry.source instanceof HTMLVideoElement){entry.source.onloadeddata=null;entry.source.onseeked=null;entry.source.pause();entry.source.removeAttribute('src');entry.source.load()}else entry.source.src='';gl!.deleteTexture(entry.texture);entries.delete(id)}
    function allocate(id:string,asset:Asset):TextureEntry{
      const texture=gl!.createTexture()!;gl!.bindTexture(gl!.TEXTURE_2D,texture);
      gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_MIN_FILTER,gl!.LINEAR);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_MAG_FILTER,gl!.LINEAR);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_WRAP_S,gl!.CLAMP_TO_EDGE);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_WRAP_T,gl!.CLAMP_TO_EDGE);
      const source=asset.kind==='video'?document.createElement('video'):new Image();
      const entry:TextureEntry={texture,source,ready:false,dirty:false,width:0,height:0,uploaded:false,frameTime:-1};entries.set(id,entry);
      if(source instanceof HTMLVideoElement){
        source.muted=true;source.loop=true;source.playsInline=true;source.preload='auto';
        source.onloadeddata=source.onseeked=()=>{entry.ready=true;entry.dirty=true;request()};
        let callback=0;
        const frame=()=>{if(closed||entries.get(id)!==entry)return;entry.ready=true;entry.dirty=true;request();callback=source.requestVideoFrameCallback(frame)};
        if(typeof source.requestVideoFrameCallback==='function'){callback=source.requestVideoFrameCallback(frame);entry.cancel=()=>source.cancelVideoFrameCallback(callback)}
        source.src='/api/media/'+id;
      }else{source.decoding='async';source.onload=()=>{if(closed||entries.get(id)!==entry)return;entry.ready=true;entry.dirty=true;request()};source.src='/api/media/'+id}
      return entry;
    }
    function upload(entry:TextureEntry){
      if(!entry.ready||!entry.dirty)return;
      let source:TexImageSource=entry.source;
      if(entry.source instanceof HTMLVideoElement&&Math.max(entry.source.videoWidth,entry.source.videoHeight)>640){
        const video=entry.source,ratio=640/Math.max(video.videoWidth,video.videoHeight);
        const preview=entry.preview||(entry.preview=document.createElement('canvas'));
        preview.dataset.videoTexture='true';
        const width=Math.round(video.videoWidth*ratio),height=Math.round(video.videoHeight*ratio);
        if(preview.width!==width||preview.height!==height){preview.width=width;preview.height=height}
        preview.getContext('2d',{alpha:false})!.drawImage(video,0,0,width,height);source=preview;
      }
      const width=source instanceof HTMLVideoElement?source.videoWidth:source instanceof HTMLCanvasElement?source.width:(source as HTMLImageElement).naturalWidth,height=source instanceof HTMLVideoElement?source.videoHeight:source instanceof HTMLCanvasElement?source.height:(source as HTMLImageElement).naturalHeight;
      if(!width||!height)return;
      gl!.bindTexture(gl!.TEXTURE_2D,entry.texture);
      if(!entry.uploaded||width!==entry.width||height!==entry.height){gl!.texImage2D(gl!.TEXTURE_2D,0,gl!.RGBA,gl!.RGBA,gl!.UNSIGNED_BYTE,source);entry.width=width;entry.height=height;entry.uploaded=true}
      else gl!.texSubImage2D(gl!.TEXTURE_2D,0,0,0,gl!.RGBA,gl!.UNSIGNED_BYTE,source);
      entry.dirty=false;
    }
    function draw(){
      raf=0;if(closed)return;
      const {scene,assets,paused}=current.current;
      const byId=new Map(assets.map(a=>[a.id,a]));
      const needed=new Set(scene.surfaces.filter(s=>!s.calibration).map(s=>s.playlist.items[0]).filter(Boolean));
      for(const id of entries.keys())if(!needed.has(id))remove(id);
      // Prepare each asset once, even when several surfaces share it.
      for(const id of needed){
        const asset=byId.get(id);if(!asset)continue;
        const entry=entries.get(id)||allocate(id,asset);
        if(entry.source instanceof HTMLVideoElement){
          const video=entry.source;
          if(paused){if(!video.paused)video.pause()}else if(video.paused)void video.play().catch(()=>{});
          if(typeof video.requestVideoFrameCallback!=='function'&&!paused){if(video.readyState>=2&&video.currentTime!==entry.frameTime){entry.frameTime=video.currentTime;entry.dirty=true}request()}
        }
        upload(entry);
      }
      gl!.viewport(0,0,node.width,node.height);gl!.clear(gl!.COLOR_BUFFER_BIT);gl!.useProgram(program);gl!.bindBuffer(gl!.ARRAY_BUFFER,buffer);gl!.enableVertexAttribArray(position);gl!.vertexAttribPointer(position,2,gl!.FLOAT,false,0,0);
      for(const surface of scene.surfaces){
        if(surface.calibration){gl!.useProgram(gridProgram);const pos=gl!.getAttribLocation(gridProgram,'position');gl!.enableVertexAttribArray(pos);gl!.vertexAttribPointer(pos,2,gl!.FLOAT,false,0,0);gl!.uniformMatrix3fv(gl!.getUniformLocation(gridProgram,'projection'),false,projection(surface.corners));gl!.drawArrays(gl!.TRIANGLES,0,6);gl!.useProgram(program);gl!.enableVertexAttribArray(position);gl!.vertexAttribPointer(position,2,gl!.FLOAT,false,0,0);continue}
        const entry=entries.get(surface.playlist.items[0]);if(!entry?.uploaded)continue;
        gl!.bindTexture(gl!.TEXTURE_2D,entry.texture);gl!.uniform2f(uvDirection,surface.flip_horizontal?-1:1,surface.flip_vertical?-1:1);gl!.uniform2fv(uvScale,textureScale(entry.width/entry.height,regionAspect(surface.corners,node.width,node.height),surface.fit||'contain'));gl!.uniformMatrix3fv(matrix,false,projection(surface.corners));gl!.drawArrays(gl!.TRIANGLES,0,6);
      }
    }
    const observer=new ResizeObserver(()=>{const width=Math.min(1920,Math.max(1,Math.round(node.clientWidth*Math.min(devicePixelRatio||1,2))));node.width=width;node.height=Math.round(width*9/16);request()});observer.observe(node);
    refresh.current=request;request();
    return()=>{closed=true;observer.disconnect();cancelAnimationFrame(raf);for(const id of entries.keys())remove(id);gl.deleteBuffer(buffer);gl.deleteProgram(program);gl.deleteProgram(gridProgram);refresh.current=()=>{}};
  },[]);
  useEffect(()=>refresh.current(),[scene,assets,paused]);
  return <canvas ref={canvas} width={1280} height={720} aria-label="Предпросмотр сцены"/>;
});
