import {memo,useEffect,useRef,useState} from 'react';
import type {Asset,CatalogItem,CatalogPage} from './types';
import {api,upload} from './api';

function ThumbnailVideo({src,poster}:{src:string;poster:string}){
  const ref=useRef<HTMLVideoElement>(null);
  useEffect(()=>{const video=ref.current;return()=>{video?.pause();video?.removeAttribute('src');video?.load()}},[]);
  return <video ref={ref} src={src} poster={poster} autoPlay muted loop playsInline/>;
}
type Tab='dynamic'|'modern'|'museum'|'library';
type Props={assets:Asset[];hasSelection:boolean;busy:boolean;task:(fn:()=>Promise<void>)=>Promise<void>;choose:(item:CatalogItem)=>Promise<void>;assign:(asset:Asset,append?:boolean)=>void;addAsset:(asset:Asset)=>void;status:(message:string)=>void};
export const Library=memo(function Library({assets,hasSelection,busy,task,choose,assign,addAsset,status:setStatus}:Props){
  const [tab,setTab]=useState<Tab>('dynamic'),[catalog,setCatalog]=useState<CatalogItem[]>([]),[catalogBusy,setCatalogBusy]=useState(false),[catalogError,setCatalogError]=useState(''),[query,setQuery]=useState(''),[page,setPage]=useState(1),[more,setMore]=useState(false),[category,setCategory]=useState('Все'),[preview,setPreview]=useState<number|string|null>(null);
  const request=useRef(0);
  async function loadCatalog(search='',next=1,section:Tab=tab){
    const version=++request.current;setCatalogBusy(true);setCatalogError('');
    const provider=section==='museum'?'met':'polotno',kind=section==='modern'?'image':'video';
    try{const result=await api<CatalogPage>(`catalog?provider=${provider}&kind=${kind}&q=${encodeURIComponent(search)}&page=${next}`);if(version!==request.current)return;setCatalog(old=>next===1?result.items:[...old,...result.items]);setPage(next);setMore(result.has_more)}catch(e){if(version===request.current)setCatalogError((e as Error).message)}finally{if(version===request.current)setCatalogBusy(false)}
  }
  useEffect(()=>{setCatalog([]);setPreview(null);setCategory('Все');if(tab!=='library')void loadCatalog('',1,tab);else ++request.current},[tab]);
  const shown=category==='Все'?catalog:catalog.filter(item=>item.category===category);
  const categories=['Все',...new Set(catalog.map(item=>item.category).filter((name):name is string=>!!name&&name!=='Музей'))];
  return <section className="library"><div className="section-heading"><div><span className="eyebrow">ЧТО ПОКАЗАТЬ</span><h2>Обои для вашей стены</h2></div><div className="tabs">
    {([['dynamic','Динамические'],['modern','Модерн'],['museum','Картины'],['library','Мои файлы']] as const).map(([key,title])=><button key={key} className={tab===key?'active':''} onClick={()=>setTab(key)}>{title}{key==='library'&&<small> {assets.length}</small>}</button>)}
  </div></div>
  {tab!=='library'?<>
    {tab==='museum'?<><form className="search" onSubmit={e=>{e.preventDefault();void loadCatalog(query)}}><input placeholder="Моне, Ван Гог, Вермеер…" aria-label="Поиск картин" value={query} onChange={e=>setQuery(e.target.value)}/><button disabled={catalogBusy}>Найти</button></form><p className="help">Открытые музейные коллекции · CC0</p></>:<><div className="categories">{categories.map(name=><button key={name} className={category===name?'active':''} onClick={()=>{setCategory(name);setPreview(null)}}>{name}</button>)}</div><p className="help">{tab==='dynamic'?'Готовые плавные видеообои. Предпросмотр — наведением или кнопкой ▶. Видео можно назначать любому числу областей.':'Современные абстракции, свет и геометрия. Статичные обои не занимают видеодекодер.'} Все встроенные обои работают без интернета.</p></>}
    {catalogError&&<div role="alert">{catalogError} <button onClick={()=>void loadCatalog(query)}>Повторить</button></div>}{catalogBusy&&!catalog.length&&<p>Открываем коллекцию…</p>}
    <div className="art-grid">{shown.map(item=><article className="art-card" key={`${item.provider}:${item.id}`} onMouseEnter={()=>{if(item.video_url)setPreview(item.id)}} onMouseLeave={()=>setPreview(current=>current===item.id?null:current)}>
      <div className="art-image wallpaper-image">{preview===item.id&&item.video_url?<ThumbnailVideo src={item.video_url} poster={item.preview_url}/>:<img src={item.preview_url} alt={item.title} loading="lazy"/>}
        {item.video_url&&<button className="play-thumbnail" aria-label={'Предпросмотр '+item.title} onClick={()=>setPreview(preview===item.id?null:item.id)}>{preview===item.id?'Ⅱ':'▶'}</button>}
      </div><div className="art-info"><h3>{item.title}</h3><p>{item.artist.split(' (')[0]}{item.date?' · '+item.date:''}</p><div className="row"><button className="primary" disabled={busy} onClick={()=>void task(()=>choose(item))}>{hasSelection?'Показать здесь':'Показать'}</button>{item.provider!=='polotno'&&<a href={item.source_url} target="_blank" rel="noreferrer" aria-label="Источник картины">↗</a>}</div></div>
    </article>)}</div>{more&&<button disabled={catalogBusy} onClick={()=>void loadCatalog(query,page+1)}>Ещё картины</button>}
  </>:<><div className="upload-row"><p>Уже на проекторе. Выберите для области или добавьте в плейлист.</p><label className="upload-button">＋ Загрузить свои<input type="file" multiple accept="image/*,video/*,.mov,.mkv,.avi,.webm,.m4v,.heic,.heif,.avif,.tif,.tiff,.gif,.bmp,.mts,.m2ts,.flv,.wmv,.mpg,.mpeg" disabled={busy} onChange={e=>{const files=Array.from(e.target.files||[]);e.target.value='';void task(async()=>{for(const file of files){setStatus('Загружаем и обрабатываем '+file.name+'…');const a:Asset=await upload(file);addAsset(a)}setStatus('Файлы сохранены на проекторе')})}}/></label></div><div className="art-grid">{assets.map(a=><article className="art-card" key={a.id} onMouseEnter={()=>{if(a.kind==='video')setPreview(a.id)}} onMouseLeave={()=>setPreview(current=>current===a.id?null:current)}><div className="art-image wallpaper-image">{a.kind==='image'?<img src={'/api/media/'+a.id} alt={a.name} loading="lazy"/>:preview===a.id?<ThumbnailVideo src={'/api/media/'+a.id} poster={'/api/media/'+a.id+'/poster'}/>:<img src={'/api/media/'+a.id+'/poster'} alt={a.name} loading="lazy"/>}{a.kind==='video'&&<button className="play-thumbnail" aria-label={'Предпросмотр '+a.name} onClick={()=>setPreview(preview===a.id?null:a.id)}>{preview===a.id?'Ⅱ':'▶'}</button>}</div><div className="art-info"><h3>{a.name}</h3>{a.original_mime&&<a href={'/api/original/'+a.id} download>Скачать оригинал</a>}<p>{a.attribution?.artist.split(' (')[0]||(a.kind==='video'?'Видео':'Свой файл')}</p><div className="row"><button className="primary" disabled={busy} onClick={()=>assign(a)}>Показать</button>{hasSelection&&<button disabled={busy} onClick={()=>assign(a,true)}>＋ В плейлист</button>}</div></div></article>)}</div>{!assets.length&&<p>Пока пусто. Выберите встроенные обои или загрузите свой файл.</p>}</>}
  </section>;
});
