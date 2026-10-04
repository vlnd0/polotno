import {memo} from 'react';
import type {Scene,Asset} from './types';
export const ScenePicker=memo(function ScenePicker({scenes,assets,activeId,disabled,onOpen,onNew}:{scenes:Scene[];assets:Asset[];activeId:string;disabled:boolean;onOpen:(s:Scene)=>void;onNew:()=>void}){
  return <nav className="scene-picker" aria-label="Выбор сцены">
    {scenes.map(scene=><button key={scene.id} className={'scene-card'+(scene.id===activeId?' active':'')} aria-label={'Открыть сцену '+scene.name} aria-pressed={scene.id===activeId} disabled={disabled} onClick={()=>onOpen(scene)}>
      <svg viewBox="0 0 160 90" aria-hidden="true"><rect width="160" height="90" fill="#15191b"/>{scene.surfaces.map(s=>{const asset=assets.find(a=>a.id===s.playlist.items[0]);return <polygon key={s.id} points={s.corners.map(p=>`${p.x*160},${p.y*90}`).join(' ')} fill={asset?.kind==='video'?'#a9784f':'#819688'} stroke="#fff8" strokeWidth=".5"/>})}</svg>
      <span>{scene.name}</span><small>{scene.surfaces.length} областей</small>
    </button>)}
    <button className="scene-card new-scene" aria-label="Новая сцена" disabled={disabled} onClick={onNew}><span className="plus">＋</span><span>Новая сцена</span></button>
  </nav>;
});
