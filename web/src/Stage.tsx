import {memo,useEffect,useRef,useState,type PointerEvent} from 'react';
import type {Scene,Asset,Point} from './types';
import {SceneCanvas} from './SceneCanvas';
import {moveCorner,translate} from './geometry';

export const Stage=memo(function Stage({scene,assets,selected,onSelect,onChange,paused,disabled}:{scene:Scene;assets:Asset[];selected:string|null;onSelect:(id:string|null)=>void;onChange:(scene:Scene)=>void;paused:boolean;disabled:boolean}) {
  const svg=useRef<SVGSVGElement>(null);
  const [grid,setGrid]=useState(false);
  const drag=useRef<{id:string;corner:number|null;start:Point;corners:Point[]}|null>(null);
  const raf=useRef(0),pending=useRef<Scene|null>(null);
  useEffect(()=>()=>{cancelAnimationFrame(raf.current);pending.current=null},[]);
  function commit(){raf.current=0;const s=pending.current;pending.current=null;if(s)onChange(s)}
  function end(){if(raf.current)cancelAnimationFrame(raf.current);commit();drag.current=null}
  function point(event:PointerEvent):Point {const r=svg.current!.getBoundingClientRect();return {x:(event.clientX-r.left)/r.width,y:(event.clientY-r.top)/r.height}}
  function begin(event:PointerEvent,id:string,corner:number|null){if(disabled)return;event.preventDefault();event.stopPropagation();onSelect(id);svg.current!.setPointerCapture(event.pointerId);drag.current={id,corner,start:point(event),corners:structuredClone(scene.surfaces.find(s=>s.id===id)!.corners)}}
  function move(event:PointerEvent){const d=drag.current;if(!d||disabled)return;const p=point(event);const corners=d.corner===null?translate(d.corners,{x:p.x-d.start.x,y:p.y-d.start.y}):moveCorner(d.corners,d.corner,p);pending.current={...scene,surfaces:scene.surfaces.map(s=>s.id===d.id?{...s,corners}:s)};if(!raf.current)raf.current=requestAnimationFrame(commit)}
  const chosen=scene.surfaces.find(s=>s.id===selected);
  return <section className="stage-section"><div className="section-heading"><div><span className="eyebrow">ОБЩИЙ КАДР</span><h2>Ваша стена</h2></div><button className={grid?'active':''} onClick={()=>setGrid(!grid)}>Сетка</button></div>
    <div className={'stage'+(grid?' grid':'')}>
      <SceneCanvas scene={scene} assets={assets} paused={paused}/>
      <svg ref={svg} viewBox="0 0 1000 562.5" onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end} onPointerDown={()=>onSelect(null)} aria-label="Перетаскивайте области и их углы">
        {grid&&<g className="grid-lines">{Array.from({length:9},(_,i)=><line key={'v'+i} x1={(i+1)*100} y1="0" x2={(i+1)*100} y2="562.5"/>)}{Array.from({length:5},(_,i)=><line key={'h'+i} x1="0" y1={(i+1)*93.75} x2="1000" y2={(i+1)*93.75}/>)}</g>}
        <defs>{scene.surfaces.filter(s=>s.frame).map(s=><filter key={s.id} id={'neon-'+s.id} x="-100%" y="-100%" width="300%" height="300%" colorInterpolationFilters="sRGB"><feGaussianBlur stdDeviation={(s.frame!.glow||0)/5}/></filter>)}</defs>
        {scene.surfaces.filter(s=>s.frame).map(s=>{const frame=s.frame!,points=s.corners.map(p=>`${p.x*1000},${p.y*562.5}`).join(' ');return <g key={'frame-'+s.id} pointerEvents="none" fill="none" strokeLinejoin="round"><polygon points={points} stroke={frame.color} strokeWidth={frame.width/1.92+frame.glow/3} opacity=".55" filter={'url(#neon-'+s.id+')'}/><polygon points={points} stroke={frame.color} strokeWidth={frame.width/1.92}/><polygon points={points} stroke="#ffffff" strokeWidth={frame.width/5.76} opacity=".8"/></g>})}
        {scene.surfaces.map((s,i)=><g key={s.id} className={s.id===selected?'surface selected':'surface'}><polygon data-surface={s.id} points={s.corners.map(p=>`${p.x*1000},${p.y*562.5}`).join(' ')} onPointerDown={e=>begin(e,s.id,null)}/><text x={s.corners[0].x*1000+12} y={s.corners[0].y*562.5+25}>{i+1}</text></g>)}
        {chosen?.corners.map((p,i)=><g key={i}><circle data-corner={i} cx={p.x*1000} cy={p.y*562.5} r="24" fill="transparent" onPointerDown={e=>begin(e,chosen.id,i)}/><circle cx={p.x*1000} cy={p.y*562.5} r="9" className="corner" pointerEvents="none"/></g>)}
      </svg>
      {!scene.surfaces.length&&<div className="empty-stage"><span>＋</span><h3>Начните с пустой стены</h3><p>Добавьте область и выберите картину ниже</p></div>}
    </div><p className="stage-hint">Тяните область, чтобы переместить. Тяните угол, чтобы подогнать к стене. Изменения сразу видны на проекторе.</p>
  </section>;
});
