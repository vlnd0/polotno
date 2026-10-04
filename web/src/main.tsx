import React,{useCallback,useEffect,useRef,useState} from 'react';
import {createRoot} from 'react-dom/client';
import type {Asset,State,Scene,CatalogPage,CatalogItem,Surface} from './types';
import {api,ApiError} from './api';
import {newSurface,uuid} from './geometry';
import {PreviewQueue} from './preview';
import {Stage} from './Stage';
import {Library} from './Library';
import {ScenePicker} from './ScenePicker';
import './style.css';

function useEvent<T extends (...args:any[])=>any>(fn:T):T { const ref=useRef(fn);ref.current=fn;return useCallback(((...args:any[])=>ref.current(...args)) as T,[]); }
function App(){
  const [scene,setScene]=useState<Scene|null>(null),[assets,setAssets]=useState<Asset[]>([]),[scenes,setScenes]=useState<Scene[]>([]);
  const [selected,setSelected]=useState<string|null>(null),[status,setStatus]=useState('Подключаемся к проектору…'),[dirty,setDirty]=useState(false),[busy,setBusy]=useState(false),[paused,setPaused]=useState(false);
  const [connection,setConnection]=useState<'loading'|'pairing'|'offline'|'ready'>('loading'),[pairCode,setPairCode]=useState('');
  const qrCode=useRef(new URLSearchParams(location.hash.slice(1)).get('pair'));
  const current=useRef<Scene|null>(null);current.current=scene;
  const queue=useRef<PreviewQueue|null>(null);if(!queue.current)queue.current=new PreviewQueue(s=>api<{revision:number}>('preview','PUT',s),setStatus);

  async function connect(code?:string){
    setConnection('loading');setBusy(true);setStatus('Подключаемся к проектору…');
    try{
      let snapshot:State;
      try{snapshot=await api<State>('state')}
      catch(error){
        if(!(error instanceof ApiError)||error.status!==401)throw error;
        if(!code){setConnection('pairing');setStatus('Нужно подключить этот браузер');return}
        await api('pair','POST',{code});snapshot=await api<State>('state');
      }
      const saved=await api<Scene[]>('scenes');
      history.replaceState(null,'',location.pathname);qrCode.current=null;
      setScene(snapshot.scene);setAssets(snapshot.assets);setPaused(snapshot.paused);setScenes(saved);
      setConnection('ready');setStatus('Подключено к проектору');
    }catch(error){
      if(error instanceof ApiError&&[400,401,429].includes(error.status)){setConnection('pairing');setStatus(error.message)}
      else{setConnection('offline');setStatus('Проектор недоступен. Проверьте, что «Полотно» открыто и вы в одной сети Wi-Fi.')}
    }finally{setBusy(false)}
  }
  useEffect(()=>{void connect(qrCode.current||undefined);return()=>queue.current?.dispose()},[]);

  function edit(s:Scene){current.current=s;setScene(s);setDirty(true);queue.current!.enqueue(s)}
  function changeSurface(id:string,fn:(s:Surface)=>Surface){const s=current.current!;edit({...s,surfaces:s.surfaces.map(r=>r.id===id?fn(r):r)})}
  function add(){const s=current.current!;const surface=newSurface(s.surfaces.length+1);edit({...s,surfaces:[...s.surfaces,surface]});setSelected(surface.id)}
  async function task(fn:()=>Promise<void>){setBusy(true);try{await fn()}catch(e){setStatus((e as Error).message)}finally{setBusy(false)}}
  async function save(){await queue.current!.flush();await api('save','PUT',current.current);setScenes(await api<Scene[]>('scenes'));setDirty(false);setStatus('Сцена сохранена на проекторе')}
  function assign(asset:Asset,append=false){let s=current.current!;let id=selected;let created=false;if(!id||!s.surfaces.some(r=>r.id===id)){const region=newSurface(s.surfaces.length+1);s={...s,surfaces:[...s.surfaces,region]};id=region.id;created=true}
    if(created)setSelected(id);edit({...s,surfaces:s.surfaces.map(r=>r.id===id?{...r,calibration:false,playlist:{...r.playlist,items:append?[...r.playlist.items,asset.id]:[asset.id]}}:r)});return true;
  }
  async function choose(item:CatalogItem){setStatus(item.provider==='polotno'?'Выбираем обои…':'Скачиваем картину на проектор…');const asset=await api<Asset>('catalog/import','POST',{id:item.id,provider:item.provider});setAssets(old=>old.some(a=>a.id===asset.id)?old:[...old,asset]);if(assign(asset))setStatus('Выбрано · обновляем стену…')}
  function reorder(id:string,offset:number){const s=current.current!;const items=[...s.surfaces],i=items.findIndex(r=>r.id===id),j=i+offset;if(j<0||j>=items.length)return;[items[i],items[j]]=[items[j],items[i]];edit({...s,surfaces:items})}
  const taskEvent=useEvent(task),chooseEvent=useEvent(choose),assignEvent=useEvent(assign);
  const addAssetEvent=useCallback((asset:Asset)=>setAssets(old=>[...old,asset]),[]);
  const editEvent=useEvent(edit),selectSceneEvent=useEvent((saved:Scene)=>{edit(structuredClone(saved));setSelected(null)});
  const newSceneEvent=useEvent(()=>{edit({id:uuid(),name:'Новая сцена',surfaces:[]});setSelected(null)});
  const chosen=scene?.surfaces.find(s=>s.id===selected);
  return <><header><a className="brand" href="/">▱ <span>Полотно</span></a><div className={"connection"+(connection!=="ready"?" disconnected":"")}><i/>{connection==="ready"?"Проектор · подключено":"Нет подключения"}</div><button className="primary" disabled={!scene||busy||!dirty} onClick={()=>void task(save)}>{busy?'Выполняем…':dirty?'Сохранить сцену':'Сохранено'}</button></header>
  <div className="notice" role="status">{status}{dirty&&<span> · есть несохранённые изменения</span>}</div>
  {!scene?<div className="pairing-empty"><h1>{connection==='loading'?'Подключаемся…':connection==='offline'?'Проектор недоступен':'Подключите этот браузер'}</h1>
    {connection==='pairing'?<><p>Нажмите Menu в «Полотне» на проекторе. Сканируйте QR телефоном или введите здесь код для компьютера.</p><form className="pairing-form" onSubmit={e=>{e.preventDefault();void connect(pairCode)}}><label>Код с экрана проектора<input aria-label="Код подключения" inputMode="numeric" autoComplete="one-time-code" maxLength={6} pattern="[0-9]{6}" placeholder="000000" value={pairCode} onChange={e=>setPairCode(e.target.value.replace(/\D/g,''))}/></label><button className="primary" disabled={busy||pairCode.length!==6}>Подключиться</button></form><p className="help">Подключение сохранится в этом браузере на 30 дней.</p></>:connection==='offline'?<><p>{status}</p><button onClick={()=>void connect(qrCode.current||undefined)}>Попробовать снова</button></>:<p>Проверяем сохранённое подключение к проектору</p>}
  </div>:<main>
    <div className="workspace"><div className="composition"><div className="scene-bar"><label>Название сцены<input value={scene.name} disabled={busy} onChange={e=>edit({...scene,name:e.target.value})}/></label></div>
      <ScenePicker scenes={scenes} assets={assets} activeId={scene.id} disabled={busy} onOpen={selectSceneEvent} onNew={newSceneEvent}/>
      <Stage scene={scene} assets={assets} selected={selected} onSelect={setSelected} onChange={editEvent} paused={paused} disabled={busy}/>
      <div className="playback-bar"><span>{scene.surfaces.length} областей · 1920 × 1080</span><button onClick={()=>void task(async()=>{await api('control','POST',{command:'pause'});setPaused(!paused);setStatus(paused?'Показ продолжается':'Плейлисты на паузе')})}>{paused?'▶ Продолжить':'Ⅱ Пауза'}</button></div>
    </div><aside><div className="section-heading"><h2>Области</h2><button className="add" onClick={add} disabled={busy} aria-label="Добавить область">＋</button></div>
      <div className="surface-list">{!scene.surfaces.length&&<p>Добавьте область или выберите картину — область появится автоматически.</p>}{scene.surfaces.map((s,i)=><button className={'surface-row'+(s.id===selected?' active':'')} key={s.id} onClick={()=>setSelected(s.id)}><span className="number">{i+1}</span><span>{s.name}<small>{s.playlist.items.length?'Плейлист · '+s.playlist.items.length+' файлов':'Пока без картины'}</small></span><span>↗</span></button>)}</div>
      {chosen&&<div className="properties"><label>Название<input value={chosen.name} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,name:e.target.value}))}/></label><div className="row"><button disabled={busy} onClick={()=>{const copy={...structuredClone(chosen),id:uuid(),name:chosen.name+' — копия'};const next={...scene,surfaces:[...scene.surfaces,copy]};edit(next);setSelected(copy.id)}}>Дублировать</button><button disabled={busy} className="danger" onClick={()=>{edit({...scene,surfaces:scene.surfaces.filter(s=>s.id!==chosen.id)});setSelected(null)}}>Удалить</button></div><div className="row"><button disabled={busy} onClick={()=>reorder(chosen.id,-1)}>↓ Назад</button><button disabled={busy} onClick={()=>reorder(chosen.id,1)}>↑ Вперёд</button></div>
        <label className="checkbox"><input type="checkbox" checked={!!chosen.calibration} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,calibration:e.target.checked}))}/>Калибровочная сетка</label>
        <h3>Пропорции</h3><div className="row">{([['contain','Целиком'],['cover','Заполнить'],['stretch','Растянуть']] as const).map(([fit,title])=><button key={fit} className={(chosen.fit||'contain')===fit?'active':''} disabled={busy} onClick={()=>changeSurface(chosen.id,s=>({...s,fit}))}>{title}</button>)}</div>
        <h3>Отражение</h3>
        <label className="checkbox"><input type="checkbox" checked={!!chosen.flip_horizontal} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,flip_horizontal:e.target.checked}))}/>Отразить по горизонтали</label>
        <label className="checkbox"><input type="checkbox" checked={!!chosen.flip_vertical} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,flip_vertical:e.target.checked}))}/>Отразить по вертикали</label>
        <label className="checkbox"><input type="checkbox" checked={!!chosen.frame} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,frame:e.target.checked?{color:'#22d3ee',width:3,glow:18}:null}))}/>Неоновая рамка</label>
        {chosen.frame&&<><div className="row neon-colors">{['#22d3ee','#ec4899','#a78bfa','#f59e0b','#ffffff'].map(color=><button key={color} style={{background:color}} aria-label={'Цвет рамки '+color} disabled={busy} onClick={()=>changeSurface(chosen.id,s=>({...s,frame:{...s.frame!,color}}))}/>)}</div><label>Цвет<input type="color" value={chosen.frame.color} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,frame:{...s.frame!,color:e.target.value}}))}/></label><label>Толщина · {chosen.frame.width} px<input type="range" min="1" max="24" value={chosen.frame.width} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,frame:{...s.frame!,width:Number(e.target.value)}}))}/></label><label>Свечение<input type="range" min="0" max="80" value={chosen.frame.glow} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,frame:{...s.frame!,glow:Number(e.target.value)}}))}/></label></>}
        <h3>Плейлист</h3>{chosen.playlist.items.map((id,i)=><div className="playlist-item" key={i}><span>{assets.find(a=>a.id===id)?.name||'Файл'}<small>{assets.find(a=>a.id===id)?.kind==='video'?'Видео':'Картина'}</small></span><button aria-label="Удалить из плейлиста" disabled={busy} onClick={()=>changeSurface(chosen.id,s=>({...s,playlist:{...s.playlist,items:s.playlist.items.filter((_,j)=>j!==i)}}))}>×</button></div>)}
        <label>Смена, секунд<input type="number" min="1" value={chosen.playlist.interval_seconds} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,playlist:{...s.playlist,interval_seconds:Math.max(1,Number(e.target.value)||1)}}))}/></label><label className="checkbox"><input type="checkbox" checked={chosen.playlist.muted} disabled={busy} onChange={e=>changeSurface(chosen.id,s=>({...s,playlist:{...s.playlist,muted:e.target.checked}}))}/>Без звука</label><p className="help">Выберите картину ниже для этой области. В «Моих файлах» можно добавить несколько элементов в плейлист.</p>
      </div>}
    </aside></div>
    <Library assets={assets} hasSelection={!!chosen} busy={busy} task={taskEvent} choose={chooseEvent} assign={assignEvent} addAsset={addAssetEvent} status={setStatus}/>
  </main>}<footer>Полотно · картины живут на вашей стене</footer></>;
}
createRoot(document.getElementById('root')!).render(<App/>);
